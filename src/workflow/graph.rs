use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use camino::Utf8Path as Path;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use petgraph::{
    acyclic::Acyclic,
    data::{Build, DataMapMut},
    graph::{DiGraph, NodeIndex},
};
use thiserror::Error;

use crate::{
    nix_environment::{FlakeOutput, FlakeSource, NixEnvironment, NixRunCommandOptions},
    utils::LockOrPanic,
    workflow::{
        step::{execution::ExecutionError, Step, StepInfo},
        WorkflowSpecification,
    },
};

use super::step::execution::{Job, JobExecutionError};

// use RefCell here since Acyclic prevents us from modifying the graph
type JobGraphInner = Acyclic<DiGraph<Job, ()>>;

type JobCount = u32;

pub struct JobGraph {
    graph: JobGraphInner,
}

impl JobGraph {
    pub fn new(
        specification: WorkflowSpecification,
        nix_environment: &Box<dyn NixEnvironment>,
        flake_path: &Path,
    ) -> JobGraph {
        let mut graph = JobGraphInner::new();

        fn add_jobs_from_step(
            graph: &mut JobGraphInner,
            step: Step,
            nix_environment: &Box<dyn NixEnvironment>,
            flake_path: &Path,
        ) -> NodeIndex {
            let step_info = StepInfo::new(
                step.name.clone(),
                step.inputs
                    .values()
                    .flat_map(|input_list| input_list.inputs.iter().map(|input| input.path.clone()))
                    .collect(),
                step.outputs
                    .into_values()
                    .flat_map(|output_list| {
                        output_list.outputs.into_iter().map(|output| output.path)
                    })
                    .collect(),
                step.log,
                step.progress_scanning,
            );
            let run_command = nix_environment.run_command(
                FlakeOutput::new(FlakeSource::Path(flake_path.to_owned()), step.name),
                NixRunCommandOptions::default().unbuffered(),
            );

            let id = graph.add_node(step.executor.build_job(&run_command, step_info));
            for (_, input_list) in step.inputs.into_iter() {
                for input in input_list.inputs.into_iter() {
                    let parent_id =
                        add_jobs_from_step(graph, input.parent_step, nix_environment, flake_path);
                    graph.add_edge(parent_id, id, ());
                }
            }

            return id;
        }

        for (_, target_list) in specification.targets.into_iter() {
            for target in target_list.targets.into_iter() {
                add_jobs_from_step(&mut graph, target.parent_step, nix_environment, flake_path);
            }
        }

        return JobGraph { graph };
    }

    pub fn is_finished(&self) -> bool {
        self.graph.node_weights().all(|job| job.is_finished())
    }

    pub fn pending_job_count(&self) -> JobCount {
        self.graph
            .node_weights()
            .filter(|weight| weight.is_pending())
            .count() as JobCount
    }

    pub fn running_job_count(&self) -> JobCount {
        self.graph
            .node_weights()
            .filter(|weight| weight.is_running())
            .count() as JobCount
    }

    pub fn node_ids(&self) -> Vec<NodeIndex> {
        self.graph.nodes_iter().collect()
    }

    pub fn job_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn job(&mut self, node_id: NodeIndex) -> &Job {
        self.graph
            .node_weight(node_id)
            .expect("expected node_id to always come from iteration over existing nodes")
    }

    pub fn job_mut(&mut self, node_id: NodeIndex) -> &mut Job {
        self.graph
            .node_weight_mut(node_id)
            .expect("expected node_id to always come from iteration over existing nodes")
    }

    pub fn parents_are_finished(&self, node_id: NodeIndex) -> bool {
        self.graph
            .neighbors_directed(node_id, petgraph::Direction::Incoming)
            .all(|parent_id| {
                self.graph
                    .node_weight(parent_id)
                    .expect("iteration over existing nodes")
                    .is_finished()
            })
    }
}

fn update_running_jobs(
    graph: &Arc<Mutex<JobGraph>>,
    progress: &ProgressBar,
) -> Result<(), JobExecutionError> {
    let node_ids = graph.lock_or_panic().node_ids();
    for node_id in node_ids {
        match graph.lock_or_panic().job_mut(node_id) {
            Job::Running(running) => {
                if !running.done()? {
                    running.update_progress()?;
                    continue;
                }
            }
            _ => continue,
        };

        // we need to take ownership to finish the job, but we only
        // have a mutable reference; hence we replace the job with
        // it's 'Finishing' state
        let running_job = std::mem::replace(graph.lock_or_panic().job_mut(node_id), Job::Finishing);
        let finished_job = match running_job {
            Job::Running(running) => Job::Finished(running.finish()?),
            _ => unreachable!("continue above for non-running jobs"),
        };
        progress.inc(1);
        let _ = std::mem::replace(graph.lock_or_panic().job_mut(node_id), finished_job);
    }

    Ok(())
}

fn execute_pending_jobs(
    graph: &Arc<Mutex<JobGraph>>,
    progress: &MultiProgress,
) -> Result<(), JobExecutionError> {
    let node_ids = graph.lock_or_panic().node_ids();
    for node_id in node_ids {
        if !graph.lock_or_panic().job(node_id).is_pending() {
            continue;
        }

        if !graph.lock_or_panic().parents_are_finished(node_id) {
            continue;
        }

        // we need to take ownership to execute the job, but we only
        // have a mutable reference; hence we replace the job with
        // it's 'Executing' state
        let pending_job = std::mem::replace(graph.lock_or_panic().job_mut(node_id), Job::Executing);
        let job = match pending_job {
            Job::Pending(pending) => match pending.execute() {
                Err(JobExecutionError(_, ExecutionError::ShouldDirectlyFinish(pending))) => {
                    Job::Finished(pending.finish())
                }
                result => {
                    Job::Running(result?.map_progress_bar(|bar| progress.insert_from_back(1, bar)))
                }
            },
            _ => unreachable!("checked pending above"),
        };
        let _ = std::mem::replace(graph.lock_or_panic().job_mut(node_id), job);
    }

    Ok(())
}

pub fn build_graph_execution_progress(job_count: usize) -> ProgressBar {
    let progress = ProgressBar::new(job_count as u64)
        .with_style(
            ProgressStyle::default_bar()
                .template("[{bar:40.yellow/black}] {pos:>3}/{len:3} {msg}")
                .expect("expected template string to be correct")
                .progress_chars("-- "),
        )
        .with_message("finished jobs");
    progress.set_position(0);
    return progress;
}

pub fn execute_graph(
    graph: JobGraph,
    max_parallel_jobs: JobCount,
    _keep_going: bool,
) -> Result<(), JobExecutionError> {
    let global_progress = MultiProgress::new();
    let graph_progress = global_progress.add(build_graph_execution_progress(graph.job_count()));

    let graph = Arc::new(Mutex::new(graph));
    let graph_ref = graph.clone();
    let running_job_updater = std::thread::spawn(move || -> Result<(), JobExecutionError> {
        while !graph_ref.lock_or_panic().is_finished() {
            update_running_jobs(&graph_ref, &graph_progress)?
        }

        graph_progress.finish();

        Ok(())
    });

    while !graph.lock_or_panic().is_finished() {
        if graph.lock_or_panic().pending_job_count() == 0
            || graph.lock_or_panic().running_job_count() >= max_parallel_jobs
        {
            sleep(Duration::from_millis(100));
            continue;
        }

        execute_pending_jobs(&graph, &global_progress)?
    }

    running_job_updater
        .join()
        .expect("expected the running job updater to not panic")?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum WorkflowExecutionError {
    #[error("failed to execute workflow\n{0}")]
    JobExecution(#[from] JobExecutionError),
}
