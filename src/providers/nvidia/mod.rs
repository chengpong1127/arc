use anyhow::Result;

use crate::{
    model::{
        device::GpuVendor,
        environment::{Diagnostics, ProviderStatus, ToolkitStatus},
    },
    platform::{os, package_manager},
    providers::AcceleratorProvider,
};

pub mod diagnostics;
pub mod driver;
pub mod gpu;
pub mod install;
pub mod repository;
pub mod toolkit;
pub mod uninstall;

pub struct NvidiaProvider;

impl AcceleratorProvider for NvidiaProvider {
    fn vendor(&self) -> GpuVendor {
        GpuVendor::Nvidia
    }

    fn inspect(&self) -> Result<ProviderStatus> {
        let devices = gpu::detect()?;
        let driver_version = driver::detect_version()?;
        let manager = os::detect()?.package_manager();
        let driver_installed = package_manager::is_installed(manager, "nvidia-open")?
            || package_manager::is_installed(manager, "cuda-drivers")?;
        let toolkits = toolkit::detect_version()?
            .map(|version| ToolkitStatus {
                name: "CUDA Toolkit".to_owned(),
                version,
            })
            .into_iter()
            .collect();

        Ok(ProviderStatus {
            vendor: self.vendor(),
            devices: devices.into_iter().map(Into::into).collect(),
            driver_installed,
            driver_version,
            toolkits,
        })
    }

    fn diagnose(&self) -> Result<Diagnostics> {
        diagnostics::detect()
    }
}
