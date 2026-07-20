use anyhow::Result;

use crate::{
    cli::UpgradeArgs,
    platform::os,
    providers::nvidia::upgrade::{self, UpgradeOptions},
    ui::{output, prompt},
};

use super::lifecycle::{self, PlanLifecycle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeOutcome {
    Success,
    Unavailable,
}

pub fn run(args: UpgradeArgs, verbose: bool, show_commands: bool) -> Result<UpgradeOutcome> {
    let options = UpgradeOptions::from_component_flags(args.driver, args.toolkit);
    let plan = match upgrade::plan(&os::detect()?, &options) {
        Ok(plan) => plan,
        Err(error) if upgrade::is_actionable(&error) => {
            output::unavailable(&format!(
                "Upgrade unavailable: {}",
                upgrade::actionable_message(&error)
            ));
            return Ok(UpgradeOutcome::Unavailable);
        }
        Err(error) => return Err(error),
    };
    lifecycle::run(
        plan,
        PlanLifecycle {
            dry_run: args.dry_run,
            assume_yes: args.yes,
            verbose,
            show_commands,
            display_noop_plan: true,
            cancellation_label: "Upgrade",
        },
        prompt::confirm_upgrade,
        |_| {
            output::notice(
                "No selected installed component has a compatible upgrade. No changes were made.",
            )
        },
    )?;
    Ok(UpgradeOutcome::Success)
}
