use std::fmt::Write;

use console::style;

use crate::model::{
    environment::{DiagnosticSection, DiagnosticStatus, Diagnostics, ProviderStatus},
    operation::OperationPlan,
    system::OsInfo,
};
use crate::platform::command::ExecutionEvent;
use crate::providers::nvidia::upgrade::AvailableUpgrades;

pub fn operation_plan(plan: &OperationPlan) {
    print!("{}", format_operation_plan(plan));
}

pub fn format_operation_plan(plan: &OperationPlan) -> String {
    let mut rendered = String::new();
    let label_width = plan
        .details
        .iter()
        .map(|detail| detail.label.chars().count())
        .chain((!plan.devices.is_empty()).then_some("GPU(s)".len()))
        .max()
        .unwrap_or(0);

    writeln!(rendered).unwrap();
    writeln!(
        rendered,
        "  {}  {}",
        style("arc").cyan().bold(),
        style(&plan.title).bold()
    )
    .unwrap();
    writeln!(rendered, "  {}", style("─".repeat(52)).dim()).unwrap();

    if !plan.details.is_empty() || !plan.devices.is_empty() {
        writeln!(rendered, "\n  {}", section_label("Environment")).unwrap();
    }
    for detail in &plan.details {
        let padded_label = format!("{:<label_width$}", detail.label);
        writeln!(
            rendered,
            "  {}  {}",
            style(padded_label).dim(),
            detail.value
        )
        .unwrap();
    }
    if !plan.devices.is_empty() {
        let padded_label = format!("{:<label_width$}", "GPU(s)");
        let empty_label = " ".repeat(label_width);
        for (index, device) in plan.devices.iter().enumerate() {
            let label = if index == 0 {
                padded_label.as_str()
            } else {
                empty_label.as_str()
            };
            writeln!(
                rendered,
                "  {}  {} {}",
                style(label).dim(),
                style("◆").cyan(),
                format_args!("{} ({})", device.name, device.vendor)
            )
            .unwrap();
        }
    }

    let step_count = match plan.steps.len() {
        1 => "1 step".to_owned(),
        count => format!("{count} steps"),
    };
    writeln!(
        rendered,
        "\n  {}  {}",
        section_label("Changes"),
        style(step_count).dim()
    )
    .unwrap();
    if plan.steps.is_empty() {
        writeln!(
            rendered,
            "  {}  {}",
            style("✓").green().bold(),
            style("No changes required").green()
        )
        .unwrap();
    } else {
        for (index, step) in plan.steps.iter().enumerate() {
            writeln!(
                rendered,
                "  {}  {}",
                style(format!("{:02}", index + 1)).cyan().bold(),
                style(&step.description).bold()
            )
            .unwrap();
            writeln!(
                rendered,
                "      {} {}",
                style("$").dim(),
                style(step.command.display()).dim()
            )
            .unwrap();
        }
    }
    if !plan.confirmation_warning.is_empty() {
        writeln!(
            rendered,
            "\n  {}  {}\n",
            style("!").yellow().bold(),
            style(&plan.confirmation_warning).yellow()
        )
        .unwrap();
    } else {
        writeln!(rendered).unwrap();
    }

    rendered
}

pub fn operation_completed(plan: &OperationPlan) {
    println!(
        "\n  {}  {}",
        style("✓").green().bold(),
        style(&plan.completion_message).green().bold()
    );
    if let Some(message) = &plan.reboot_message {
        println!(
            "  {}  {}",
            style("↻").yellow().bold(),
            style(message).yellow()
        );
    }
    println!();
}

pub fn execution_event(event: ExecutionEvent<'_>) {
    match event {
        ExecutionEvent::Started { index, total, step } => {
            if index == 0 {
                println!("\n  {}\n", section_label("Applying changes"));
            }
            println!(
                "  {}  {}  {}",
                style("◆").cyan(),
                style(format!("{}/{}", index + 1, total)).cyan().bold(),
                style(&step.description).bold()
            );
        }
        ExecutionEvent::Completed { index, total, step } => println!(
            "  {}  {}  {}\n",
            style("✓").green().bold(),
            style(format!("{}/{}", index + 1, total)).dim(),
            style(format!("{} complete", step.description)).green()
        ),
        ExecutionEvent::Failed { index, total, step } => println!(
            "  {}  {}  {}\n",
            style("✗").red().bold(),
            style(format!("{}/{}", index + 1, total)).dim(),
            style(format!("{} failed", step.description)).red().bold()
        ),
    }
}

