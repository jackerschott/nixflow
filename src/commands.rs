use std::process::{Command, ExitStatus, Output};

use crate::utils::IoError;

pub fn clone_command(cmd: &Command) -> Command {
    let mut cmd_clone = Command::new(cmd.get_program());
    cmd_clone.args(cmd.get_args());

    for (k, v) in cmd.get_envs() {
        match v {
            Some(v) => cmd_clone.env(k, v),
            None => cmd_clone.env_remove(k),
        };
    }

    if let Some(current_dir) = cmd.get_current_dir() {
        cmd_clone.current_dir(current_dir);
    }

    cmd_clone
}

pub fn shell_command(command: &Command) -> String {
    let variable_settings = command
        .get_envs()
        .map(|(name, value)| match value {
            Some(value) => format!(
                "'{name}={value}'",
                name = name.to_string_lossy().to_string(),
                value = value.to_string_lossy().to_string()
            ),
            None => format!("-u '{name}'", name = name.to_string_lossy().to_string()),
        })
        .collect::<Vec<_>>()
        .join(" ");

    let shell_command = Iterator::chain(
        std::iter::once(format!(
            "'{}'",
            command.get_program().to_string_lossy().to_string()
        )),
        command
            .get_args()
            .map(|arg| format!("'{}'", arg.to_string_lossy().to_string())),
    )
    .collect::<Vec<_>>()
    .join(" ");

    return format!("env {variable_settings} {shell_command}");
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum CommandError {
    #[error("failed to execute `{command}`\n{io_error}")]
    Io {
        command: String,
        io_error: IoError,
    },

    #[error("failed to execute `{command}`, exit code {code} is non-zero")]
    NonZeroExitCode {
        command: String,
        code: i32,
        output: Option<String>,
    },

    #[error("failed to execute `{command}`, terminated by a signal")]
    SignalTermination {
        command: String,
        output: Option<String>,
    },
}
impl CommandError {
    pub fn new_io<E: Into<IoError>>(command: &Command, io_error: E) -> Self {
        Self::Io {
            command: format!("{command:?}"),
            io_error: io_error.into(),
        }
    }

    pub fn new_non_zero_exit_code(command: &Command, code: i32) -> Self {
        Self::NonZeroExitCode {
            command: format!("{command:?}"),
            code,
            output: None,
        }
    }

    pub fn new_piped_non_zero_exit_code<S: Into<String>>(
        command: &Command,
        code: i32,
        output: S,
    ) -> Self {
        Self::NonZeroExitCode {
            command: format!("{command:?}"),
            code,
            output: Some(output.into()),
        }
    }

    pub fn new_signal_termination(command: &Command) -> Self {
        Self::SignalTermination {
            command: format!("{command:?}"),
            output: None,
        }
    }

    pub fn new_piped_signal_termination<S: Into<String>>(command: &Command, output: S) -> Self {
        Self::SignalTermination {
            command: format!("{command:?}"),
            output: Some(output.into()),
        }
    }
}

pub trait AsCommandError {
    fn as_command_result(&self, command: &Command) -> Result<(), CommandError>;
    fn as_piped_command_result(
        &self,
        command: &Command,
        stdout: &str,
        stderr: &str,
    ) -> Result<(), CommandError>;
}
impl AsCommandError for ExitStatus {
    fn as_command_result(&self, command: &Command) -> Result<(), CommandError> {
        match self.code() {
            Some(0) => Ok(()),
            Some(code) => Err(CommandError::new_non_zero_exit_code(command, code)),
            None => Err(CommandError::new_signal_termination(command)),
        }
    }

    fn as_piped_command_result(
        &self,
        command: &Command,
        stdout: &str,
        stderr: &str,
    ) -> Result<(), CommandError> {
        if let Some(0) = self.code() {
            return Ok(());
        }

        let (stdout, stderr) = (stdout.trim(), stderr.trim());
        let mut combined_output = String::new();
        if !stdout.is_empty() {
            combined_output += &format!("--- stdout ---\n{stdout}");
        }
        if !stderr.is_empty() {
            combined_output += &format!("--- stderr ---\n{stderr}")
        }

        match self.code() {
            Some(code) => Err(CommandError::new_piped_non_zero_exit_code(
                command,
                code,
                combined_output,
            )),
            None => Err(CommandError::new_piped_signal_termination(
                command,
                combined_output,
            )),
        }
    }
}

pub struct OutputUtf8 {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl From<Output> for OutputUtf8 {
    fn from(value: Output) -> Self {
        Self {
            status: value.status,
            stdout: String::from_utf8(value.stdout)
                .expect("only used for output of commands that always print valid utf8"),
            stderr: String::from_utf8(value.stderr)
                .expect("only used for output of commands that always print valid utf8"),
        }
    }
}
