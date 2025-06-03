use camino::Utf8Path as Path;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use petgraph::{
    acyclic::Acyclic,
    data::Build,
    graph::{DiGraph, NodeIndex},
};

use crate::{
    nix_environment::{FlakeOutput, FlakeSource, NixEnvironment, NixRunCommandOptions},
    workflow::step::Step,
};

use super::{
    specification::WorkflowSpecification,
    step::execution::{AsFailedJob, ExecutionError, Job, JobExecutionError},
};

type JobCount = u32;

#[derive(Clone, Debug, Copy)]
pub enum MaybeTransitioning<T> {
    Stable(T),
    Transitioning,
}
impl<T> MaybeTransitioning<T> {
    pub fn expect(self, message: &str) -> T {
        match self {
            Self::Stable(value) => value,
            Self::Transitioning => unreachable!("{}", message),
        }
    }

    fn as_ref(&self) -> MaybeTransitioning<&T> {
        match *self {
            Self::Stable(ref value) => MaybeTransitioning::Stable(value),
            Self::Transitioning => MaybeTransitioning::Transitioning,
        }
    }

    fn stable(self) -> Option<T> {
        match self {
            Self::Stable(value) => Some(value),
            Self::Transitioning => None,
        }
    }
}
impl From<Job> for MaybeTransitioning<Job> {
    fn from(job: Job) -> Self {
        MaybeTransitioning::Stable(job)
    }
}

#[derive(Debug)]
pub struct JobGraph(DiGraph<MaybeTransitioning<Job>, ()>);

impl JobGraph {
    pub fn new(
        specification: WorkflowSpecification,
        nix_environment: &Box<dyn NixEnvironment>,
        flake_path: &Path,
    ) -> JobGraph {
        fn add_jobs_from_step(
            graph: &mut Acyclic<DiGraph<MaybeTransitioning<Job>, ()>>,
            step: Step,
            nix_environment: &Box<dyn NixEnvironment>,
            flake_path: &Path,
        ) -> NodeIndex {
            let run_command = nix_environment.run_command(
                FlakeOutput::new(FlakeSource::Path(flake_path.to_owned()), step.name.clone()),
                NixRunCommandOptions::default().unbuffered(),
            );

            let id = graph.add_node(step.executor.build_job(&run_command, step.info()).into());
            for (_, input_list) in step.inputs.into_iter() {
                for input in input_list.inputs.into_iter() {
                    let parent_id =
                        add_jobs_from_step(graph, input.parent_step, nix_environment, flake_path);
                    graph.add_edge(parent_id, id, ());
                }
            }

            return id;
        }

        let mut graph = Acyclic::new();
        for (_, targets) in specification.targets.into_iter() {
            for target in targets.into_iter() {
                add_jobs_from_step(&mut graph, target.parent_step, nix_environment, flake_path);
            }
        }

        return JobGraph(graph.into_inner());
    }

    pub fn job_indices(&self) -> impl Iterator<Item = NodeIndex> {
        self.0.node_indices()
    }

    pub fn job_count(&self) -> JobCount {
        self.0.node_count() as JobCount
    }

    pub fn count_stable(&self, mut f: impl FnMut(&Job) -> bool) -> JobCount {
        self.0
            .node_weights()
            .filter_map(|job| job.as_ref().stable())
            .filter(|job| f(*job))
            .count() as JobCount
    }

    pub fn job_mut(&mut self, job_index: NodeIndex) -> &mut MaybeTransitioning<Job> {
        self.0
            .node_weight_mut(job_index)
            .expect("job index comes from iteration over existing job indices")
    }

    pub fn is_finished(&self) -> bool {
        self.0.node_weights().all(|job| {
            job.as_ref()
                .expect("is_finished is only called outside of job transition")
                .finished()
        })
    }

