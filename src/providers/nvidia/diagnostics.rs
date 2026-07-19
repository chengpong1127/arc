use anyhow::Result;

use crate::{
    model::{
        device::GpuVendor,
        environment::{DiagnosticCheck, Diagnostics},
    },
    platform::{os, package_manager},
};

use super::{driver, gpu};

pub fn detect() -> Result<Diagnostics> {
    let gpu_detected = !gpu::detect()?.is_empty();
    let manager = os::detect()?.package_manager();
    let driver_installed = package_manager::is_installed(manager, "nvidia-open")?
        || package_manager::is_installed(manager, "cuda-drivers")?;
    let driver_operational = driver::detect_version()?.is_some();
    let nvidia_smi_available = driver::nvidia_smi_available();
    let kernel_headers_available = driver::kernel_headers_available();
    let secure_boot_enabled = driver::secure_boot_enabled();
    Ok(Diagnostics {
        vendor: GpuVendor::Nvidia,
        checks: vec![
            DiagnosticCheck {
                name: "NVIDIA GPU detected".into(),
                passed: gpu_detected,
                problem: (!gpu_detected)
                    .then(|| "No NVIDIA GPU was detected by lspci or sysfs.".into()),
            },
            DiagnosticCheck {
                name: "NVIDIA driver installed".into(),
                passed: driver_installed,
                problem: (!driver_installed)
                    .then(|| "No NVIDIA driver meta-package is installed.".into()),
            },
            DiagnosticCheck {
                name: "NVIDIA driver loaded and operational".into(),
                passed: driver_operational,
                problem: (!driver_operational).then(|| {
                    if driver_installed {
                        "The driver package is installed, but the kernel module is not operational. Reboot, then check Secure Boot and kernel headers if the problem remains.".into()
                    } else {
                        "Install the NVIDIA driver first.".into()
                    }
                }),
            },
            DiagnosticCheck {
                name: "nvidia-smi available".into(),
                passed: nvidia_smi_available,
                problem: (!nvidia_smi_available)
                    .then(|| "nvidia-smi is not available in PATH.".into()),
            },
            DiagnosticCheck {
                name: "Kernel headers available for the running kernel".into(),
                passed: kernel_headers_available,
                problem: (!kernel_headers_available).then(|| {
                    "Matching kernel headers were not found under /lib/modules/$(uname -r)/build.".into()
                }),
            },
            DiagnosticCheck {
                name: "Secure Boot permits the NVIDIA driver".into(),
                passed: secure_boot_enabled != Some(true) || driver_operational,
                problem: (secure_boot_enabled == Some(true) && !driver_operational).then(|| {
                    "Secure Boot is enabled and the NVIDIA driver is not operational; module signing or MOK enrollment may be required.".into()
                }),
            },
        ],
    })
}
