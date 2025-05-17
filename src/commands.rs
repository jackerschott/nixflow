use std::process::Command;

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
