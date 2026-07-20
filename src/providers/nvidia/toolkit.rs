use std::{env, path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};

use crate::model::{
    command::CommandSpec,
    environment::{ToolkitSource, ToolkitStatus},
};

const LATEST_TOOLKIT_PACKAGE: &str = "cuda-toolkit";
pub const LATEST_TOOLKIT_VERSION: &str = "13.3";

pub fn package(version: Option<&str>) -> Result<String> {
    match version {
        Some(version) => versioned_package(version),
        None => Ok(LATEST_TOOLKIT_PACKAGE.to_owned()),
    }
}

pub fn detect_active() -> Result<Option<ToolkitStatus>> {
    let Some(nvcc) = find_active_nvcc() else {
        return Ok(None);
    };
    let output = match Command::new(&nvcc).arg("--version").output() {
        Ok(output) => output,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to run {}", nvcc.display()));
        }
    };
    let version = output
        .status
        .success()
        .then(|| parse_nvcc_version(&String::from_utf8_lossy(&output.stdout)).map(str::to_owned))
        .flatten();
    Ok(Some(ToolkitStatus {
        name: "Active nvcc".to_owned(),
        version,
        executable_path: Some(nvcc.display().to_string()),
        source: ToolkitSource::ActivePath,
        packages: vec![],
        manageable: false,
    }))
}

pub fn managed_status(installed: &[String], active: Option<&ToolkitStatus>) -> Vec<ToolkitStatus> {
    let mut packages = installed
        .iter()
        .filter(|name| is_toolkit_package(name))
        .cloned()
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    if packages.is_empty() {
        return vec![];
    }
    let version = packages
        .iter()
        .filter_map(|name| version_from_package(name))
        .max_by(|left, right| version_parts(left).cmp(&version_parts(right)))
        .or_else(|| {
            active
                .filter(|value| {
                    value
                        .executable_path
                        .as_deref()
                        .is_some_and(|path| path.starts_with("/usr/local/cuda"))
                })
                .and_then(|value| value.version.clone())
        });
    let executable_path = version
        .as_ref()
        .map(|version| format!("/usr/local/cuda-{version}/bin/nvcc"))
        .or_else(|| Some("/usr/local/cuda/bin/nvcc".to_owned()));
    vec![ToolkitStatus {
        name: "System-managed CUDA Toolkit".to_owned(),
        version,
        executable_path,
        source: ToolkitSource::SystemPackageManager,
        packages,
        manageable: true,
    }]
}

pub fn is_toolkit_package(name: &str) -> bool {
    name == "cuda-toolkit"
        || name
            .strip_prefix("cuda-toolkit-")
            .is_some_and(|suffix| suffix.split('-').all(|part| part.parse::<u32>().is_ok()))
}

fn version_from_package(name: &str) -> Option<String> {
    let suffix = name.strip_prefix("cuda-toolkit-")?;
    let mut parts = suffix.split('-');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    parts.next().is_none().then(|| format!("{major}.{minor}"))
}

fn version_parts(version: &str) -> Vec<u32> {
    version
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn find_active_nvcc() -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .map(|directory| directory.join("nvcc"))
        .find(|path| path.is_file())
        .or_else(|| {
            let fallback = PathBuf::from("/usr/local/cuda/bin/nvcc");
            fallback.is_file().then_some(fallback)
        })
}

fn parse_nvcc_version(output: &str) -> Option<&str> {
    let (_, after_release) = output.split_once("release ")?;
    after_release
        .split(|character: char| character == ',' || character.is_whitespace())
        .find(|part| !part.is_empty())
}

pub fn versioned_package(version: &str) -> Result<String> {
    let normalized = version.trim().replace('.', "-");
    let mut parts = normalized.split('-');
    let (Some(major), Some(minor), None) = (parts.next(), parts.next(), parts.next()) else {
        bail!("invalid CUDA Toolkit version {version:?}; expected MAJOR.MINOR, for example 13.3");
    };
    if major.is_empty()
        || minor.is_empty()
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
    {
        bail!("invalid CUDA Toolkit version {version:?}; expected MAJOR.MINOR, for example 13.3");
    }
    Ok(format!("cuda-toolkit-{major}-{minor}"))
}

pub fn verification_command() -> CommandSpec {
    CommandSpec::new("/usr/local/cuda/bin/nvcc", ["--version"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_versioned_package_names() {
        assert_eq!(versioned_package("13.3").unwrap(), "cuda-toolkit-13-3");
        assert_eq!(versioned_package("12-8").unwrap(), "cuda-toolkit-12-8");
        for invalid in ["13", "13.3.0", "latest", "13.x", ""] {
            assert!(versioned_package(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn parses_nvcc_toolkit_version() {
        let output = "Cuda compilation tools, release 13.1, V13.1.80\n";
        assert_eq!(parse_nvcc_version(output), Some("13.1"));
    }

    #[test]
    fn uses_latest_meta_package_when_version_is_not_pinned() {
        assert_eq!(package(None).unwrap(), "cuda-toolkit");
        assert_eq!(package(Some("13.3")).unwrap(), "cuda-toolkit-13-3");
    }

    #[test]
    fn custom_active_nvcc_does_not_create_a_managed_toolkit() {
        let active = ToolkitStatus {
            name: "Active nvcc".into(),
            version: Some("12.8".into()),
            executable_path: Some("/opt/conda/envs/ml/bin/nvcc".into()),
            source: ToolkitSource::ActivePath,
            packages: vec![],
            manageable: false,
        };
        assert!(managed_status(&[], Some(&active)).is_empty());
        let managed = managed_status(&["cuda-toolkit-12-8".into()], Some(&active));
        assert_eq!(managed[0].version.as_deref(), Some("12.8"));
        assert_eq!(managed[0].packages, ["cuda-toolkit-12-8"]);
        assert!(managed[0].manageable);
    }
}
