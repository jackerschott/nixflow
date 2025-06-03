use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use default::DefaultExecutor;
use indicatif::ProgressBar;
use miette::Diagnostic;
use serde::Deserialize;
use slurm::SlurmExecutor;
use std::{
    cell::RefCell,
    fmt::Display,
    fs::File,
    process::{Child, Command, Stdio},
    rc::Rc,
};

use super::{
    StepInfo,
    progress::{ProgressScanError, ProgressScanner},
};
use crate::nix_environment::NixRunCommand;

mod default;
mod slurm;

#[derive(Debug, Deserialize)]
pub enum Executor {
    Default(DefaultExecutor),
    Slurm(SlurmExecutor),
}

impl Executor {
    fn execution_command<'s>(&'s self, target: &Box<dyn NixRunCommand>) -> Command {
        match self {
            Executor::Default(default) => default.execution_command(target),
            Executor::Slurm(slurm) => slurm.execution_command(target),
        }
    }

    pub fn build_job(&self, command: &Box<dyn NixRunCommand>, step: StepInfo) -> Job {
        Job::new(self.execution_command(&command), step)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Executor::Default(DefaultExecutor {})
    }
}

impl std::fmt::Display for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Executor::Default(default) => write!(f, "{default}"),
            Executor::Slurm(slurm) => write!(f, "{slurm}"),
        }
    }
}

#[derive(Debug)]
pub enum Job {
    Pending(PendingJob),
    Running(RunningJob),
    Successful(SuccessfulJob),
    Failed(FailedJob),
    Terminated(TerminatedJob),
}
impl Job {
    pub fn new(command: Command, step: StepInfo) -> Self {
        Self::Pending(PendingJob::new(command, step))
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running(_))
    }

    pub fn finished(&self) -> bool {
        matches!(
            self,
            Self::Successful(_) | Self::Failed(_) | Self::Terminated(_)
        )
    }

    pub fn successful(&self) -> bool {
        matches!(self, Self::Successful(_))
    }

    pub fn failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    pub fn step(&self) -> &StepInfo {
        match self {
            Self::Pending(pending) => &pending.step,
            Self::Running(running) => &running.step,
            Self::Successful(successful) => &successful.step,
            Self::Failed(failed) => &failed.step,
            Self::Terminated(terminated) => &terminated.step,
        }
    }
}
impl From<ExecutedJob> for Job {
    fn from(executed: ExecutedJob) -> Self {
        match executed {
            ExecutedJob::Running(running) => Job::Running(running),
            ExecutedJob::Finished(successful) => Job::Successful(successful),
            ExecutedJob::Failed(failed) => Job::Failed(failed),
        }
    }
}
impl From<FinishedJob> for Job {
    fn from(finished: FinishedJob) -> Self {
        match finished {
            FinishedJob::Successful(successful) => Job::Successful(successful),
            FinishedJob::Failed(failed) => Job::Failed(failed),
        }
    }
}
impl From<PendingJob> for Job {
    fn from(pending: PendingJob) -> Self {
        Job::Pending(pending)
    }
}
impl From<RunningJob> for Job {
    fn from(running: RunningJob) -> Self {
        Job::Running(running)
    }
}
impl From<SuccessfulJob> for Job {
    fn from(successful: SuccessfulJob) -> Self {
        Job::Successful(successful)
    }
}
impl From<FailedJob> for Job {
    fn from(failed: FailedJob) -> Self {
        Job::Failed(failed)
    }
}
impl From<TerminatedJob> for Job {
    fn from(terminated: TerminatedJob) -> Self {
        Job::Terminated(terminated)
    }
}

#[derive(Debug)]
pub struct PendingJob {
    command: Command,
    step: StepInfo,
}
impl PendingJob {
    pub fn new(command: Command, step: StepInfo) -> Self {
        Self { command, step }
    }

