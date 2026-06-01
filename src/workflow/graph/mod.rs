use camino::Utf8Path as Path;
use petgraph::{
    acyclic::Acyclic,
    data::Build,
    graph::{DiGraph, NodeIndex},
};

use crate::{nix_environment::{FlakeOutput, FlakeSource, NixEnvironment, NixRunCommandOptions}, workflow::job::execution::JobExecutionCommand};

use super::{
    job::{execution::ExecutionMethod, Job},
    specification::{Step, WorkflowSpecification},
};

pub mod execution;
pub mod progress;

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
        profile: &str,
        execution_method: ExecutionMethod,
    ) -> JobGraph {
        fn add_jobs_from_step(
            graph: &mut Acyclic<DiGraph<MaybeTransitioning<Job>, ()>>,
            step: Step,
            nix_environment: &Box<dyn NixEnvironment>,
            flake_path: &Path,
            profile: &str,
            execution_method: ExecutionMethod,
        ) -> NodeIndex {
            let run_command = nix_environment.run_command(
                FlakeOutput::new(
                    FlakeSource::Path(flake_path.to_owned()),
                    format!("{name}.{profile}", name = step.name),
                ),
                NixRunCommandOptions::default().unbuffered(),
            );

            let command = JobExecutionCommand::new(execution_method, &run_command, step.log, step.execution);
            let id = graph.add_node(
                Job::new(command, step.info()).into(),
            );
            for (_, input_list) in step.inputs.into_iter() {
                for input in input_list.inputs.into_iter() {
                    let parent_id = add_jobs_from_step(
                        graph,
                        input.parent_step,
                        nix_environment,
                        flake_path,
                        profile,
                        execution_method,
                    );
                    graph.add_edge(parent_id, id, ());
                }
            }

            return id;
        }

        let mut graph = Acyclic::new();
        for (_, targets) in specification.targets.into_iter() {
            for target in targets.into_iter() {
                add_jobs_from_step(
                    &mut graph,
                    target.parent_step,
                    nix_environment,
                    flake_path,
                    profile,
                );
            }
        }

        return JobGraph(graph.into_inner());
    }

    pub fn job_indices(&self) -> impl Iterator<Item = NodeIndex> {
        self.0.node_indices()
    }

    pub fn jobs(&self) -> impl Iterator<Item = &Job> {
        self.0
            .node_weights()
            .map(|job| job.as_ref().expect("never called in main execution loop"))
    }

    pub fn job_count(&self) -> u32 {
        self.0.node_count() as u32
    }

    pub fn count_stable(&self, mut f: impl FnMut(&Job) -> bool) -> u32 {
        self.0
            .node_weights()
            .filter_map(|job| job.as_ref().stable())
            .filter(|job| f(*job))
            .count() as u32
    }

    pub fn job_mut(&mut self, index: NodeIndex) -> &mut MaybeTransitioning<Job> {
        self.0
            .node_weight_mut(index)
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

    pub fn print_report(&self) {
        for job in self.0.node_weights() {
            let job = job.as_ref().expect("only called after execution");
            match job {
                Job::Failed(failed) => {
                    println!("{:?}", miette::Report::new(failed.clone()));
                }
                Job::Successful(_) | Job::Terminated(_) => {}
                _ => unreachable!(),
            }
        }
    }
}