pub fn notice(message: &str) {
    println!("\n  {}  {}\n", style("•").cyan().bold(), message);
}

pub fn cancelled(action: &str) {
    println!(
        "\n  {}  {} cancelled. No changes were made.\n",
        style("○").yellow(),
        action
    );
}

fn section_label(label: &str) -> String {
    style(label.to_uppercase()).cyan().bold().to_string()
}

pub fn system_status(
    os: &OsInfo,
    providers: &[ProviderStatus],
    upgrades: Option<&AvailableUpgrades>,
) {
    println!("GPU Environment\n");
    println!("OS:\n{}", os.display_name());
    for status in providers {
        println!("\n{} GPU(s):", status.vendor);
        if status.devices.is_empty() {
            println!("Not detected");
        } else {
            for device in &status.devices {
                println!("{}", device.name);
            }
        }
        println!(
            "\n{} Driver package:\n{}",
            status.vendor,
            status.driver.description()
        );
        if let Some(version) = upgrades.and_then(|value| value.driver.as_deref()) {
            println!("Available: {version}");
        }
        println!(
            "\n{} Driver runtime:\n{}",
            status.vendor,
            status
                .driver_version
                .as_deref()
                .unwrap_or("Not loaded or not operational")
        );
        for toolkit in &status.toolkits {
            println!(
                "\n{}:\n{}\nPackages: {}\nManageable by arc: {}",
                toolkit.name,
                toolkit.version.as_deref().unwrap_or("version unknown"),
                toolkit.packages.join(", "),
                if toolkit.manageable { "yes" } else { "no" }
            );
            if let Some(version) = upgrades.and_then(|value| value.toolkit.as_deref()) {
                println!("Available compatible version: {version}");
            }
        }
        if status.toolkits.is_empty() {
            println!("\nSystem-managed CUDA Toolkit:\nNot installed");
        }
        if let Some(active) = &status.active_toolkit {
            println!(
                "\nActive nvcc (informational):\n{}\nPath: {}\nManaged by arc: no",
                active.version.as_deref().unwrap_or("version unknown"),
                active.executable_path.as_deref().unwrap_or("unknown")
            );
        } else {
            println!("\nActive nvcc (informational):\nNot found on PATH");
        }
    }
}

pub fn diagnostics(diagnostics: &Diagnostics) {
    print!("{}", format_diagnostics(diagnostics));
}

pub fn format_diagnostics(diagnostics: &Diagnostics) -> String {
    let mut rendered = String::new();
    writeln!(rendered, "{} Diagnostics", diagnostics.vendor).unwrap();
    for section in [
        DiagnosticSection::Hardware,
        DiagnosticSection::OperatingSystem,
        DiagnosticSection::Driver,
        DiagnosticSection::CudaToolkit,
    ] {
        writeln!(rendered, "\n{section}").unwrap();
        for check in diagnostics
            .checks
            .iter()
            .filter(|check| check.section == section)
        {
            writeln!(rendered, "{} {}", mark(check.status), check.name).unwrap();
            if check.status != DiagnosticStatus::Pass {
                if let Some(problem) = &check.problem {
                    writeln!(rendered, "  {problem}").unwrap();
                }
                for evidence in &check.evidence {
                    writeln!(rendered, "  Evidence: {evidence}").unwrap();
                }
            }
        }
    }
    if diagnostics.healthy() && diagnostics.fix_plan.causes.is_empty() {
        let has_warnings = diagnostics
            .checks
            .iter()
            .any(|check| check.status == DiagnosticStatus::Warning);
        writeln!(
            rendered,
            "\n{}",
            if has_warnings {
                "Completed with warnings"
            } else {
                "Healthy"
            }
        )
        .unwrap();
        return rendered;
    }
    writeln!(rendered, "\nActionable fix plan").unwrap();
    for (index, cause) in diagnostics.fix_plan.causes.iter().enumerate() {
        writeln!(
            rendered,
            "{}. Likely root cause: {} ({} confidence)",
            index + 1,
            cause.title,
            cause.confidence
        )
        .unwrap();
        for evidence in &cause.evidence {
            writeln!(rendered, "   Evidence: {evidence}").unwrap();
        }
    }
    for (index, fix) in diagnostics.fix_plan.fixes.iter().enumerate() {
        writeln!(rendered, "\n{}. {}", index + 1, fix.title).unwrap();
        for command in &fix.commands {
            writeln!(rendered, "   $ {}", command.display()).unwrap();
        }
        for step in &fix.manual_steps {
            writeln!(rendered, "   - {step}").unwrap();
        }
    }
    writeln!(
        rendered,
        "\nNo fixes were executed. After completing the plan, rerun `arc doctor`."
    )
    .unwrap();
    rendered
}