    pub fn non_existing_associated_paths<'p>(
        &self,
        paths: &'p Vec<PathBuf>,
    ) -> Result<Vec<&'p Path>, (PathBuf, std::io::Error)> {
        paths
            .iter()
            .filter_map(|output| match std::fs::exists(output) {
                Ok(exists) => (!exists).then_some(Ok(output.as_path())),
                Err(err) => Some(Err((output.to_owned(), err))),
            })
            .collect()
    }

    fn non_existing_inputs(&self) -> Result<Vec<&Path>, ExecutionError> {
        self.non_existing_associated_paths(&self.step.inputs)
            .map_err(|(path, err)| ExecutionError::InputExistenceCheck(path, err))
    }

    fn non_existing_outputs(&self) -> Result<Vec<&Path>, ExecutionError> {
        self.non_existing_associated_paths(&self.step.outputs)
            .map_err(|(path, err)| ExecutionError::OutputExistenceCheck(path, err))
    }

    pub fn execute(mut self) -> ExecutedJob {
        let non_existing_inputs = match self.non_existing_inputs() {
            Ok(inputs) => inputs,
            Err(err) => return err.as_failed_job(self.step).into(),
        };
        if !non_existing_inputs.is_empty() {
            return ExecutionError::InputExistence {
                input_paths: non_existing_inputs
                    .into_iter()
                    .map(|path| path.to_owned())
                    .collect(),
            }
            .as_failed_job(self.step)
            .into();
        }

        let non_existing_outputs = match self.non_existing_outputs() {
            Ok(outputs) => outputs,
            Err(err) => return err.as_failed_job(self.step).into(),
        };
        if non_existing_outputs.is_empty() {
            return SuccessfulJob::new(self.step).into();
        }

        match std::fs::create_dir_all(
            self.step
                .log
                .parent()
                .expect("expected log to be validated as a file path"),
        ) {
            Ok(()) => {}
            Err(err) => {
                return ExecutionError::LogFileParentDirectoryCreation(self.step.log.clone(), err)
                    .as_failed_job(self.step)
                    .into();
            }
        };
        let log_file = match File::create(&self.step.log) {
            Ok(file) => file,
            Err(err) => {
                return ExecutionError::LogFileCreation(self.step.log.clone(), err)
                    .as_failed_job(self.step)
                    .into();
            }
        };
        let log_file_stderr = match log_file.try_clone() {
            Ok(file) => file,
            Err(err) => {
                return ExecutionError::LogFileDuplication(self.step.log.clone(), err)
                    .as_failed_job(self.step)
                    .into();
            }
        };

        let child = match self
            .command
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_stderr))
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                return ExecutionError::Spawn(format!("{:?}", self.command), err)
                    .as_failed_job(self.step)
                    .into();
            }
        };

        RunningJob::new(child, self.command, self.step).into()
    }
}

#[derive(Debug)]
pub struct ProgressHandler {
    scanner: Option<ProgressScanner>,
    bar: ProgressBar,
}

impl ProgressHandler {
    fn new(scanner: Option<ProgressScanner>, bar: ProgressBar) -> Self {
        Self { bar, scanner }
    }

    fn update<P: AsRef<Path>>(&mut self, log: &P) -> Result<(), ExecutionError> {
        match &mut self.scanner {
            None => self.bar.tick(),
            Some(scan_info) => {
                let log_contents = std::fs::read_to_string(log.as_ref())
                    .map_err(|err| ExecutionError::ProgressLogRead(log.as_ref().to_owned(), err))?;

                let progress = scan_info
                    .read_progress(log_contents)
                    .map_err(|err| ExecutionError::ProgressScan(log.as_ref().to_owned(), err))?;

                self.bar.set_position(progress as u64);
            }
        }

        Ok(())
    }

    fn finish(&self) {
        self.bar.finish();
    }
}

#[derive(Debug)]
pub struct RunningJob {
    child: RefCell<Child>,
    command: Command,
    progress: Option<ProgressHandler>,
    step: StepInfo,
    warnings: Rc<RefCell<Vec<ExecutionError>>>,
}

