use indicatif::MultiProgress;
use petgraph::graph::NodeIndex;

use crate::workflow::job::{AsFailedJob, ExecutionError, FailedJob, Job};

use super::{JobGraph, MaybeTransitioning, progress::build_progress_style};

pub struct GraphExecutionState {
    job_execution_index: u32,
    progress: MultiProgress,
    failure: bool,
}
impl GraphExecutionState {
    fn new() -> Self {
        Self {
            job_execution_index: 1,
            progress: MultiProgress::new(),
            failure: false,
        }
    }
}

pub struct GraphExecutionOptions {
    pub max_parallel_jobs: u32,
    pub keep_going: bool,
    pub inspection_target: Option<String>,
}

pub fn execute_job_graph(
    mut graph: JobGraph,
    options: GraphExecutionOptions,
) -> miette::Result<JobGraph> {
    let mut state = GraphExecutionState::new();
    while !graph.is_finished() {
        for job_index in graph.job_indices().collect::<Vec<_>>() {
            let job: Job =
                std::mem::replace(graph.job_mut(job_index), MaybeTransitioning::Transitioning)
                    .expect(
                        "transitioning job was previously stable or got replaced \
                        with a stable job in previous iteration after transition",
                    );
            let job = match update_job(&graph, job_index, job, &mut state, &options) {
                Ok(job) => job,
                Err(failed) => {
                    state.failure = !options.keep_going;
                    Job::Failed(failed)
                }
            };
            let _ = std::mem::replace(graph.job_mut(job_index), job.into());
        }
    }

    graph.jobs().for_each(|job| job.cleanup());

    return Ok(graph);
}

pub fn update_job(
    graph: &JobGraph,
    job_index: NodeIndex,
    job: Job,
    state: &mut GraphExecutionState,
    options: &GraphExecutionOptions,
) -> Result<Job, FailedJob> {
    match job {
        Job::Pending(pending) if state.failure => Ok(pending.terminate().into()),
        Job::Pending(pending)
            if graph.parents(job_index).all(|p| p.successful())
                && graph.count_stable(|job| job.is_running()) < options.max_parallel_jobs =>
        {
            let progress_style = build_progress_style(state.job_execution_index, graph.job_count());
            state.job_execution_index += 1;

            let inspect = options
                .inspection_target
                .as_ref()
                .is_some_and(|name| *name == pending.step.name);
            pending
                .execute(&state.progress, progress_style, options.keep_going, inspect)
                .map(|job| job.into())
        }
        job @ Job::Pending(_)
            if graph.parents(job_index).all(|p| p.finished())
                && graph.parents(job_index).any(|p| p.failed()) =>
        {
            let parents = graph
                .parents(job_index)
                .filter(|parent| parent.failed())
                .map(|parent| parent.step().clone())
                .collect();

            Err(ExecutionError::ParentsFailed { parents }.as_failed_job(job.report(), None))
        }
        job @ Job::Pending(_) => Ok(job),

        Job::Running(running) if state.failure => running.terminate().map(|job| job.into()),
        Job::Running(mut running) => {
            if running.done()? {
                let finished_job = running.finish()?;
                Ok(finished_job.into())
            } else {
                Ok(Job::Running(running.update_progress()?))
            }
        }

        job @ Job::Successful(_) => Ok(job),
        job @ Job::Failed(_) => Ok(job),
        job @ Job::Terminated(_) => Ok(job),
    }
}
