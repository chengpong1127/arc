use anyhow::Result;

use crate::{
    model::{environment::ProviderStatus, system::OsInfo},
    platform::{command::SystemCommandRunner, kernel, package_manager},
};

use super::InstallOptions;
use crate::providers::nvidia::{gpu, repository, state, toolkit};

#[derive(Clone, Debug)]
pub struct InstallContext {
    pub os: OsInfo,
    pub kernel: String,
    pub gpus: Vec<gpu::NvidiaGpu>,
    pub repository: repository::NvidiaRepository,
    pub repository_configured: bool,
    pub repository_downloader_available: bool,
    pub installed_packages: Vec<String>,
    pub status: ProviderStatus,
    pub kernel_headers_available: bool,
}

impl InstallContext {
    pub fn inspect(os: &OsInfo) -> Result<Self> {
        os.ensure_driver_installable("NVIDIA")?;
        let runner = SystemCommandRunner;
        let kernel = kernel::release_with(&runner)?;
        let gpus = gpu::detect()?;
        let installed_packages =
            package_manager::installed_packages_with(&runner, os.package_manager())?;
        let status = state::inspect_with(&runner, gpus.clone(), installed_packages.clone())?;
        let repository = repository::resolve(os)?;
        let repository_configured = repository::is_configured(os, &repository)?;
        Ok(Self {
            os: os.clone(),
            kernel,
            gpus,
            repository,
            repository_configured,
            repository_downloader_available: repository::downloader_available(),
            installed_packages,
            status,
            kernel_headers_available: crate::providers::nvidia::driver::kernel_headers_available(),
        })
    }
}

pub(super) fn requested_toolkit(options: &InstallOptions) -> Result<Option<String>> {
    match options.profile {
        super::InstallProfile::ModelTraining => Ok(None),
        super::InstallProfile::CudaDevelopment => {
            toolkit::package(options.toolkit_version.as_deref()).map(Some)
        }
    }
}