    pub fn parents(&self, job_index: NodeIndex) -> impl Iterator<Item = &Job> {
        self.0
            .neighbors_directed(job_index, petgraph::Direction::Incoming)
            .map(|parent_index| {
                self.0
                    .node_weight(parent_index)
                    .expect("job_index comes from iteration over existing job indices")
                    .as_ref()
                    .expect(
                        "only one job is transitioning, so all parents of \
                        the currently transitioning job should be fine",
                    )
            })
    }
}

pub struct GraphExecutionState {
    job_count: JobCount,
    job_execution_index: usize,
    progress: MultiProgress,
}
impl GraphExecutionState {
    fn new(job_count: JobCount) -> Self {
        Self {
            job_count,
            job_execution_index: 1,
            progress: MultiProgress::new(),
        }
    }
}

pub struct GraphExecutionOptions {
    pub max_parallel_jobs: JobCount,
    pub keep_going: bool,
    pub only_warn_job_update_failures: bool,
}

pub fn execute_job_graph(
    mut graph: JobGraph,
    options: GraphExecutionOptions,
) -> Result<(), JobExecutionError> {
    let mut state = GraphExecutionState::new(graph.job_count());
    while !graph.is_finished() {
        for job_index in graph.job_indices().collect::<Vec<_>>() {
            let job: Job =
                std::mem::replace(graph.job_mut(job_index), MaybeTransitioning::Transitioning)
                    .expect(
                        "transitioning job was previously stable or got replaced \
                        with a stable job in previous iteration after transition",
                    );
            let job = update_job(&graph, job_index, job, &mut state, &options)?;
            let _ = std::mem::replace(graph.job_mut(job_index), job.into());
        }
    }

    Ok(())
}

pub fn update_job(
    graph: &JobGraph,
    job_index: NodeIndex,
    job: Job,
    state: &mut GraphExecutionState,
    options: &GraphExecutionOptions,
) -> Result<Job, JobExecutionError> {
    match job {
        Job::Pending(pending)
            if graph.parents(job_index).all(|p| p.successful())
                && graph.count_stable(|job| job.is_running()) < options.max_parallel_jobs =>
        {
            let executed_job = pending
                .execute()
                .map_running(|running| {
                    running.with_progress(
                        |job| add_job_progress(state, job.progress_max(), &job.step().name),
                        options.only_warn_job_update_failures,
                    )
                })
                .map(|job| job.into());
            executed_job
        }
        job @ Job::Pending(_) if graph.parents(job_index).any(|p| p.failed()) => {
            let parents = graph
                .parents(job_index)
                .filter(|parent| parent.failed())
                .map(|parent| parent.step().clone())
                .collect();

            Ok(ExecutionError::ParentsFailed { parents }
                .as_failed_job(job.step())
                .into())
        }
        job @ Job::Pending(_) => Ok(job),

        Job::Running(running) if running.done(options.keep_going)? => {
            let finished_job = running.finish();
            Ok(finished_job.into())
        }
        Job::Running(running) => Ok(Job::Running(
            running.progress(options.only_warn_job_update_failures)?,
        )),

        job @ Job::Successful(_) => Ok(job),
        job @ Job::Failed(_) => Ok(job),
        Job::Terminated(_) => unreachable!("jobs are never terminated in main execution loop"),
    }
}

pub fn add_job_progress<S: Into<String>>(
    state: &mut GraphExecutionState,
    progress_max: Option<u32>,
    step_name: S,
) -> ProgressBar {
    let progress = if let Some(progress_max) = progress_max {
        ProgressBar::new(progress_max as u64)
            .with_style(
                ProgressStyle::default_bar()
                    .template(&format!(
                        "[{job_index}/{job_count}] {{msg:.green}}  {{pos}}/{{len}}",
                        job_index = state.job_execution_index,
                        job_count = state.job_count,
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
                        job_index = state.job_execution_index,
                        job_count = state.job_count,
                    ))
                    .expect("expected template string to be correct"),
            )
            .with_message(step_name.into())
    };

    state.job_execution_index += 1;
    state.progress.add(progress)
}
