use anyhow::{Result, bail};

use crate::{
    model::operation::{NextStep, OperationPlan, PlanDetail, PlanStage, PlanStep},
    platform::package_manager,
    providers::nvidia::{driver::DriverFlavor, recipe, repository, toolkit},
};

use super::{InstallContext, InstallDecision, InstallOptions};

pub(super) fn plan(
    context: &InstallContext,
    decision: &InstallDecision,
    options: &InstallOptions,
) -> Result<OperationPlan> {
    let os = &context.os;
    let kernel = &context.kernel;
    let status = &context.status;
    let repository = &context.repository;
    let policy = decision.policy;
    let recipe = recipe::resolve(os, kernel, policy)?;
    let broken_managed_packages = decision.broken_managed_packages.as_deref();
    let current_flavor = decision.current_flavor;
    let toolkit_package = decision.toolkit_package.as_deref();
    let install_toolkit = decision.install_toolkit;
    let install_driver = decision.install_driver;
    let driver_pending_activation = decision.driver_pending_activation;
    let transition = decision.transition_driver;
    let driver_name = match policy.flavor {
        DriverFlavor::Open => "NVIDIA Open driver",
        DriverFlavor::Proprietary => "NVIDIA proprietary driver",
    };
    let repository_stage = PlanStage::new(
        "Configure the NVIDIA CUDA repository",
        "Configuring the NVIDIA repository...",
        "Configured the NVIDIA repository",
        "Could not configure the NVIDIA repository",
    );
    let refresh_stage = PlanStage::new(
        "Refresh package metadata",
        "Refreshing package metadata...",
        "Refreshed package metadata",
        "Could not refresh package metadata",
    );
    let prerequisites_stage = PlanStage::new(
        "Install driver prerequisites",
        "Installing driver prerequisites...",
        "Installed driver prerequisites",
        "Could not install driver prerequisites",
    );
    let driver_stage = PlanStage::new(
        format!("Install the {driver_name}"),
        format!("Installing the {driver_name}..."),
        format!("Installed the {driver_name}"),
        format!("Could not install the {driver_name}"),
    );
    let driver_verification_stage = PlanStage::new(
        "Verify the installation",
        "Verifying the installation...",
        "Verified the installation",
        "Could not verify the installation",
    );
    let toolkit_stage = PlanStage::new(
        "Install the CUDA Toolkit",
        "Installing the CUDA Toolkit...",
        "Installed the CUDA Toolkit",
        "Could not install the CUDA Toolkit",
    );
    let toolkit_verification_stage = PlanStage::new(
        "Verify the CUDA Toolkit",
        "Verifying the CUDA Toolkit...",
        "Verified the CUDA Toolkit",
        "Could not verify the CUDA Toolkit",
    );
    let mut steps = Vec::new();
    if install_driver || install_toolkit {
        if !context.repository_configured {
            if !context.repository_downloader_available {
                bail!(
                    "Configuring the NVIDIA repository requires curl or wget, but neither command is available."
                );
            }
            steps.extend(
                repository::setup_commands(os.package_manager(), repository)
                    .into_iter()
                    .map(|command| {
                        PlanStep::new("Configure the NVIDIA CUDA repository", command)
                            .in_stage(&repository_stage)
                    }),
            );
        }
        steps.push(
            PlanStep::new(
                "Refresh package metadata",
                package_manager::refresh_command(os.package_manager()),
            )
            .in_stage(&refresh_stage),
        );
    }
    if install_driver {
        steps.extend(recipe.prerequisites.into_iter().map(|command| {
            PlanStep::new("Ensure NVIDIA driver prerequisites", command)
                .in_stage(&prerequisites_stage)
        }));
        steps.push(
            PlanStep::new(
                "Refresh package metadata after ensuring prerequisites",
                package_manager::refresh_command(os.package_manager()),
            )
            .in_stage(&prerequisites_stage),
        );
        if let Some(packages) = broken_managed_packages {
            if let Some(command) =
                package_manager::reinstall_command(os.package_manager(), packages)
            {
                steps.push(
                    PlanStep::new("Reinstall the detected NVIDIA driver packages", command)
                        .in_stage(&driver_stage),
                );
            }
            if packages.iter().any(|package| package.contains("dkms")) {
                steps.push(
                    PlanStep::new(
                        "Rebuild the NVIDIA module for the running kernel",
                        crate::model::command::CommandSpec::sudo(
                            "dkms",
                            ["autoinstall", "-k", kernel],
                        ),
                    )
                    .in_stage(&driver_stage),
                );
            }
        }
        if let Some(from) = current_flavor {
            steps.extend(
                recipe::transition_commands(os, policy, from)
                    .into_iter()
                    .map(|command| {
                        PlanStep::new("Transition the NVIDIA driver package stream", command)
                            .in_stage(&driver_stage)
                    }),
            );
        } else {
            steps.extend(recipe.driver_preparation.into_iter().map(|command| {
                PlanStep::new("Select the NVIDIA driver package stream", command)
                    .in_stage(&driver_stage)
            }));
            steps.push(
                PlanStep::new(
                    format!(
                        "Verify NVIDIA driver package {} is available",
                        policy.flavor.package()
                    ),
                    package_manager::query_command(os.package_manager(), policy.flavor.package()),
                )
                .in_stage(&driver_stage),
            );
            steps.push(
                PlanStep::new("Install the NVIDIA driver", recipe.driver_install)
                    .in_stage(&driver_stage),
            );
        }
        steps.push(
            PlanStep::new(
                "Verify that NVIDIA kernel module metadata is installed",
                recipe.driver_verification,
            )
            .in_stage(&driver_verification_stage),
        );
    }
    if install_toolkit && let Some(package) = toolkit_package {
        steps.push(
            PlanStep::new(
                format!("Verify CUDA Toolkit package {package} is available"),
                package_manager::query_command(os.package_manager(), package),
            )
            .in_stage(&toolkit_stage),
        );
        steps.push(
            PlanStep::new(
                format!("Install {package}"),
                package_manager::install_command(os.package_manager(), package),
            )
            .in_stage(&toolkit_stage),
        );
        steps.push(
            PlanStep::new(
                "Verify the CUDA Toolkit with nvcc",
                toolkit::verification_command(),
            )
            .in_stage(&toolkit_verification_stage),
        );
    }
    let driver_detail = if driver_pending_activation {
        format!(
            "{} installed; kernel module is ready but not loaded — reboot required",
            policy.flavor.package()
        )
    } else if !install_driver {
        format!(
            "{} already managed correctly — skipped",
            policy.flavor.package()
        )
    } else if broken_managed_packages.is_some() {
        format!(
            "repair detected packages and ensure {} is installed",
            policy.flavor.package()
        )
    } else if transition {
        format!(
            "transition to {}{}",
            policy.flavor.package(),
            policy
                .branch
                .map(|b| format!(" pinned to R{b}"))
                .unwrap_or_default()
        )
    } else {
        format!(
            "install {}{}",
            policy.flavor.package(),
            policy
                .branch
                .map(|b| format!(" pinned to R{b}"))
                .unwrap_or_default()
        )
    };
    Ok(OperationPlan {
        title: "NVIDIA Installation Plan".into(),
        details: vec![
            PlanDetail::new("OS", os.display_name()),
            PlanDetail::new("Kernel", kernel),
            PlanDetail::new("Package manager", os.package_manager().to_string()),
            PlanDetail::new("Repository", repository.base_url.clone()),
            PlanDetail::new(
                "Release validation",
                format!(
                    "repository-compatible {}; NVIDIA validated: {}; arc tested: {}",
                    repository.family,
                    if repository.nvidia_validated {
                        "yes"
                    } else {
                        "no"
                    },
                    if repository.arc_tested { "yes" } else { "no" }
                ),
            ),
            PlanDetail::new("Profile", options.profile.plan_label()),
            PlanDetail::new("Existing driver", status.driver.description()),
            PlanDetail::new("Driver", driver_detail),
            PlanDetail::new(
                "CUDA Toolkit",
                toolkit_package.map_or("not requested".into(), |p| {
                    if install_toolkit {
                        format!("install {p}")
                    } else {
                        "requested version already installed — skipped".into()
                    }
                }),
            ),
            PlanDetail::new(
                "Kernel headers",
                if context.kernel_headers_available {
                    "detected for running kernel"
                } else {
                    "install exact matching prerequisites before driver"
                },
            ),
        ],
        devices: status.devices.clone(),
        steps,
        confirmation_warning: String::new(),
        completion_message: match (install_driver, install_toolkit) {
            (true, true) => format!("{driver_name} and CUDA Toolkit installed and verified."),
            (true, false) => format!("{driver_name} installed and verified."),
            (false, true) => "CUDA Toolkit installed and verified.".into(),
            (false, false) => "Requested NVIDIA components are already installed.".into(),
        },
        next_step: (driver_pending_activation || install_driver)
            .then_some(NextStep::LoadNvidiaDriver),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::profile::InstallProfile;
    use crate::model::{
        device::GpuVendor,
        environment::{
            DriverFlavorState, DriverInstallation, DriverPackageScope, ProviderStatus,
            ToolkitSource, ToolkitStatus,
        },
        system::{Distribution, OsInfo},
    };
    use crate::providers::nvidia::{
        driver::DriverPreference,
        gpu::{Generation, NvidiaGpu},
    };
    fn os() -> OsInfo {
        OsInfo {
            distribution: Distribution::Ubuntu,
            name: "Ubuntu".into(),
            version_id: "24.04".into(),
            architecture: "x86_64".into(),
            is_wsl: false,
        }
    }
    fn gpu(generation: Generation) -> NvidiaGpu {
        NvidiaGpu {
            name: "GPU".into(),
            pci_device_id: None,
            generation,
        }
    }
    fn status(driver: DriverInstallation, toolkit_version: Option<&str>) -> ProviderStatus {
        ProviderStatus {
            vendor: GpuVendor::Nvidia,
            devices: vec![],
            driver,
            driver_version: Some("580.65.06".into()),
            driver_runtime_operational: true,
            driver_runtime_state: crate::model::environment::DriverRuntimeState::Operational,
            dkms_status: None,
            driver_module: None,
            kernel_version: None,
            secure_boot_enabled: None,
            toolkits: toolkit_version
                .map(|v| ToolkitStatus {
                    name: "System-managed CUDA Toolkit".into(),
                    version: Some(v.into()),
                    executable_path: Some(format!("/usr/local/cuda-{v}/bin/nvcc")),
                    source: ToolkitSource::SystemPackageManager,
                    packages: vec![format!("cuda-toolkit-{}", v.replace('.', "-"))],
                    manageable: true,
                })
                .into_iter()
                .collect(),
            active_toolkit: None,
        }
    }
    fn managed(flavor: DriverFlavorState, branch: Option<u32>) -> DriverInstallation {
        DriverInstallation::Managed {
            flavor,
            scope: DriverPackageScope::Full,
            branch,
            packages: vec![],
        }
    }
    fn options(profile: InstallProfile, toolkit: Option<&str>) -> InstallOptions {
        InstallOptions {
            profile,
            toolkit_version: toolkit.map(Into::into),
            driver: DriverPreference::Auto,
        }
    }

    fn build_plan(
        options: &InstallOptions,
        kernel: &str,
        gpus: &[NvidiaGpu],
        status: &ProviderStatus,
        repository_configured: bool,
        toolkit_package_installed: bool,
    ) -> Result<OperationPlan> {
        let os = os();
        let context = InstallContext {
            os: os.clone(),
            kernel: kernel.into(),
            gpus: gpus.to_vec(),
            repository: repository::resolve(&os)?,
            repository_configured,
            repository_downloader_available: true,
            status: status.clone(),
            installed_packages: if toolkit_package_installed {
                super::super::inspect::requested_toolkit(options)?
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            },
            kernel_headers_available: true,
        };
        let decision = InstallDecision::decide(&context, options)?;
        plan(&context, &decision, options)
    }

    #[test]
    fn matching_modern_install_is_noop() {
        let plan = build_plan(
            &options(InstallProfile::CudaDevelopment, Some("13.1")),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &status(managed(DriverFlavorState::Open, None), Some("13.1")),
            true,
            true,
        )
        .unwrap();
        assert!(plan.is_noop());
    }

    #[test]
    fn installed_module_waiting_for_reboot_is_a_noop_with_guidance() {
        let mut current = status(managed(DriverFlavorState::Open, None), None);
        current.driver_version = None;
        let plan = build_plan(
            &options(InstallProfile::ModelTraining, None),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &current,
            true,
            false,
        )
        .unwrap();

        assert!(plan.is_noop());
        assert_eq!(plan.next_step, Some(NextStep::LoadNvidiaDriver));
    }

    #[test]
    fn broken_managed_driver_gets_an_executable_repair_plan() {
        let mut current = status(
            DriverInstallation::BrokenManaged {
                flavor: DriverFlavorState::Open,
                packages: vec!["nvidia-open".into(), "nvidia-dkms-610-open".into()],
            },
            None,
        );
        current.driver_version = None;
        let plan = build_plan(
            &options(InstallProfile::ModelTraining, None),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &current,
            true,
            false,
        )
        .unwrap();
        let commands = plan
            .steps
            .iter()
            .map(|step| step.command.display())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            commands.contains("apt-get install --reinstall -y nvidia-open nvidia-dkms-610-open")
        );
        assert!(commands.contains("dkms autoinstall -k 6.8.0-generic"));
        assert!(commands.contains("apt-get install -y nvidia-open"));
        assert!(commands.contains("modinfo nvidia"));
        assert!(plan.next_step.is_some());
    }

    #[test]
    fn low_level_driver_commands_are_grouped_under_past_tense_ui_stages() {
        let plan = build_plan(
            &options(InstallProfile::ModelTraining, None),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &status(DriverInstallation::Missing, None),
            true,
            false,
        )
        .unwrap();
        let titles = plan
            .steps
            .iter()
            .enumerate()
            .filter_map(|(index, step)| {
                let (_, first, _) = plan.stage_position(index);
                first.then_some(step.stage.title.as_str())
            })
            .collect::<Vec<_>>();

        assert_eq!(
            titles,
            [
                "Refresh package metadata",
                "Install driver prerequisites",
                "Install the NVIDIA Open driver",
                "Verify the installation",
            ]
        );
        assert!(plan.steps.len() > plan.stage_count());
        assert_eq!(plan.steps[0].stage.success, "Refreshed package metadata");
        assert!(
            plan.steps
                .iter()
                .all(|step| !step.stage.success.ends_with(" complete"))
        );
    }

    #[test]
    fn custom_active_nvcc_does_not_suppress_system_toolkit_install() {
        let mut current = status(managed(DriverFlavorState::Open, None), None);
        current.active_toolkit = Some(ToolkitStatus {
            name: "Active nvcc".into(),
            version: Some("13.1".into()),
            executable_path: Some("/opt/conda/envs/cuda/bin/nvcc".into()),
            source: ToolkitSource::ActivePath,
            packages: vec![],
            manageable: false,
        });
        let plan = build_plan(
            &options(InstallProfile::CudaDevelopment, Some("13.1")),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &current,
            true,
            false,
        )
        .unwrap();
        assert!(plan.steps.iter().any(|step| {
            step.command
                .display()
                .contains("apt-get install -y cuda-toolkit-13-1")
        }));
    }
    #[test]
    fn legacy_plan_pins_r580_and_rejects_cuda_13() {
        let plan = build_plan(
            &options(InstallProfile::CudaDevelopment, Some("12.8")),
            "6.8.0-generic",
            &[gpu(Generation::MaxwellPascalVolta)],
            &status(DriverInstallation::Missing, None),
            true,
            false,
        )
        .unwrap();
        assert!(
            plan.steps
                .iter()
                .any(|s| s.command.display().contains("nvidia-driver-pinning-580"))
        );
        assert!(
            build_plan(
                &options(InstallProfile::CudaDevelopment, Some("13.1")),
                "6.8.0-generic",
                &[gpu(Generation::MaxwellPascalVolta)],
                &status(DriverInstallation::Missing, None),
                true,
                false
            )
            .is_err()
        );
    }
    #[test]
    fn refuses_working_unmanaged_driver() {
        let result = build_plan(
            &options(InstallProfile::ModelTraining, None),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &status(
                DriverInstallation::Unmanaged {
                    working: true,
                    evidence: vec![
                        crate::model::environment::UnmanagedDriverEvidence::RunfileUninstaller,
                    ],
                },
                None,
            ),
            true,
            false,
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("will not install repository packages")
        );
    }

    #[test]
    fn incompatible_managed_driver_is_upgraded_before_toolkit_install() {
        let mut current = status(managed(DriverFlavorState::Open, None), None);
        current.driver_version = Some("570.26".into());
        let plan = build_plan(
            &options(InstallProfile::CudaDevelopment, Some("13.3")),
            "6.8.0-generic",
            &[gpu(Generation::TuringOrNewer)],
            &current,
            true,
            false,
        )
        .unwrap();
        let commands = plan
            .steps
            .iter()
            .map(|s| s.command.display())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(commands.contains("apt-get install -y nvidia-open"));
        assert!(
            commands.find("nvidia-open").unwrap() < commands.find("cuda-toolkit-13-3").unwrap()
        );
    }
}
