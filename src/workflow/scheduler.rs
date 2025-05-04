use std::collections::VecDeque;

use super::{executors::ExecutionHandle, step::Job, Error};

pub struct Scheduler<'s> {
    jobs: VecDeque<Job<'s>>,
    handles: Vec<ExecutionHandle<'s>>,
}

impl<'s> Scheduler<'s> {
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
            handles: Vec::new(),
        }
    }

    pub fn schedule(&mut self, job: Job<'s>) {
        self.jobs.push_back(job);
    }

    pub fn execute_scheduled_jobs(
        mut self,
        max_parallel_jobs: u16,
        _keep_going: bool,
    ) -> Result<(), Error> {
        for _ in 0..max_parallel_jobs {
            self.handles.push(match self.jobs.pop_front() {
                Some(job) => job.execute()?,
                None => return Ok(()),
            });
        }

        loop {
            let mut ready_indices = vec![];
            for (index, handle) in self.handles.iter_mut().enumerate() {
                if !handle
                    .try_wait()
                    .map_err(|execution_error| Error::StepExecutionFailure {
                        step_name: handle.step.name.clone(),
                        execution_error,
                    })?
                {
                    continue;
                }

                ready_indices.push(index);
            }

            for index in ready_indices.into_iter().rev() {
                let handle = self.handles.swap_remove(index);
                let handle_name = handle.step.name.clone();
                handle
                    .wait()
                    .map_err(|execution_error| Error::StepExecutionFailure {
                        step_name: handle_name.clone(),
                        execution_error,
                    })?;

                self.handles.push(match self.jobs.pop_front(){
                    Some(job) => job.execute()?,
                    None => return Ok(()),
                });
            }
        }
    }
}
