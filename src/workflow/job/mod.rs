use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use derive_more::Debug;
use execution::{
    ExecutionError, ExecutionMethod, JobExecutionChild, JobExecutionCommand, JobExecutionError,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use miette::Diagnostic;
use std::{
    error::Error,
    fmt::Display,
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Write},
    process::Command,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};
use warnings::{ErrorCatcher, TryCatch};

use crate::utils::{IoError, JoinOrPanic};

use super::specification::{
    StepInfo,
    progress::{ProgressScanError, ProgressScanner},
};

pub mod execution;
pub mod warnings;

#[derive(Debug)]
pub enum Job {
    Pending(PendingJob),
    Running(RunningJob),
    Successful(SuccessfulJob),
    Failed(FailedJob),
    Terminated(TerminatedJob),
}
impl Job {
    pub fn new(command: Box<dyn JobExecutionCommand>, step: StepInfo) -> Self {
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
            Self::Successful(successful) => &successful.report.step,
            Self::Failed(failed) => &failed.report.step,
            Self::Terminated(terminated) => &terminated.report.step,
        }
    }

    pub fn report(&self) -> JobReport {
        match self {
            Self::Pending(pending) => pending.report(),
            Self::Running(running) => running.report(),
            Self::Successful(successful) => successful.report.clone(),
            Self::Failed(failed) => failed.report.clone(),
            Self::Terminated(terminated) => terminated.report.clone(),
        }
    }

    pub fn cleanup(&self) {
        match self {
            Self::Successful(successful) => successful.cleanup(),
            Self::Failed(failed) => failed.cleanup(),
            Self::Terminated(terminated) => terminated.cleanup(),
            _ => unreachable!(),
        }
    }
}
macro_rules! impl_from_variant {
    ($job:ty, $variant:ident) => {
        impl From<$job> for Job {
            fn from(job: $job) -> Self {
                Job::$variant(job)
            }
        }
    };
}
impl_from_variant!(PendingJob, Pending);
impl_from_variant!(RunningJob, Running);
impl_from_variant!(SuccessfulJob, Successful);
impl_from_variant!(FailedJob, Failed);
impl_from_variant!(TerminatedJob, Terminated);

