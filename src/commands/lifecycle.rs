use anyhow::Result;

use crate::{model::operation::OperationPlan, platform::command, ui::output};

pub struct PlanLifecycle<'a> {
    pub dry_run: bool,
    pub assume_yes: bool,
    pub verbose: bool,
    pub show_commands: bool,
    pub display_noop_plan: bool,
    pub cancellation_label: &'a str,
}

/// Apply the shared plan lifecycle while allowing each command to retain its
/// existing confirmation prompt and no-op message.
pub fn run(
    mut plan: OperationPlan,
    options: PlanLifecycle<'_>,
    confirm: impl FnOnce() -> Result<bool>,
    noop: impl FnOnce(&OperationPlan),
) -> Result<()> {
    command::normalize_for_current_user(&mut plan);
    if options.display_noop_plan || !plan.is_noop() {
        output::operation_plan(&plan, options.show_commands);
    }
    if options.dry_run {
        output::notice("Dry run complete. No changes were made.");
        return Ok(());
    }
    if plan.is_noop() {
        noop(&plan);
        return Ok(());
    }
    if !options.assume_yes && !confirm()? {
        output::cancelled(options.cancellation_label);
        return Ok(());
    }
    let mut reporter = output::ExecutionReporter::new(&plan, options.verbose);
    let execution = command::execute_plan(
        &command::SystemCommandRunner,
        &plan,
        options.verbose,
        |event| reporter.report(event),
    )?;
    output::operation_completed(&plan);
    output::execution_log(execution.log_path.as_deref());
    Ok(())
}
