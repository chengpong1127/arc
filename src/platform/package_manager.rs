use anyhow::{Context, Result};

use crate::{
    model::{command::CommandSpec, system::PackageManager},
    platform::command::{self, CommandRunner, SystemCommandRunner},
};

pub fn refresh_command(manager: PackageManager) -> CommandSpec {
    match manager {
        PackageManager::AptGet => CommandSpec::sudo("apt-get", ["update"]),
        PackageManager::Dnf => CommandSpec::sudo("dnf", ["makecache"]),
        PackageManager::Tdnf => CommandSpec::sudo("tdnf", ["makecache"]),
        PackageManager::Zypper => CommandSpec::sudo("zypper", ["--non-interactive", "refresh"]),
    }
}

pub fn query_command(manager: PackageManager, package: &str) -> CommandSpec {
    match manager {
        PackageManager::AptGet => {
            CommandSpec::new("apt-cache", ["show", "--no-all-versions", package])
        }
        PackageManager::Dnf => CommandSpec::new("dnf", ["--quiet", "list", "--available", package]),
        PackageManager::Tdnf => CommandSpec::new("tdnf", ["list", "available", package]),
        PackageManager::Zypper => {
            CommandSpec::new("zypper", ["--non-interactive", "info", package])
        }
    }
}

pub fn install_command(manager: PackageManager, package: &str) -> CommandSpec {
    install_command_with_options(manager, package, false)
}

pub fn install_command_with_options(
    manager: PackageManager,
    package: &str,
    allow_erasing: bool,
) -> CommandSpec {
    match manager {
        PackageManager::AptGet => CommandSpec::sudo("apt-get", ["install", "-y", package]),
        PackageManager::Dnf if allow_erasing => {
            CommandSpec::sudo("dnf", ["install", "-y", "--allowerasing", package])
        }
        PackageManager::Dnf => CommandSpec::sudo("dnf", ["install", "-y", package]),
        PackageManager::Tdnf => CommandSpec::sudo("tdnf", ["install", "-y", package]),
        PackageManager::Zypper => {
            CommandSpec::sudo("zypper", ["--non-interactive", "install", package])
        }
    }
}

pub fn reinstall_command(manager: PackageManager, packages: &[String]) -> Option<CommandSpec> {
    if packages.is_empty() {
        return None;
    }
    let package_refs = packages.iter().map(String::as_str);
    Some(match manager {
        PackageManager::AptGet => CommandSpec::sudo(
            "apt-get",
            ["install", "--reinstall", "-y"]
                .into_iter()
                .chain(package_refs),
        ),
        PackageManager::Dnf => {
            CommandSpec::sudo("dnf", ["reinstall", "-y"].into_iter().chain(package_refs))
        }
        PackageManager::Tdnf => {
            CommandSpec::sudo("tdnf", ["reinstall", "-y"].into_iter().chain(package_refs))
        }
        PackageManager::Zypper => CommandSpec::sudo(
            "zypper",
            ["--non-interactive", "install", "--force"]
                .into_iter()
                .chain(package_refs),
        ),
    })
}

pub fn installed_packages(manager: PackageManager) -> Result<Vec<String>> {
    installed_packages_with(&SystemCommandRunner, manager)
}

pub fn installed_packages_qualified(manager: PackageManager) -> Result<Vec<String>> {
    installed_packages_with_options(&SystemCommandRunner, manager, true)
}

pub fn installed_packages_with(
    runner: &impl CommandRunner,
    manager: PackageManager,
) -> Result<Vec<String>> {
    installed_packages_with_options(runner, manager, false)
}

fn installed_packages_with_options(
    runner: &impl CommandRunner,
    manager: PackageManager,
    preserve_architecture: bool,
) -> Result<Vec<String>> {
    let command = match manager {
        PackageManager::AptGet => CommandSpec::new(
            "dpkg-query",
            ["-W", "-f=${db:Status-Abbrev}\t${binary:Package}\\n"],
        ),
        _ => CommandSpec::new("rpm", ["-qa", "--qf", "%{NAME}\\n"]),
    };
    let output =
        command::capture(runner, command).context("could not inspect installed packages")?;
    if !output.success {
        return Ok(Vec::new());
    }
    let mut result = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if manager == PackageManager::AptGet {
                let (status, package) = line.split_once('\t')?;
                status.starts_with("ii ").then(|| {
                    if preserve_architecture {
                        package.to_owned()
                    } else {
                        package.split(':').next().unwrap_or(package).to_owned()
                    }
                })
            } else {
                Some(line.trim().to_owned())
            }
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    Ok(result)
}

pub fn apt_remove_command(options: &[&str], packages: &[&str]) -> CommandSpec {
    CommandSpec::sudo(
        "apt-get",
        options.iter().copied().chain(packages.iter().copied()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_each_package_manager_pipeline() {
        for manager in [
            PackageManager::AptGet,
            PackageManager::Dnf,
            PackageManager::Tdnf,
            PackageManager::Zypper,
        ] {
            assert!(!refresh_command(manager).args.is_empty());
            assert!(
                query_command(manager, "gpu-sdk")
                    .display()
                    .contains("gpu-sdk")
            );
            assert!(
                install_command(manager, "gpu-sdk")
                    .display()
                    .contains("gpu-sdk")
            );
            assert!(
                reinstall_command(manager, &["gpu-driver".into()])
                    .unwrap()
                    .display()
                    .contains("gpu-driver")
            );
        }
        assert!(
            install_command_with_options(PackageManager::Dnf, "nvidia-open", true)
                .display()
                .contains("--allowerasing")
        );
    }
}