fn mark(status: DiagnosticStatus) -> &'static str {
    match status {
        DiagnosticStatus::Pass => "✓",
        DiagnosticStatus::Warning => "⚠",
        DiagnosticStatus::Error => "✗",
        DiagnosticStatus::Skipped => "↷ skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        command::CommandSpec,
        device::GpuVendor,
        environment::{
            DiagnosticCheck, DiagnosticId, DiagnosticSection, DiagnosticStatus, FixPlan,
        },
        operation::{OperationPlan, PlanDetail, PlanStep},
    };

    #[test]
    fn operation_plan_output_has_a_scannable_hierarchy() {
        let plan = OperationPlan {
            title: "NVIDIA Installation Plan".into(),
            details: vec![PlanDetail::new("OS", "Ubuntu 24.04")],
            devices: vec![],
            steps: vec![PlanStep::new(
                "Install the NVIDIA driver",
                CommandSpec::sudo("apt-get", ["install", "nvidia-driver"]),
            )],
            confirmation_warning: "No changes until confirmation.".into(),
            completion_message: "Installation completed.".into(),
            reboot_message: None,
        };

        let rendered = format_operation_plan(&plan);
        let output = console::strip_ansi_codes(&rendered);

        for expected in [
            "arc",
            "NVIDIA Installation Plan",
            "ENVIRONMENT",
            "Ubuntu 24.04",
            "CHANGES",
            "1 step",
            "01",
            "Install the NVIDIA driver",
            "$ sudo apt-get install nvidia-driver",
            "No changes until confirmation.",
        ] {
            assert!(
                output.contains(expected),
                "missing {expected:?} in {output}"
            );
        }
    }

    #[test]
    fn diagnostic_output_has_sections_marks_and_rerun_instruction() {
        let diagnostics = Diagnostics {
            vendor: GpuVendor::Nvidia,
            checks: vec![
                sample(DiagnosticSection::Hardware, DiagnosticStatus::Pass),
                sample(
                    DiagnosticSection::OperatingSystem,
                    DiagnosticStatus::Warning,
                ),
                sample(DiagnosticSection::Driver, DiagnosticStatus::Error),
                sample(DiagnosticSection::CudaToolkit, DiagnosticStatus::Skipped),
            ],
            fix_plan: FixPlan::default(),
        };
        let output = format_diagnostics(&diagnostics);
        for expected in [
            "Hardware",
            "Operating System",
            "Driver",
            "CUDA Toolkit",
            "✓",
            "⚠",
            "✗",
            "↷ skipped",
            "rerun `arc doctor`",
        ] {
            assert!(
                output.contains(expected),
                "missing {expected:?} in {output}"
            );
        }
    }

    fn sample(section: DiagnosticSection, status: DiagnosticStatus) -> DiagnosticCheck {
        DiagnosticCheck {
            id: DiagnosticId::NvidiaGpu,
            section,
            name: "check".into(),
            status,
            evidence: vec!["fact".into()],
            problem: Some("problem".into()),
            dependencies: vec![],
            recommended_fixes: vec![],
        }
    }
}