impl RunningJob {
    pub fn new(child: Child, command: Command, step: StepInfo) -> Self {
        Self {
            child: RefCell::new(child),
            command,
            progress: None,
            step,
            warnings: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn progress_max(&self) -> Option<u32> {
        self.step
            .progress_scanning
            .as_ref()
            .map(|info| info.indicator_max)
    }

    pub fn with_progress(
        mut self,
        mut build_progress: impl FnMut(&Self) -> ProgressBar,
        only_warn_on_failure: bool,
    ) -> Result<Self, JobExecutionError> {
        let result = self
            .step
            .progress_scanning
            .as_ref()
            .map(|scanning_info| ProgressScanner::new(scanning_info))
            .transpose()
            .map_err(|err| ExecutionError::ProgressScanSetup(err));

        let progress_scanner = if only_warn_on_failure {
            result.warn(&self)
        } else {
            Some(result.attach_step_info(&self.step)?)
        };

        if let Some(progress_scanner) = progress_scanner {
            self.progress = Some(ProgressHandler::new(
                progress_scanner,
                build_progress(&self),
            ));
        }

        Ok(self)
    }

    pub fn done(&self, only_warn_on_failure: bool) -> Result<bool, JobExecutionError> {
        let result = self
            .child
            .borrow_mut()
            .try_wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", self.command), err))
            .map(|status| status.is_some());

        if only_warn_on_failure {
            Ok(result.warn(self).unwrap_or(false))
        } else {
            result.attach_step_info(&self.step)
        }
    }

    pub fn finish(self) -> FinishedJob {
        let exit_status = match self.child.borrow_mut().wait() {
            Ok(status) => status,
            Err(err) => {
                return ExecutionError::Wait(format!("{:?}", &self.command), err)
                    .as_failed_job_warnings(self.step, self.warnings)
                    .into();
            }
        };

        let finished_job: FinishedJob = match exit_status.code() {
            Some(0) => SuccessfulJob::new(self.step).into(),
            Some(code) => ExecutionError::NonZeroExitCode(format!("{:?}", self.command), code)
                .as_failed_job_warnings(self.step, self.warnings)
                .into(),
            None => ExecutionError::SignalTermination(format!("{:?}", self.command))
                .as_failed_job_warnings(self.step, self.warnings)
                .into(),
        };

        self.progress.inspect(|progress| progress.finish());

        return finished_job;
    }

    #[allow(unused)]
    pub fn terminate(self) -> Result<TerminatedJob, JobExecutionError> {
        self.child
            .borrow_mut()
            .kill()
            .map_err(|err| ExecutionError::Kill(format!("{:?}", &self.command), err))
            .attach_step_info(&self.step)?;

        self.progress.inspect(|progress| progress.finish());

        return Ok(TerminatedJob::new(self.step));
    }

    pub fn step(&self) -> &StepInfo {
        &self.step
    }

    pub fn progress(mut self, only_warn_on_failure: bool) -> Result<RunningJob, JobExecutionError> {
        let result = match &mut self.progress {
            Some(progress) => progress.update(&self.step.log),
            None => Ok(()),
        };

        if only_warn_on_failure {
            result.warn(&self);
            Ok(self)
        } else {
            result.attach_step_info(&self.step).map(|_| self)
        }
    }

    pub fn println<D: Display>(&self, message: D) {
        match &self.progress {
            Some(progress) if progress.bar.is_hidden() => {
                progress.bar.println(format!("{}", message))
            }
            Some(progress) => progress.bar.suspend(|| println!("{}", message)),
            None => println!("{}", message),
        }
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct FailedJob {
    error: ExecutionError,
    warnings: Rc<RefCell<Vec<ExecutionError>>>,
    step: StepInfo,
}
impl FailedJob {
    pub fn new(
        step: StepInfo,
        error: ExecutionError,
        warnings: Rc<RefCell<Vec<ExecutionError>>>,
    ) -> Self {
        Self {
            step,
            error,
            warnings,
        }
    }
}

#[derive(Debug)]
pub struct SuccessfulJob {
    step: StepInfo,
}
impl SuccessfulJob {
    pub fn new(step: StepInfo) -> Self {
        Self { step }
    }
}

#[derive(Debug)]
pub struct TerminatedJob {
    step: StepInfo,
}
impl TerminatedJob {
    #![allow(unused)]
    pub fn new(step: StepInfo) -> Self {
        Self { step }
    }
}

pub enum ExecutedJob {
    Running(RunningJob),
    Finished(SuccessfulJob),
    Failed(FailedJob),
}
impl ExecutedJob {
    #![allow(unused)]
    pub fn is_running(&self) -> bool {
        matches!(self, ExecutedJob::Running(_))
    }

    pub fn map_running<F>(self, f: F) -> Result<ExecutedJob, JobExecutionError>
    where
        F: FnOnce(RunningJob) -> Result<RunningJob, JobExecutionError>,
    {
        match self {
            ExecutedJob::Running(running) => Ok(Self::Running(f(running)?)),
            job => Ok(job),
        }
    }
}
impl From<FailedJob> for ExecutedJob {
    fn from(failed: FailedJob) -> Self {
        ExecutedJob::Failed(failed)
    }
}
impl From<SuccessfulJob> for ExecutedJob {
    fn from(successful: SuccessfulJob) -> Self {
        ExecutedJob::Finished(successful)
    }
}
impl From<RunningJob> for ExecutedJob {
    fn from(running: RunningJob) -> Self {
        ExecutedJob::Running(running)
    }
}

pub enum FinishedJob {
    Successful(SuccessfulJob),
    Failed(FailedJob),
}
impl From<SuccessfulJob> for FinishedJob {
    fn from(successful: SuccessfulJob) -> Self {
        FinishedJob::Successful(successful)
    }
}
impl From<FailedJob> for FinishedJob {
    fn from(value: FailedJob) -> Self {
        FinishedJob::Failed(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("failed to spawn `{0}`\n{1}")]
    Spawn(String, std::io::Error),

    #[error("failed to check for the existence of {0}\n{1}")]
    InputExistenceCheck(PathBuf, std::io::Error),

    #[error(
        "the following inputs do not exist:\n\t{}",
        input_paths.iter().map(|path| format!("`{path}`")).collect::<Vec<_>>().join("\n\t")
    )]
    InputExistence { input_paths: Vec<PathBuf> },

    #[error("failed to check for the existence of {0}\n{1}")]
    OutputExistenceCheck(PathBuf, std::io::Error),

    #[error("failed to create the parent directory for the specified log file `{0}`\n{1}")]
    LogFileParentDirectoryCreation(PathBuf, std::io::Error),

    #[error("failed to create the log file `{0}`")]
    LogFileCreation(PathBuf, std::io::Error),

    #[error("failed to duplicate log file handle to `{0}`\n{1}")]
    LogFileDuplication(PathBuf, std::io::Error),

    #[error("failed to poll `{0}`\n{1}")]
    Wait(String, std::io::Error),

    #[allow(unused)]
    #[error("failed to kill `{0}`\n{1}")]
    Kill(String, std::io::Error),

    #[error("failed to execute `{0}`, terminated by a signal")]
    SignalTermination(String),

    #[error("failed to execute `{0}`, exit code {1} is non-zero")]
    NonZeroExitCode(String, i32),

    #[error("failed to setup progress scanning\n{0}")]
    ProgressScanSetup(ProgressScanError),

    #[error("failed to read progress from `{0}`\n{1}")]
    ProgressLogRead(PathBuf, std::io::Error),

    #[error("failed to read progress from `{0}`\n{1}")]
    ProgressScan(PathBuf, ProgressScanError),

    #[error(
        "one or more parent jobs failed:\n\t{}",
        parents.into_iter().map(|step| step.name.as_str()).collect::<Vec<_>>().join("\n\t"))
    ]
    ParentsFailed { parents: Vec<StepInfo> },
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error(
    "failure while executing `{name}`\n\
    {error}\n\
    check {log} or execute nixflow with `--inspect {name}` to inspect the job output",
    name = step.name,
    log = step.log,
)]
#[diagnostic(help("try doing this instead"))]
pub struct JobExecutionError {
    pub step: StepInfo,
    pub error: ExecutionError,
}

pub trait AsFailedJob {
    fn as_failed_job<S: Into<StepInfo>>(self, step: S) -> FailedJob;

