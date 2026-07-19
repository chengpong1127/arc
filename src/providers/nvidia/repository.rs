use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::model::{
    command::CommandSpec,
    system::{Distribution, OsInfo, PackageManager},
};

const CUDA_KEYRING_PACKAGE: &str = "cuda-keyring_1.1-1_all.deb";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NvidiaRepository {
    pub distro: String,
    pub base_url: String,
}

pub fn resolve(os: &OsInfo) -> Result<NvidiaRepository> {
    let major = version_major(&os.version_id);
    let distro = match os.distribution {
        Distribution::Ubuntu if matches!(release(&os.version_id), "22.04" | "24.04" | "26.04") => {
            format!("ubuntu{}", release(&os.version_id).replace('.', ""))
        }
        Distribution::Debian if matches!(major, Some(12 | 13)) => {
            format!("debian{}", major.unwrap())
        }
        Distribution::Rhel | Distribution::AlmaLinux | Distribution::RockyLinux
            if matches!(major, Some(8..=10)) =>
        {
            format!("rhel{}", major.unwrap())
        }
        Distribution::OracleLinux if matches!(major, Some(8 | 9)) => {
            format!("rhel{}", major.unwrap())
        }
        Distribution::Fedora if major == Some(44) => "fedora44".to_owned(),
        Distribution::AmazonLinux if major == Some(2023) => "amzn2023".to_owned(),
        Distribution::AzureLinux if major == Some(3) => "azl3".to_owned(),
        Distribution::OpenSuse if major == Some(15) => "opensuse15".to_owned(),
        Distribution::OpenSuse if major == Some(16) => "suse16".to_owned(),
        Distribution::Sles if major == Some(15) => "sles15".to_owned(),
        Distribution::Sles if major == Some(16) => "suse16".to_owned(),
        Distribution::KylinOs
            if major == Some(11)
                || os.version_id.to_ascii_uppercase().starts_with('V')
                    && version_major(&os.version_id[1..]) == Some(11) =>
        {
            "kylin11".to_owned()
        }
        _ => bail!(
            "NVIDIA does not publish an exact repository target for {}. Refusing to substitute another distribution or release.",
            os.display_name()
        ),
    };
    let architecture = match os.architecture.as_str() {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64"
            if matches!(
                os.distribution,
                Distribution::Rhel
                    | Distribution::Ubuntu
                    | Distribution::Sles
                    | Distribution::KylinOs
                    | Distribution::AzureLinux
                    | Distribution::AmazonLinux
            ) =>
        {
            "sbsa"
        }
        architecture => bail!(
            "NVIDIA does not publish a supported CUDA repository for architecture {architecture} on {}",
            os.display_name()
        ),
    };
    let base_url = format!(
        "https://developer.download.nvidia.com/compute/cuda/repos/{distro}/{architecture}/"
    );
    Ok(NvidiaRepository { distro, base_url })
}

pub fn is_configured(os: &OsInfo, repository: &NvidiaRepository) -> Result<bool> {
    let roots: &[&str] = match os.package_manager() {
        PackageManager::AptGet => &["/etc/apt/sources.list", "/etc/apt/sources.list.d"],
        PackageManager::Dnf | PackageManager::Tdnf => &["/etc/yum.repos.d"],
        PackageManager::Zypper => &["/etc/zypp/repos.d"],
    };
    roots.iter().try_fold(false, |found, root| {
        Ok(found || path_contains_repository(Path::new(root), &repository.base_url)?)
    })
}

