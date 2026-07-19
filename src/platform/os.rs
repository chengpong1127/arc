use std::{collections::HashMap, fs};

use anyhow::{Context, Result, bail};

use crate::model::system::{Distribution, OsInfo};

const OS_RELEASE_PATH: &str = "/etc/os-release";

pub fn detect() -> Result<OsInfo> {
    let contents = fs::read_to_string(OS_RELEASE_PATH)
        .with_context(|| format!("could not read {OS_RELEASE_PATH}"))?;
    let fields = parse_os_release(&contents);
    let id = required_field(&fields, "ID")?.to_ascii_lowercase();
    let distribution = map_distribution(&id)?;
    let version_id = required_field(&fields, "VERSION_ID")?.to_owned();
    let name = fields
        .get("NAME")
        .cloned()
        .unwrap_or_else(|| distribution_name(distribution).to_owned());

    Ok(OsInfo {
        distribution,
        name,
        version_id,
        architecture: std::env::consts::ARCH.to_owned(),
        is_wsl: detect_wsl(),
    })
}

fn parse_os_release(contents: &str) -> HashMap<String, String> {
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.to_owned(), unquote(value).to_owned()))
        })
        .collect()
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn required_field<'a>(fields: &'a HashMap<String, String>, key: &str) -> Result<&'a str> {
    fields
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{OS_RELEASE_PATH} does not contain {key}"))
}

fn map_distribution(id: &str) -> Result<Distribution> {
    let distribution = match id {
        "ubuntu" => Distribution::Ubuntu,
        "debian" => Distribution::Debian,
        "rhel" | "redhat" => Distribution::Rhel,
        "almalinux" => Distribution::AlmaLinux,
        "rocky" | "rockylinux" => Distribution::RockyLinux,
        "ol" | "oracle" | "oraclelinux" => Distribution::OracleLinux,
        "fedora" => Distribution::Fedora,
        "amzn" => Distribution::AmazonLinux,
        "azurelinux" | "azl" | "mariner" => Distribution::AzureLinux,
        "opensuse-leap" | "opensuse" => Distribution::OpenSuse,
        "sles" => Distribution::Sles,
        "kylin" | "kylinos" => Distribution::KylinOs,
        _ => bail!("GPU package installation is not supported on Linux distribution {id:?}"),
    };
    Ok(distribution)
}

fn distribution_name(distribution: Distribution) -> &'static str {
    match distribution {
        Distribution::Ubuntu => "Ubuntu",
        Distribution::Debian => "Debian",
        Distribution::Rhel => "Red Hat Enterprise Linux",
        Distribution::AlmaLinux => "AlmaLinux",
        Distribution::RockyLinux => "Rocky Linux",
        Distribution::OracleLinux => "Oracle Linux",
        Distribution::Fedora => "Fedora",
        Distribution::AmazonLinux => "Amazon Linux",
        Distribution::AzureLinux => "Azure Linux",
        Distribution::OpenSuse => "openSUSE",
        Distribution::Sles => "SUSE Linux Enterprise Server",
        Distribution::KylinOs => "KylinOS",
    }
}

fn detect_wsl() -> bool {
    std::env::var_os("WSL_INTEROP").is_some()
        || std::env::var_os("WSL_DISTRO_NAME").is_some()
        || ["/proc/sys/kernel/osrelease", "/proc/version"]
            .iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .any(|value| value.to_ascii_lowercase().contains("microsoft"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::system::PackageManager;

    #[test]
    fn preserves_version_id_exactly() {
        let fields = parse_os_release(
            "NAME=\"Ubuntu\"\nID=ubuntu\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04.2 LTS\"\n",
        );
        assert_eq!(required_field(&fields, "VERSION_ID").unwrap(), "24.04");
        assert_eq!(required_field(&fields, "NAME").unwrap(), "Ubuntu");
    }

    #[test]
    fn parses_comments_unquoted_values_and_equals_signs() {
        let fields = parse_os_release("# comment\nID=debian\nVERSION_ID=13\nEXTRA=one=two\n");
        assert_eq!(fields.get("ID").map(String::as_str), Some("debian"));
        assert_eq!(fields.get("EXTRA").map(String::as_str), Some("one=two"));
    }

    #[test]
    fn maps_os_release_ids() {
        let cases = [
            ("ubuntu", Distribution::Ubuntu),
            ("debian", Distribution::Debian),
            ("rhel", Distribution::Rhel),
            ("almalinux", Distribution::AlmaLinux),
            ("rocky", Distribution::RockyLinux),
            ("ol", Distribution::OracleLinux),
            ("fedora", Distribution::Fedora),
            ("amzn", Distribution::AmazonLinux),
            ("azurelinux", Distribution::AzureLinux),
            ("opensuse-leap", Distribution::OpenSuse),
            ("sles", Distribution::Sles),
            ("kylin", Distribution::KylinOs),
        ];
        for (id, expected) in cases {
            assert_eq!(map_distribution(id).unwrap(), expected);
        }
    }

    #[test]
    fn selects_package_manager_family() {
        for (distribution, expected) in [
            (Distribution::Ubuntu, PackageManager::AptGet),
            (Distribution::KylinOs, PackageManager::Dnf),
            (Distribution::OracleLinux, PackageManager::Dnf),
            (Distribution::AzureLinux, PackageManager::Tdnf),
            (Distribution::Sles, PackageManager::Zypper),
        ] {
            assert_eq!(sample(distribution, false).package_manager(), expected);
        }
    }

    #[test]
    fn rejects_wsl_with_vendor_neutral_host_explanation() {
        let error = sample(Distribution::Ubuntu, true)
            .ensure_driver_installable("NVIDIA")
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Windows host"));
        assert!(message.contains("NVIDIA"));
    }

    fn sample(distribution: Distribution, is_wsl: bool) -> OsInfo {
        OsInfo {
            distribution,
            name: "test".into(),
            version_id: "1".into(),
            architecture: "x86_64".into(),
            is_wsl,
        }
    }
}