    fn as_failed_job_warnings<S: Into<StepInfo>>(
        self,
        step: S,
        warnings: Rc<RefCell<Vec<ExecutionError>>>,
    ) -> FailedJob;
}

impl AsFailedJob for ExecutionError {
    fn as_failed_job<S: Into<StepInfo>>(self, step: S) -> FailedJob {
        FailedJob::new(step.into(), self, Rc::default())
    }

    fn as_failed_job_warnings<S: Into<StepInfo>>(
        self,
        step: S,
        warnings: Rc<RefCell<Vec<ExecutionError>>>,
    ) -> FailedJob {
        FailedJob::new(step.into(), self, warnings)
    }
}

pub trait AttachStepInfo<T> {
    fn attach_step_info<S: Into<StepInfo>>(self, step: S) -> Result<T, JobExecutionError>;
}

impl<T> AttachStepInfo<T> for Result<T, ExecutionError> {
    fn attach_step_info<S: Into<StepInfo>>(self, step: S) -> Result<T, JobExecutionError> {
        self.map_err(|error| JobExecutionError {
            step: step.into(),
            error,
        })
    }
}

trait Warn<T> {
    fn warn(self, job: &RunningJob) -> Option<T>;
}

impl<T> Warn<T> for Result<T, ExecutionError> {
    fn warn(self, job: &RunningJob) -> Option<T> {
        let err = match self {
            Ok(value) => return Some(value),
            Err(err) => err,
        };

        if job.warnings.borrow().len() == 0 {
            job.println(format!(
                "warning: failed to update the progress of {} at least once",
                job.step.name
            ));
        }
        job.warnings.borrow_mut().push(err);

        return None;
    }
}
