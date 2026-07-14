use std::{fs, path::Path};

#[cfg(target_os = "windows")]
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::i18n::Language;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub base: BaseConfig,
    pub rds: RdsConfig,
    pub users: UserConfig,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    pub language: Language,
}

impl AppConfig {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if !path.exists() {
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&source).with_context(|| format!("invalid TOML in {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let source = toml::to_string_pretty(self).context("failed to serialize configuration")?;
        fs::write(path, source).with_context(|| format!("failed to write {}", path.display()))?;
        secure_config_file(path)?;
        Ok(())
    }

    pub fn validate_base_access(&self, kms: bool, ssh: bool, rdp: bool) -> Result<()> {
        if kms
            && (self.base.windows_product_key.trim().is_empty()
                || self.base.kms_server.trim().is_empty())
        {
            bail!("KMS activation requires both a product key and a KMS server")
        }
        if ssh && !is_supported_public_key(&self.base.administrator_public_key) {
            bail!("SSH requires a valid Administrator public key")
        }
        if (ssh || rdp) && self.base.allowed_remote_addresses.is_empty() {
            bail!("at least one Windows Firewall source address is required")
        }
        Ok(())
    }

    pub fn validate_rds_licensing(&self) -> Result<()> {
        if self.rds.cal_count == 0 {
            bail!("RDS CAL count must be greater than zero")
        }
        match self.rds.cal_install_method {
            CalInstallMethod::None => {
                bail!("select Agreement or KeyPackId before activating production CALs")
            }
            CalInstallMethod::Agreement => {
                let agreement = self.rds.agreement_number.trim();
                if agreement.len() != 7 || !agreement.chars().all(|ch| ch.is_ascii_digit()) {
                    bail!("the Microsoft agreement/enrollment number must contain seven digits")
                }
            }
            CalInstallMethod::KeyPackId => {
                let normalized: String = self
                    .rds
                    .license_key_pack_id
                    .chars()
                    .filter(|ch| !ch.is_whitespace() && *ch != '-')
                    .collect();
                if normalized.len() != 35
                    || !normalized.chars().all(|ch| ch.is_ascii_alphanumeric())
                {
                    bail!("the Microsoft Clearinghouse Key Pack ID must contain 35 characters")
                }
            }
        }

        let contact = &self.rds.contact;
        if [
            contact.first_name.as_str(),
            contact.last_name.as_str(),
            contact.company.as_str(),
            contact.country_region.as_str(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            bail!("license-server activation requires first name, last name, company, and country")
        }
        Ok(())
    }

    pub fn validate_local_users(&self) -> Result<()> {
        if !(1..=200).contains(&self.users.count) {
            bail!("user count must be between 1 and 200")
        }
        if self.users.prefix.trim().is_empty()
            || !self
                .users
                .prefix
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            bail!("user prefix may contain only ASCII letters, numbers, underscore, and hyphen")
        }
        if !(1..=6).contains(&self.users.number_width) {
            bail!("username number width must be between 1 and 6")
        }
        if self.users.password_length < 14 {
            bail!("generated passwords must contain at least 14 characters")
        }
        let last_name = format!(
            "{}{:0width$}",
            self.users.prefix,
            self.users.start_index + self.users.count - 1,
            width = self.users.number_width as usize
        );
        if last_name.len() > 20 {
            bail!("generated local usernames must not exceed 20 characters")
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BaseConfig {
    pub windows_product_key: String,
    pub kms_server: String,
    pub administrator_public_key: String,
    pub allowed_remote_addresses: Vec<String>,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            windows_product_key: String::new(),
            kms_server: String::new(),
            administrator_public_key: "ssh-ed25519 REPLACE_WITH_YOUR_PUBLIC_KEY administrator"
                .to_owned(),
            allowed_remote_addresses: vec!["Any".to_owned()],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum LicensingMode {
    #[default]
    PerDevice,
    PerUser,
}

impl LicensingMode {
    pub const ALL: [Self; 2] = [Self::PerDevice, Self::PerUser];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PerDevice => "PerDevice",
            Self::PerUser => "PerUser",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum CalInstallMethod {
    #[default]
    None,
    Agreement,
    KeyPackId,
}

impl CalInstallMethod {
    pub const ALL: [Self; 3] = [Self::None, Self::Agreement, Self::KeyPackId];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Agreement => "Agreement",
            Self::KeyPackId => "KeyPackId",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AgreementType {
    Select,
    #[default]
    Enterprise,
    Campus,
    School,
    ServiceProvider,
    Other,
}

impl AgreementType {
    pub const ALL: [Self; 6] = [
        Self::Select,
        Self::Enterprise,
        Self::Campus,
        Self::School,
        Self::ServiceProvider,
        Self::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Enterprise => "Enterprise",
            Self::Campus => "Campus",
            Self::School => "School",
            Self::ServiceProvider => "ServiceProvider",
            Self::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct RdsConfig {
    pub licensing_mode: LicensingMode,
    pub license_server: String,
    pub cal_install_method: CalInstallMethod,
    pub agreement_type: AgreementType,
    pub agreement_number: String,
    pub license_key_pack_id: String,
    pub cal_product_version_id: u32,
    pub cal_count: u32,
    pub contact: LicenseContact,
}

impl Default for RdsConfig {
    fn default() -> Self {
        Self {
            licensing_mode: LicensingMode::PerDevice,
            license_server: "localhost".to_owned(),
            cal_install_method: CalInstallMethod::None,
            agreement_type: AgreementType::Enterprise,
            agreement_number: String::new(),
            license_key_pack_id: String::new(),
            cal_product_version_id: 7,
            cal_count: 20,
            contact: LicenseContact::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct LicenseContact {
    pub first_name: String,
    pub last_name: String,
    pub company: String,
    pub country_region: String,
    pub email: String,
    pub org_unit: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
}

impl Default for LicenseContact {
    fn default() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
            company: String::new(),
            country_region: "China".to_owned(),
            email: String::new(),
            org_unit: String::new(),
            address: String::new(),
            city: String::new(),
            state: String::new(),
            postal_code: String::new(),
        }
    }
}

fn secure_config_file(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let status = Command::new("icacls.exe")
            .arg(path)
            .args([
                "/inheritance:r",
                "/grant:r",
                "*S-1-5-32-544:F",
                "*S-1-5-18:F",
            ])
            .status()
            .context("failed to protect the configuration file with icacls")?;
        if !status.success() {
            bail!("icacls failed while protecting {}", path.display())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
    }
    Ok(())
}

fn is_supported_public_key(value: &str) -> bool {
    let Some(key_type) = value.split_whitespace().next() else {
        return false;
    };
    matches!(
        key_type,
        "ssh-ed25519"
            | "ssh-rsa"
            | "ecdsa-sha2-nistp256"
            | "ecdsa-sha2-nistp384"
            | "ecdsa-sha2-nistp521"
    ) && value.split_whitespace().nth(1).is_some()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct UserConfig {
    pub count: u32,
    pub prefix: String,
    pub start_index: u32,
    pub number_width: u32,
    pub password_length: u32,
    pub reset_existing_passwords: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            count: 20,
            prefix: "user".to_owned(),
            start_index: 1,
            number_width: 2,
            password_length: 16,
            reset_existing_passwords: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_users_accepts_user01_through_user20() {
        let config = AppConfig::default();
        config.validate_local_users().unwrap();
    }

    #[test]
    fn rds_licensing_rejects_missing_entitlement() {
        let config = AppConfig::default();
        assert!(config.validate_rds_licensing().is_err());
    }

    #[test]
    fn accepts_ed25519_administrator_key() {
        let mut config = AppConfig::default();
        config.base.administrator_public_key =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITest administrator".to_owned();
        config.validate_base_access(false, true, false).unwrap();
    }

    #[test]
    fn example_configuration_stays_parseable() {
        let config: AppConfig = toml::from_str(include_str!("../config.example.toml")).unwrap();
        assert_eq!(config.users.count, 20);
        assert_eq!(config.rds.cal_install_method, CalInstallMethod::None);
        assert_eq!(config.ui.language, Language::SimplifiedChinese);
    }
}
