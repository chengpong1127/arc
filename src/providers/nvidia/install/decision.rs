use anyhow::{Result, bail};

use crate::model::environment::{DriverFlavorState, DriverInstallation};

use super::{InstallContext, InstallOptions, InstallProfile, inspect};
use crate::providers::nvidia::{
    compatibility::{self, Compatibility},
    driver::DriverFlavor,
    policy::{self, DriverPolicy},
    toolkit,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallDecision {
    pub policy: DriverPolicy,
    pub current_flavor: Option<DriverFlavor>,
    pub broken_managed_packages: Option<Vec<String>>,
    pub toolkit_package: Option<String>,
    pub install_driver: bool,
    pub install_toolkit: bool,
    pub driver_pending_activation: bool,
    pub transition_driver: bool,
}

impl InstallDecision {
    pub fn decide(context: &InstallContext, options: &InstallOptions) -> Result<Self> {
        let cuda_development = options.profile == InstallProfile::CudaDevelopment;
        let policy = policy::resolve(
            &context.os,
            &context.gpus,
            options.driver,
            options.toolkit_version.as_deref(),
            cuda_development,
        )?;
        let status = &context.status;
        if let DriverInstallation::Unmanaged { working, evidence } = &status.driver {
            bail!(
                "A{} unmanaged NVIDIA driver installation was detected (evidence: {}). arc will not install repository packages over it. Remove it with its original installer or migrate it to distribution packages first.",
                if *working { " working" } else { " broken" },
                evidence
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        if status.driver.flavor() == Some(DriverFlavorState::Mixed) {
            bail!(
                "Conflicting open and proprietary NVIDIA packages are installed. Repair or remove the mixed package installation before using arc install."
            );
        }
        let broken_managed_packages = match &status.driver {
            DriverInstallation::BrokenManaged { packages, .. } => Some(packages.clone()),
            _ => None,
        };
        let target = match policy.flavor {
            DriverFlavor::Open => DriverFlavorState::Open,
            DriverFlavor::Proprietary => DriverFlavorState::Proprietary,
        };
        let current_flavor = status.driver.flavor().and_then(|value| match value {
            DriverFlavorState::Open => Some(DriverFlavor::Open),
            DriverFlavorState::Proprietary => Some(DriverFlavor::Proprietary),
            DriverFlavorState::Mixed => None,
        });
        let branch_matches = policy.branch.is_none()
            || matches!(&status.driver, DriverInstallation::Managed { branch: Some(branch), .. } if Some(*branch) == policy.branch);
        let requested_toolkit_version = cuda_development.then(|| {
            options
                .toolkit_version
                .as_deref()
                .unwrap_or(toolkit::LATEST_TOOLKIT_VERSION)
        });
        let driver_compatible = status
            .driver_version
            .as_deref()
            .zip(requested_toolkit_version)
            .and_then(|(driver, toolkit)| compatibility::evaluate(driver, toolkit))
            != Some(Compatibility::Incompatible);
        let driver_correct = matches!(status.driver, DriverInstallation::Managed { .. })
            && status.driver.flavor() == Some(target)
            && branch_matches
            && driver_compatible;
        let driver_pending_activation = driver_correct && status.driver_version.is_none();
        let branch_transition = policy.branch.is_some()
            && matches!(&status.driver, DriverInstallation::Managed { branch, .. } if *branch != policy.branch);
        let transition_driver =
            current_flavor.is_some_and(|from| from != policy.flavor) || branch_transition;
        let toolkit_package = inspect::requested_toolkit(options)?;
        let current_toolkit = status
            .toolkits
            .first()
            .and_then(|value| value.version.as_deref());
        let install_toolkit = toolkit_package.as_deref().is_some_and(|package| {
            toolkit_install_needed(
                package,
                current_toolkit,
                context
                    .installed_packages
                    .iter()
                    .any(|installed| installed == package),
            )
        });
        Ok(Self {
            policy,
            current_flavor,
            broken_managed_packages,
            toolkit_package,
            install_driver: !driver_correct,
            install_toolkit,
            driver_pending_activation,
            transition_driver,
        })
    }
}

fn toolkit_install_needed(package: &str, current_version: Option<&str>, installed: bool) -> bool {
    if installed {
        return false;
    }
    if package == "cuda-toolkit" {
        return true;
    }
    current_version
        .and_then(|version| toolkit::versioned_package(version).ok())
        .is_none_or(|current| current != package)
}