fn path_contains_repository(path: &Path, base_url: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if path.is_file() {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return Ok(false);
        };
        if !matches!(extension, "list" | "sources" | "repo")
            && path.file_name().and_then(|value| value.to_str()) != Some("sources.list")
        {
            return Ok(false);
        }
        let contents = fs::read_to_string(path).with_context(|| {
            format!(
                "could not inspect repository configuration at {}",
                path.display()
            )
        })?;
        return Ok(contents.contains(base_url) || contents.contains(base_url.trim_end_matches('/')));
    }
    for entry in fs::read_dir(path)
        .with_context(|| format!("could not inspect repository directory {}", path.display()))?
    {
        let entry = entry.with_context(|| format!("could not inspect {}", path.display()))?;
        if entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
            && path_contains_repository(&entry.path(), base_url)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn setup_commands(manager: PackageManager, repository: &NvidiaRepository) -> Vec<CommandSpec> {
    let repo_url = format!("{}cuda-{}.repo", repository.base_url, repository.distro);
    let temporary_path = temporary_download_path(match manager {
        PackageManager::AptGet => CUDA_KEYRING_PACKAGE,
        _ => "cuda.repo",
    });
    let temporary_path = temporary_path.to_string_lossy().into_owned();
    match manager {
        PackageManager::AptGet => vec![
            CommandSpec::new(
                "curl",
                [
                    "--fail",
                    "--location",
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "--output",
                    &temporary_path,
                    &format!("{}{CUDA_KEYRING_PACKAGE}", repository.base_url),
                ],
            ),
            CommandSpec::sudo("dpkg", ["-i", &temporary_path]),
            CommandSpec::new("rm", ["-f", &temporary_path]),
        ],
        PackageManager::Dnf | PackageManager::Tdnf => vec![
            CommandSpec::new(
                "curl",
                [
                    "--fail",
                    "--location",
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "--output",
                    &temporary_path,
                    &repo_url,
                ],
            ),
            CommandSpec::sudo(
                "install",
                [
                    "-m",
                    "0644",
                    &temporary_path,
                    &format!("/etc/yum.repos.d/cuda-{}.repo", repository.distro),
                ],
            ),
            CommandSpec::new("rm", ["-f", &temporary_path]),
        ],
        PackageManager::Zypper => vec![CommandSpec::sudo(
            "zypper",
            ["--non-interactive", "addrepo", &repo_url, "cuda-nvidia"],
        )],
    }
}

fn temporary_download_path(file_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(format!(
        "cudaenv-{}-{nonce}-{file_name}",
        std::process::id()
    ))
}

fn release(version: &str) -> &str {
    let end = version
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || *c == '.')
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    &version[..end]
}

fn version_major(version: &str) -> Option<u32> {
    version
        .split(['.', ' ', '-'])
        .next()?
        .trim_start_matches(['v', 'V'])
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(distribution: Distribution, version: &str) -> OsInfo {
        OsInfo {
            distribution,
            name: "Test".into(),
            version_id: version.into(),
            architecture: "x86_64".into(),
            is_wsl: false,
        }
    }

    #[test]
    fn resolves_official_repository_targets() {
        for (distribution, version, expected) in [
            (Distribution::Ubuntu, "22.04", "ubuntu2204"),
            (Distribution::Ubuntu, "24.04", "ubuntu2404"),
            (Distribution::Ubuntu, "26.04", "ubuntu2604"),
            (Distribution::Debian, "12", "debian12"),
            (Distribution::Debian, "13", "debian13"),
            (Distribution::Rhel, "8.10", "rhel8"),
            (Distribution::Rhel, "9.7", "rhel9"),
            (Distribution::Rhel, "10.1", "rhel10"),
            (Distribution::RockyLinux, "9.7", "rhel9"),
            (Distribution::AlmaLinux, "10.1", "rhel10"),
            (Distribution::OracleLinux, "9", "rhel9"),
            (Distribution::Fedora, "44", "fedora44"),
            (Distribution::AmazonLinux, "2023", "amzn2023"),
            (Distribution::AzureLinux, "3.0", "azl3"),
            (Distribution::OpenSuse, "15.6", "opensuse15"),
            (Distribution::OpenSuse, "16", "suse16"),
            (Distribution::Sles, "15.7", "sles15"),
            (Distribution::Sles, "16", "suse16"),
            (Distribution::KylinOs, "V11", "kylin11"),
        ] {
            assert_eq!(
                resolve(&os(distribution, version)).unwrap().distro,
                expected
            );
        }
    }

    #[test]
    fn rejects_unpublished_release_instead_of_substituting() {
        assert!(resolve(&os(Distribution::Ubuntu, "25.10")).is_err());
    }

    #[test]
    fn rejects_unsupported_architecture_distribution_combinations() {
        let mut debian_arm = os(Distribution::Debian, "13");
        debian_arm.architecture = "aarch64".into();
        assert!(resolve(&debian_arm).is_err());

        let mut ubuntu_arm = os(Distribution::Ubuntu, "24.04");
        ubuntu_arm.architecture = "aarch64".into();
        assert_eq!(
            resolve(&ubuntu_arm).unwrap().base_url,
            "https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/sbsa/"
        );
    }

    #[test]
    fn generates_repository_setup_for_each_manager_family() {
        let repository = resolve(&os(Distribution::Ubuntu, "24.04")).unwrap();
        assert_eq!(setup_commands(PackageManager::AptGet, &repository).len(), 3);
        assert!(
            setup_commands(PackageManager::Dnf, &repository)[1]
                .display()
                .contains("/etc/yum.repos.d/")
        );
        assert!(
            setup_commands(PackageManager::Zypper, &repository)[0]
                .display()
                .contains("addrepo")
        );
    }
}
