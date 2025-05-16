use std::{
    sync::{Mutex, RwLock},
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
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use thiserror::Error;

use crate::{
    nix_environment::{FlakeOutput, FlakeSource, NixEnvironment, NixRunCommandOptions},
    utils::{LockOrPanic, ReadOrPanic, WriteOrPanic},
    workflow::{
        step::{execution::ExecutionError, Step, StepInfo},
        WorkflowSpecification,
    },
};

use super::step::execution::{Job, JobExecutionError, RunningJob};

// use RefCell here since Acyclic prevents us from modifying the graph
type JobGraphInner = Acyclic<DiGraph<Job, ()>>;

type JobCount = u32;

#[derive(Debug)]
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

    pub fn job_ids(&self) -> Vec<NodeIndex> {
        self.graph.nodes_iter().collect()
    }

    pub fn job_count(&self) -> usize {
        self.graph.node_count()
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

    fn update_job(&mut self, job_id: NodeIndex) -> Result<(), JobExecutionError> {
        self.job_mut(job_id).update()
    }
}

pub struct GraphExecutor {
    max_parallel_jobs: JobCount,
    job_count: usize,
    job_execution_index: usize,
    run_allocated_job_count: JobCount,
    progress: MultiProgress,
}

impl GraphExecutor {
    pub fn new(job_count: usize, max_parallel_jobs: JobCount, _keep_going: bool) -> Self {
        Self {
            max_parallel_jobs,
            job_count,
            job_execution_index: 1,
            run_allocated_job_count: 0,
            progress: MultiProgress::new(),
        }
    }

    fn build_job_progress<S: Into<String>>(
        &self,
        progress_max: Option<u32>,
        step_name: S,
    ) -> ProgressBar {
        let progress = if let Some(progress_max) = progress_max {
            ProgressBar::new(progress_max as u64)
                .with_style(
                    ProgressStyle::default_bar()
                        .template(&format!(
                            "[{job_index}/{job_count}] {{msg:.green}}  {{pos}}/{{len}}",
                            job_index = self.job_execution_index,
                            job_count = self.job_count,
                        ))
                        .expect("expected template string to be correct"),
                )
                .with_message(step_name.into())
        } else {
            ProgressBar::new_spinner()
                .with_style(
                    ProgressStyle::default_spinner()
                        .template(&format!(
                            "[{job_index}/{job_count}] {{msg:.green}} {{spinner}}",
                            job_index = self.job_execution_index,
                            job_count = self.job_count,
                        ))
                        .expect("expected template string to be correct"),
                )
                .with_message(step_name.into())
        };

        self.progress.add(progress)
    }

    fn try_initialize_job_state_update(
        executor: &RwLock<&mut Self>,
        graph: &Mutex<JobGraph>,
        job_id: NodeIndex,
    ) -> Result<Option<Job>, JobExecutionError> {
        let mut graph = graph.lock_or_panic();
        let new_state = match graph.job_mut(job_id) {
            Job::Running(running) => running.done()?.then_some(Job::Finishing),
            Job::Pending(_) => (graph.parents_are_finished(job_id) && {
                // we need to do the check and the increment together atomically
                let mut executor = executor.write_or_panic();
                (executor.run_allocated_job_count < executor.max_parallel_jobs)
                    .then(|| {
                        executor.run_allocated_job_count += 1;
                    })
                    .is_some()
            })
            .then_some(Job::Executing),
            Job::Finished(_) => None,
            _ => None,
        };

        Ok(new_state.map(|new_state| std::mem::replace(graph.job_mut(job_id), new_state)))
    }

    fn update_job_state(
        executor: &RwLock<&mut Self>,
        graph: &Mutex<JobGraph>,
        job_id: NodeIndex,
        old_state: Job,
    ) -> Result<(), JobExecutionError> {
        let new_state = match old_state {
            Job::Pending(pending) => {
                let running = pending.execute();

                let mut executor = executor.write_or_panic();
                let new_job = match running {
                    Ok(running) => Job::Running(running.with_progress(|job| {
                        executor.build_job_progress(job.progress_max(), job.step_name())
                    })?),
                    Err(JobExecutionError(_, ExecutionError::ShouldDirectlyFinish(pending))) => {
                        executor
                            .build_job_progress(None, pending.step_name())
                            .finish();
                        let job = Job::Finished(pending.finish());
                        executor.run_allocated_job_count -= 1;
                        job
                    }
                    Err(err) => return Err(err),
                };
                executor.job_execution_index += 1;

                new_job
            }
            Job::Running(running) => {
                let job = Job::Finished(running.finish()?);
                executor.write_or_panic().run_allocated_job_count -= 1;
                job
            }
            _ => unreachable!("continued above for other job states"),
        };
        let _ = std::mem::replace(graph.lock_or_panic().job_mut(job_id), new_state);

        Ok(())
    }

    pub fn execute(&mut self, graph: JobGraph) -> Result<JobGraph, JobExecutionError> {
        let executor = RwLock::new(self);
        let graph = Mutex::new(graph);
        while !graph.lock_or_panic().is_finished() {
            let job_ids = graph.lock_or_panic().job_ids();
            job_ids
                .into_iter()
                .map(|job_id| {
                    graph.lock_or_panic().update_job(job_id)?;

                    if let Some(old_state) =
                        Self::try_initialize_job_state_update(&executor, &graph, job_id)?
                    {
                        Self::update_job_state(&executor, &graph, job_id, old_state)?
                    }

                    Ok(())
                })
                .collect::<Result<Vec<_>, _>>()?;

            sleep(Duration::from_millis(10));
        }

        Ok(graph
            .into_inner()
            .expect("we want to panic when other threads panic"))
    }
}

#[derive(Debug, Error)]
pub enum WorkflowExecutionError {
    #[error("failed to execute workflow\n{0}")]
    JobExecution(#[from] JobExecutionError),
}