impl From<ExecutedJob> for Job {
    fn from(executed: ExecutedJob) -> Self {
        match executed {
            ExecutedJob::Running(running) => Job::Running(running),
            ExecutedJob::Finished(successful) => Job::Successful(successful),
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

#[derive(Debug)]
pub struct PendingJob {
    command: Box<dyn JobExecutionCommand>,
    pub step: StepInfo,
}
impl PendingJob {
    pub fn new(command: Box<dyn JobExecutionCommand>, step: StepInfo) -> Self {
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

    fn non_existing_inputs(&self) -> Result<Vec<&Path>, JobError> {
        self.non_existing_associated_paths(&self.step.inputs)
            .map_err(|(path, err)| JobError::InputExistenceCheck(path, err.into()))
    }

    fn non_existing_outputs(&self) -> Result<Vec<&Path>, JobError> {
        self.non_existing_associated_paths(&self.step.outputs)
            .map_err(|(path, err)| JobError::OutputExistenceCheck(path, err.into()))
    }

    pub fn terminate(self) -> TerminatedJob {
        TerminatedJob::new(
            JobReport {
                warnings: Vec::new(),
                step: self.step,
            },
            None,
        )
    }

    pub fn execute(
        self,
        progress: &MultiProgress,
        progress_style: JobProgressStyle,
        prefer_warnings: bool,
        inspect: bool,
    ) -> Result<ExecutedJob, FailedJob> {
        let non_existing_inputs = self
            .non_existing_inputs()
            .map_err(|err| FailedJob::new(err, self.report(), None))?;
        if !non_existing_inputs.is_empty() {
            return Err(JobError::InputExistence {
                input_paths: non_existing_inputs
                    .into_iter()
                    .map(|path| path.to_owned())
                    .collect(),
            }
            .as_failed_job(self.report(), None));
        }

        let non_existing_outputs = self
            .non_existing_outputs()
            .map_err(|err| FailedJob::new(err, self.report(), None))?;
        if non_existing_outputs.is_empty() {
            return Ok(SuccessfulJob::new(
                JobReport {
                    warnings: Vec::new(),
                    step: self.step,
                },
                None,
            )
            .into());
        }

        std::fs::create_dir_all(
            self.step
                .log
                .parent()
                .expect("expected log to be validated as a file path"),
        )
        .map_err(|err| {
            JobError::LogFileParentDirectoryCreation(self.step.log.clone(), err.into())
                .as_failed_job(self.report(), None)
        })?;

        let report = self.report();
        let child = self
            .command
            .spawn()
            .map_err(|err| err.into())
            .map_err(|err: JobError| err.as_failed_job(report, None))?;

        match RunningJob::new(
            child,
            self.step.clone(),
            progress,
            progress_style,
            prefer_warnings,
            inspect,
        ) {
            Ok(job) => Ok(job.into()),
            Err(err) => Err(err.as_failed_job(
                JobReport {
                    warnings: Vec::new(),
                    step: self.step,
                },
                None,
            )),
        }
    }

    fn report(&self) -> JobReport {
        JobReport {
            warnings: Vec::new(),
            step: self.step.clone(),
        }
    }
}

pub struct JobProgressStyle {
    pub bar_style: ProgressStyle,
    pub spinner_style: ProgressStyle,
    pub failed_bar_style: ProgressStyle,
    pub failed_spinner_style: ProgressStyle,
}

#[derive(Clone, Debug)]
pub struct ProgressHandler {
    scanner: Option<ProgressScanner>,
    bar: ProgressBar,
    #[debug(skip)]
    failed_style: ProgressStyle,
}

impl ProgressHandler {
    fn new<S: Into<String>>(
        step_name: S,
        progress_max: Option<u32>,
        scanner: Option<ProgressScanner>,
        style: JobProgressStyle,
    ) -> Self {
        let (bar, failed_style) = if let Some(progress_max) = progress_max {
            (
                ProgressBar::new(progress_max as u64)
                    .with_style(style.bar_style)
                    .with_message(step_name.into()),
                style.failed_bar_style,
            )
        } else {
            (
                ProgressBar::new_spinner()
                    .with_style(style.spinner_style)
                    .with_message(step_name.into()),
                style.failed_spinner_style,
            )
        };

        Self {
            bar,
            scanner,
            failed_style,
        }
    }

    fn update<P: AsRef<Path>>(&mut self, log: &P) -> Result<(), JobError> {
        match &mut self.scanner {
            None => self.bar.tick(),
            Some(scan_info) => {
                let log_contents = std::fs::read_to_string(log.as_ref()).map_err(|err| {
                    JobError::ProgressLogRead(log.as_ref().to_owned(), err.into())
                })?;

                let progress = scan_info
                    .read_progress(log_contents)
                    .map_err(|err| JobError::ProgressScan(log.as_ref().to_owned(), err))?;

                self.bar.set_position(progress as u64);
            }
        }

        Ok(())
    }

    fn set_as_failed(&self) {
        self.bar.set_style(self.failed_style.clone());
        self.bar.tick();
    }
}

#[derive(Debug)]
struct JobOutputInspector {
    stop: Arc<AtomicBool>,
    stdout_handle: JoinHandle<Result<(), JobError>>,
    stderr_handle: JoinHandle<Result<(), JobError>>,
}
impl JobOutputInspector {
    fn new<P: Into<PathBuf>>(child: &JobExecutionChild, progress: &MultiProgress, log: P) -> Self {
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .expect("stdout exists when inspect is true"),
        );
        let stderr = BufReader::new(
            child
                .stderr
                .take()
                .expect("stderr exists when inspect is true"),
        );

        let stop = Arc::new(AtomicBool::new(false));
        let log = log.into();
        Self {
            stdout_handle: JobOutputInspector::write(stdout, progress, log.clone(), &stop),
            stderr_handle: JobOutputInspector::write(stderr, progress, log, &stop),
            stop,
        }
    }

    fn write<R: io::Read + Send + 'static>(
        reader: BufReader<R>,
        progress: &MultiProgress,
        log: PathBuf,
        stop: &Arc<AtomicBool>,
    ) -> JoinHandle<Result<(), JobError>> {
        let progress = progress.clone();
        let stop_stdout = stop.clone();
        thread::spawn(move || -> Result<_, JobError> {
            for line in reader.lines() {
                if stop_stdout.load(Ordering::Acquire) {
                    break;
                }

                let line = line.map_err(|err| JobError::InspectionOutputRead(err.into()))?;
                progress
                    .println(&line)
                    .map_err(|err| JobError::InspectionOutputPrint(err.into()))?;

                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log)
                    .map_err(|err| JobError::InspectionOutputWriteToLog(err.into()))?
                    .write_all(format!("{line}\n").as_bytes())
                    .map_err(|err| JobError::InspectionOutputWriteToLog(err.into()))?;
            }
            Ok(())
        })
    }

    fn join(self) -> Result<(), JobError> {
        self.stop.store(true, Ordering::Relaxed);
        self.stdout_handle.join_or_panic()?;
        self.stderr_handle.join_or_panic()?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct RunningJob {
    child: Box<dyn JobExecutionChild>,
    pub progress: ProgressHandler,
    error_catcher: ErrorCatcher,
    step: StepInfo,
    output_inspector: Option<JobOutputInspector>,
}

impl RunningJob {
    pub fn new(
        child: Box<dyn JobExecutionChild>,
        step: StepInfo,
        progress: &MultiProgress,
        progress_style: JobProgressStyle,
        prefer_warnings: bool,
        inspect: bool,
    ) -> Result<Self, JobError> {
        let mut error_catcher = ErrorCatcher::new(!prefer_warnings);

        let progress_scanner = step
            .progress_scanning
            .as_ref()
            .map(|info| ProgressScanner::new(&info))
            .transpose()
            .map_err(|err| JobError::ProgressScanSetup(err))
            .try_catch(&mut error_catcher)?
            .unwrap_or(None);

        let mut progress_handler = ProgressHandler::new(
            step.name.clone(),
            step.progress_max(),
            progress_scanner,
            progress_style,
        );
        progress_handler.bar = progress.add(progress_handler.bar);

        Ok(Self {
            output_inspector: inspect.then(|| JobOutputInspector::new(&child, progress, &step.log)),
            child,
            progress: progress_handler,
            step,
            error_catcher,
        })
    }

    pub fn cleanup_fail(&mut self) -> Result<(), JobError> {
        self.progress.set_as_failed();
        self.output_inspector
            .take()
            .map(|inspector| inspector.join())
            .transpose()?;
        Ok(())
    }

    pub fn cleanup_success(&mut self) -> Result<(), JobError> {
        self.output_inspector
            .take()
            .map(|inspector| inspector.join())
            .transpose()?;
        Ok(())
    }

    pub fn done(&mut self) -> Result<bool, FailedJob> {
        let result = self
            .child
            .borrow_mut()
            .try_wait()
            .map_err(|err| {
                JobError::Wait(format!("{:?}", self.command), err.into())
                    .as_failed_job(self.report(), Some(self.progress.bar.clone()))
            })
            .map(|status| status.is_some());

        if result.is_err() {
            // we only care about the first error
            let _ = self.cleanup_fail();
        }

        return result;
    }

    pub fn finish(mut self) -> Result<SuccessfulJob, FailedJob> {
        let exit_status = self.child.borrow_mut().wait().map_err(|err| {
            JobError::Wait(format!("{:?}", &self.command), err.into())
                .as_failed_job(self.report(), Some(self.progress.bar.clone()))
        })?;

        match exit_status.code() {
            Some(0) => {
                self.cleanup_success()
                    .try_catch(&mut self.error_catcher)
                    .map_err(|err| {
                        FailedJob::new(err, self.report(), Some(self.progress.bar.clone()))
                    })?;
                Ok(SuccessfulJob::new(
                    JobReport {
                        warnings: self.error_catcher.warnings,
                        step: self.step,
                    },
                    Some(self.progress.bar),
                ))
            }
            Some(code) => {
                // we only care about the first error
                let _ = self.cleanup_fail();
                Err(
                    JobError::NonZeroExitCode(format!("{:?}", self.command), code)
                        .as_failed_job(self.report(), Some(self.progress.bar)),
                )
            }
            None => {
                // we only care about the first error
                let _ = self.cleanup_fail();
                Err(JobError::SignalTermination(format!("{:?}", self.command))
                    .as_failed_job(self.report(), Some(self.progress.bar)))
            }
        }
    }

    pub fn terminate(mut self) -> Result<TerminatedJob, FailedJob> {
        let result = match self.child.borrow_mut().kill() {
            Ok(()) => Ok(TerminatedJob::new(
                JobReport {
                    warnings: self.error_catcher.warnings.clone(),
                    step: self.step.clone(),
                },
                Some(self.progress.bar.clone()),
            )),
            Err(err) => Err(JobError::Kill(format!("{:?}", &self.command), err.into())
                .as_failed_job(self.report(), Some(self.progress.bar.clone()))),
        };

        if result.is_ok() {
            self.cleanup_success()
                .try_catch(&mut self.error_catcher)
                .map_err(|err| FailedJob::new(err, self.report(), Some(self.progress.bar)))?;
        } else {
            // we only care about the first error
            let _ = self.cleanup_fail();
        }

        return result;
    }

    pub fn update_progress(mut self) -> Result<RunningJob, FailedJob> {
        self.progress
            .update(&self.step.log)
            .try_catch(&mut self.error_catcher)
            .map_err(|err| {
                // we only care about the first error
                let _ = self.cleanup_fail();
                FailedJob::new(err, self.report(), Some(self.progress.bar.clone()))
            })
            .map(|_| self)
    }

    pub fn report(&self) -> JobReport {
        JobReport {
            warnings: self.error_catcher.warnings.clone(),
            step: self.step.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct JobReport {
    warnings: Vec<JobError>,
    step: StepInfo,
}

#[derive(Clone, Debug, thiserror::Error, Diagnostic)]
#[error(
    "failure while executing `{name}`\n\
    {error}",
    name = report.step.name,
)]
#[diagnostic(help("check {log} or execute nixflow with `--inspect {name}` (if not done so already) to inspect the job output", name = report.step.name, log = report.step.log))]
pub struct FailedJob {
    error: JobError,
    report: JobReport,
    progress: Option<ProgressBar>,
}
impl FailedJob {
    pub fn new(error: JobError, report: JobReport, progress: Option<ProgressBar>) -> Self {
        Self {
            error,
            report,
            progress,
        }
    }

    fn cleanup(&self) {
        self.progress.as_ref().inspect(|bar| bar.abandon());
    }
}

#[derive(Debug)]
pub struct SuccessfulJob {
    report: JobReport,
    progress: Option<ProgressBar>,
}
impl SuccessfulJob {
    pub fn new(report: JobReport, progress: Option<ProgressBar>) -> Self {
        Self { report, progress }
    }

    fn cleanup(&self) {
        self.progress.as_ref().inspect(|bar| bar.finish());
    }
}

#[derive(Debug)]
pub struct TerminatedJob {
    report: JobReport,
    progress: Option<ProgressBar>,
}
impl TerminatedJob {
    fn new(report: JobReport, progress: Option<ProgressBar>) -> Self {
        Self { report, progress }
    }

    fn cleanup(&self) {
        self.progress.as_ref().inspect(|bar| bar.abandon());
    }
}

pub enum ExecutedJob {
    Running(RunningJob),
    Finished(SuccessfulJob),
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

#[derive(Clone, Debug, thiserror::Error)]
pub enum JobError {
    #[error("usage of the `{0}` executor which was not provided in the job specification")]
    UnprovidedExecutorUsage(ExecutionMethod),

    #[error("failed to check for the existence of {0}\n{1}")]
    InputExistenceCheck(PathBuf, IoError),

    #[error(
        "the following inputs do not exist:\n\t{}",
        input_paths.iter().map(|path| format!("`{path}`")).collect::<Vec<_>>().join("\n\t")
    )]
    InputExistence { input_paths: Vec<PathBuf> },

    #[error("failed to check for the existence of {0}\n{1}")]
    OutputExistenceCheck(PathBuf, IoError),

    #[error("failed to create the parent directory for the specified log file `{0}`\n{1}")]
    LogFileParentDirectoryCreation(PathBuf, IoError),

    #[error("failed to poll `{0}`\n{1}")]
    Wait(String, IoError),

    #[allow(unused)]
    #[error("failed to kill `{0}`\n{1}")]
    Kill(String, IoError),

    #[error("failed to execute `{0}`, terminated by a signal")]
    SignalTermination(String),

    #[error("failed to execute `{0}`, exit code {1} is non-zero")]
    NonZeroExitCode(String, i32),

    #[error("failed to read progress from `{0}`\n{1}")]
    ProgressLogRead(PathBuf, IoError),

    #[error("failed to read progress from `{0}`\n{1}")]
    ProgressScan(PathBuf, ProgressScanError),

    #[error("failed to setup progress scanning\n{0}")]
    ProgressScanSetup(ProgressScanError),

    #[error(
        "one or more parent jobs failed:\n\t{}",
        parents.into_iter().map(|step| step.name.as_str()).collect::<Vec<_>>().join("\n\t"))
    ]
    ParentsFailed { parents: Vec<StepInfo> },

    #[error("failed to read a line from stdout/stderr during job output inspection")]
    InspectionOutputRead(#[source] IoError),

    #[error("failed to print a line to stdout during job output inspection")]
    InspectionOutputPrint(#[source] IoError),

    #[error("failed to write a line to the job log during job output inspection")]
    InspectionOutputWriteToLog(#[source] IoError),

    #[error("failed to execute\n{0}")]
    JobExecution(
        #[source]
        #[from]
        JobExecutionError,
    ),
}

pub trait AsFailedJob {
    fn as_failed_job(self, report: JobReport, progress: Option<ProgressBar>) -> FailedJob;
}

impl AsFailedJob for JobError {
    fn as_failed_job(self, report: JobReport, progress: Option<ProgressBar>) -> FailedJob {
        FailedJob::new(self, report, progress)
    }
}
