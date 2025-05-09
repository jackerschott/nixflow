use std::collections::VecDeque;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use super::{
    step::execution::{Job, RunningJob},
    WorkflowError,
};

pub struct DecoratedRunningJob {
    job: RunningJob,
    bar: ProgressBar,
}

impl DecoratedRunningJob {
    fn build_progress_bar(job: &RunningJob) -> ProgressBar {
        job.progress_indicator_max()
            .map(|indicator_max| {
                ProgressBar::new(indicator_max as u64)
                    .with_style(
                        ProgressStyle::default_bar()
                            .template("[{bar:40.green/black}] {pos:>3}/{len:3} {msg}")
                            .expect("expected template string to be correct")
                            .progress_chars("-- "),
                    )
                    .with_message(job.step_name().to_owned())
            })
            .unwrap_or(
                ProgressBar::new_spinner()
                    .with_style(ProgressStyle::default_spinner())
                    .with_message(job.step_name().to_owned()),
            )
    }

    fn new(job: RunningJob) -> Self {
        let bar = Self::build_progress_bar(&job);
        Self { job, bar }
    }

    fn done(&mut self) -> Result<bool, WorkflowError> {
        self.job
            .try_wait()
            .map_err(|err| WorkflowError::JobExecution(self.job.step_name().to_owned(), err))
    }

    fn finish(self) -> Result<(), WorkflowError> {
        let step_name = self.job.step_name().to_owned();
        self.job
            .wait()
            .map_err(|err| WorkflowError::JobExecution(step_name, err))?;

        self.bar.finish();

        Ok(())
    }

    fn update_progress(&mut self) -> Result<(), WorkflowError> {
        let progress = self
            .job
            .read_progress()
            .map_err(|err| WorkflowError::JobExecution(self.job.step_name().to_owned(), err))?;

        if let Some(progress) = progress {
            self.bar.set_position(progress as u64);
        } else {
            self.bar.tick();
        }

        Ok(())
    }
}

pub struct Scheduler {
    jobs: VecDeque<Job>,
    running_jobs: Vec<DecoratedRunningJob>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
            running_jobs: Vec::new(),
        }
    }

    pub fn schedule(&mut self, job: Job) {
        self.jobs.push_back(job);
    }

    pub fn start_job(&mut self, job: Job, progress: &MultiProgress) -> Result<(), WorkflowError> {
        let step_name = job.step_name().to_owned();
        let mut running_job = DecoratedRunningJob::new(
            job.execute()
                .map_err(|err| WorkflowError::JobExecution(step_name, err))?,
        );
        running_job.bar = progress.insert_from_back(1, running_job.bar);
        self.running_jobs.push(running_job);

        Ok(())
    }

    pub fn execute_scheduled_jobs(
        mut self,
        max_parallel_jobs: u16,
        _keep_going: bool,
    ) -> Result<(), WorkflowError> {
        let progress = MultiProgress::new();
        let workflow_progress = progress.add(Self::build_workflow_progress_bar(self.jobs.len()));
        workflow_progress.set_position(0);

        for _ in 0..max_parallel_jobs {
            if let Some(job) = self.jobs.pop_front() {
                self.start_job(job, &progress)?;
            }
        }

        loop {
            if self.running_jobs.len() == 0 {
                workflow_progress.finish();
                return Ok(());
            }

            let mut ready_indices = vec![];
            for (index, job) in self.running_jobs.iter_mut().enumerate() {
                if job.done()? {
                    ready_indices.push(index);
                }
                job.update_progress()?;
            }

            for index in ready_indices.into_iter().rev() {
                let job = self.running_jobs.swap_remove(index);
                job.finish()?;
                workflow_progress.inc(1);

                if let Some(job) = self.jobs.pop_front() {
                    self.start_job(job, &progress)?;
                }
            }
        }
    }

    fn build_workflow_progress_bar(length: usize) -> ProgressBar {
        ProgressBar::new(length as u64).with_style(
            ProgressStyle::default_bar()
                .template("[{bar:40.green/black}] {pos:>3}/{len:3} {msg}")
                .expect("expected template string to be correct")
                .progress_chars("-- "),
        ).with_message("workflow")
    }
}
