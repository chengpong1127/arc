use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{
    model::{
        environment::ProviderStatus,
        operation::{OperationPlan, PlanDetail, PlanStep},
        system::{Distribution, OsInfo},
    },
    platform::package_manager,
};

const CUDA_TOOLKIT_PACKAGES: &[&str] = &["cuda-toolkit", "cuda-toolkit-*-*"];
const NVIDIA_DRIVER_PACKAGES: &[&str] = &["cuda-drivers*", "nvidia-open*"];

pub fn plan(os: &OsInfo, status: &ProviderStatus) -> Result<OperationPlan> {
    if os.distribution != Distribution::Ubuntu {
        bail!(
            "cudaenv uninstall supports NVIDIA packages on Ubuntu only (detected {}).",
            os.display_name()
        );
    }
    let toolkit_packages = installed_apt_packages(CUDA_TOOLKIT_PACKAGES)?;
    let driver_packages = installed_apt_packages(NVIDIA_DRIVER_PACKAGES)?;
    Ok(build_plan(status, toolkit_packages, driver_packages))
}

fn build_plan(
    status: &ProviderStatus,
    toolkit_packages: Vec<String>,
    driver_packages: Vec<String>,
) -> OperationPlan {
    let toolkit_installed = !toolkit_packages.is_empty();
    let driver_installed = !driver_packages.is_empty();
    let mut steps = Vec::new();
    if toolkit_installed {
        steps.push(PlanStep::new(
            "Remove the listed CUDA Toolkit packages",
            package_manager::apt_remove_command(
                &["remove", "--purge", "--yes"],
                &toolkit_packages
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
        ));
    }
    if driver_installed {
        steps.push(PlanStep::new(
            "Remove the listed NVIDIA driver packages",
            package_manager::apt_remove_command(
                &["remove", "--purge", "-V", "--yes"],
                &driver_packages
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
        ));
    }
    OperationPlan {
        title: "NVIDIA Uninstall Plan".into(),
        details: vec![
            PlanDetail::new(
                "Driver",
                if driver_installed {
                    "remove"
                } else {
                    "not detected"
                },
            ),
            PlanDetail::new(
                "CUDA Toolkit",
                if toolkit_installed {
                    "remove"
                } else {
                    "not detected"
                },
            ),
            PlanDetail::new(
                "Exact packages",
                toolkit_packages
                    .iter()
                    .chain(&driver_packages)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ],
        devices: status.devices.clone(),
        steps,
        confirmation_warning:
            "Only the exact packages listed above will be removed. Dependencies are retained."
                .into(),
        completion_message: "Detected CUDA/NVIDIA components were removed.".into(),
        reboot_message: driver_installed
            .then(|| "Reboot Ubuntu before installing another driver.".into()),
    }
}

fn installed_apt_packages(patterns: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${db:Status-Abbrev}\t${binary:Package}\\n"])
        .args(patterns)
        .output()
        .context("could not inspect installed CUDA/NVIDIA packages")?;
    let mut packages = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (status, package) = line.split_once('\t')?;
            (status.starts_with("ii ")
                && patterns
                    .iter()
                    .any(|pattern| package_matches(pattern, package)))
            .then(|| package.to_owned())
        })
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    Ok(packages)
}

fn package_matches(pattern: &str, package: &str) -> bool {
    let package = package.split(':').next().unwrap_or(package);
    match pattern {
        "cuda-toolkit" => package == "cuda-toolkit",
        "cuda-toolkit-*-*" => numeric_suffix(package, "cuda-toolkit-", 2),
        "cuda-drivers*" => package == "cuda-drivers" || numeric_suffix(package, "cuda-drivers-", 1),
        "nvidia-open*" => package == "nvidia-open" || numeric_suffix(package, "nvidia-open-", 1),
        _ => false,
    }
}

fn numeric_suffix(package: &str, prefix: &str, parts: usize) -> bool {
    let Some(suffix) = package.strip_prefix(prefix) else {
        return false;
    };
    let values = suffix.split('-').collect::<Vec<_>>();
    values.len() == parts
        && values
            .iter()
            .all(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{device::GpuVendor, environment::ToolkitStatus};

    #[test]
    fn plan_uses_same_typed_commands_that_will_be_executed() {
        let status = ProviderStatus {
            vendor: GpuVendor::Nvidia,
            devices: vec![],
            driver_installed: true,
            driver_version: Some("570".into()),
            toolkits: vec![ToolkitStatus {
                name: "CUDA Toolkit".into(),
                version: "13.1".into(),
            }],
        };
        let plan = build_plan(
            &status,
            vec!["cuda-toolkit-13-1".into()],
            vec!["nvidia-open".into()],
        );
        assert_eq!(plan.steps.len(), 2);
        assert!(
            plan.steps[0]
                .command
                .display()
                .contains("cuda-toolkit-13-1")
        );
        assert!(plan.steps[1].command.display().contains("nvidia-open"));
    }

    #[test]
    fn plan_only_removes_detected_components() {
        let status = ProviderStatus {
            vendor: GpuVendor::Nvidia,
            devices: vec![],
            driver_installed: false,
            driver_version: None,
            toolkits: vec![ToolkitStatus {
                name: "CUDA Toolkit".into(),
                version: "13.1".into(),
            }],
        };
        let plan = build_plan(&status, vec!["cuda-toolkit-13-1".into()], vec![]);
        let commands = plan
            .steps
            .iter()
            .map(|step| step.command.display())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(commands.contains("cuda-toolkit-13-1"));
        assert!(!commands.contains("nvidia-open"));
    }

    #[test]
    fn removal_patterns_only_accept_meta_packages() {
        assert!(package_matches("cuda-toolkit-*-*", "cuda-toolkit-13-1"));
        assert!(package_matches("cuda-drivers*", "cuda-drivers-610"));
        assert!(package_matches("nvidia-open*", "nvidia-open"));
        assert!(!package_matches(
            "cuda-toolkit-*-*",
            "cuda-toolkit-13-config-common"
        ));
        assert!(!package_matches("nvidia-open*", "nvidia-open-dkms"));
    }
}
