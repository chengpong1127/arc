use anyhow::Result;

use crate::{
    cli::UninstallArgs,
    platform::os,
    providers::{
        AcceleratorProvider,
        nvidia::{NvidiaProvider, uninstall},
    },
    ui::{output, prompt},
};

use super::lifecycle::{self, PlanLifecycle};

pub fn run(args: UninstallArgs, verbose: bool, show_commands: bool) -> Result<()> {
    let system = os::detect()?;
    let status = NvidiaProvider.inspect()?;
    let plan = uninstall::plan(&system, &status)?;
    lifecycle::run(
        plan,
        PlanLifecycle {
            dry_run: false,
            assume_yes: args.yes,
            verbose,
            show_commands,
            display_noop_plan: false,
            cancellation_label: "Uninstall",
        },
        prompt::confirm_uninstall,
        |_| output::notice("No installed CUDA Toolkit or NVIDIA driver was detected."),
    )
}
