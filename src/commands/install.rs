use anyhow::{Result, bail};

use crate::{
    cli::{DriverMode, InstallArgs, UsageProfile},
    platform::os,
    providers::nvidia::{
        driver::DriverPreference,
        install::{self, InstallOptions},
    },
    ui::{output, prompt},
};

use super::lifecycle::{self, PlanLifecycle};

pub fn run(args: InstallArgs, verbose: bool, show_commands: bool) -> Result<()> {
    let profile = resolve_profile(args.profile, args.toolkit.as_deref())?;
    let options = InstallOptions {
        profile,
        toolkit_version: args.toolkit.clone(),
        driver: match args.driver {
            DriverMode::Auto => DriverPreference::Auto,
            DriverMode::Open => DriverPreference::Open,
            DriverMode::Proprietary => DriverPreference::Proprietary,
        },
    };
    let plan = install::plan(&os::detect()?, &options)?;
    lifecycle::run(
        plan,
        PlanLifecycle {
            dry_run: args.dry_run,
            assume_yes: args.yes,
            verbose,
            show_commands,
            display_noop_plan: true,
            cancellation_label: "Installation",
        },
        prompt::confirm_install,
        |plan| {
            if plan.next_step.is_some() {
                output::operation_completed(plan);
            } else {
                output::notice("Requested components are already installed. No changes were made.");
            }
        },
    )
}

fn resolve_profile(profile: Option<UsageProfile>, toolkit: Option<&str>) -> Result<UsageProfile> {
    match (profile, toolkit) {
        (Some(UsageProfile::ModelTraining), Some(_)) => {
            bail!("--toolkit cannot be used with --profile model-training; choose cuda-development")
        }
        (Some(profile), _) => Ok(profile),
        (None, Some(_)) => Ok(UsageProfile::CudaDevelopment),
        (None, None) => prompt::select_usage_profile(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolkit_option_selects_cuda_development() {
        assert_eq!(
            resolve_profile(None, Some("13.1")).unwrap(),
            UsageProfile::CudaDevelopment
        );
        assert!(resolve_profile(Some(UsageProfile::ModelTraining), Some("13.1")).is_err());
    }
}
