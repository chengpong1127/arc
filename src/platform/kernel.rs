use anyhow::{Context, Result, ensure};

use crate::{
    model::command::CommandSpec,
    platform::command::{self, CommandRunner},
};

pub fn release_with(runner: &impl CommandRunner) -> Result<String> {
    let output = command::capture(runner, CommandSpec::new("uname", ["-r"]))
        .context("could not determine running kernel")?;
    ensure!(output.success, "uname -r failed");
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
