use std::{collections::BTreeMap, sync::mpsc::Sender};

#[cfg(target_os = "windows")]
use std::{
    io::{BufRead, BufReader, Read},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(any(target_os = "windows", test))]
use anyhow::Context;
#[cfg(target_os = "windows")]
use anyhow::anyhow;
use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};

use crate::{config::AppConfig, scripts};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Feature {
    BaseAccess,
    RdsRoles,
    RdsLicensing,
    LocalUsers,
}

impl Feature {
    pub const ALL: [Self; 4] = [
        Self::BaseAccess,
        Self::RdsRoles,
        Self::RdsLicensing,
        Self::LocalUsers,
    ];
}

#[derive(Clone, Copy, Debug)]
pub struct BaseAccessSelection {
    pub kms: bool,
    pub ssh: bool,
    pub rdp: bool,
}

impl Default for BaseAccessSelection {
    fn default() -> Self {
        Self {
            kms: true,
            ssh: true,
            rdp: true,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Task {
    BaseAccess(BaseAccessSelection),
    RdsRoles,
    RdsLicensing,
    LocalUsers,
    Reboot,
}

#[derive(Clone, Debug, Default)]
pub struct TaskOutcome {
    pub reboot_required: bool,
    pub credential_file: Option<String>,
}

#[derive(Debug)]
pub enum TaskEvent {
    Log(String),
    Finished(std::result::Result<TaskOutcome, String>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoleInstallState {
    Installed,
    PendingRestart,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RdsRoleStatus {
    pub session_host: RoleInstallState,
    pub licensing: RoleInstallState,
    pub licensing_ui: RoleInstallState,
    pub part_of_domain: bool,
}

impl RdsRoleStatus {
    pub fn fully_installed(self) -> bool {
        [self.session_host, self.licensing, self.licensing_ui]
            .into_iter()
            .all(|state| state == RoleInstallState::Installed)
    }

    pub fn all_present(self) -> bool {
        [self.session_host, self.licensing, self.licensing_ui]
            .into_iter()
            .all(|state| state != RoleInstallState::Missing)
    }

    pub fn has_pending_restart(self) -> bool {
        [self.session_host, self.licensing, self.licensing_ui]
            .contains(&RoleInstallState::PendingRestart)
    }
}

pub fn spawn_rds_role_check(tx: Sender<std::result::Result<RdsRoleStatus, String>>) {
    std::thread::spawn(move || {
        let result = check_rds_roles().map_err(|error| format!("{error:#}"));
        let _ = tx.send(result);
    });
}

fn check_rds_roles() -> Result<RdsRoleStatus> {
    #[cfg(not(target_os = "windows"))]
    {
        bail!("RDS role detection is supported only on Windows Server")
    }

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let encoded = encode_powershell(&prepare_powershell_script(scripts::CHECK_RDS_ROLES));
        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand",
                encoded.as_str(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .context("failed to query Windows RDS roles")?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).replace('\0', "");
            bail!("RDS role query failed: {}", error.trim())
        }
        let stdout = String::from_utf8_lossy(&output.stdout).replace('\0', "");
        parse_rds_role_status(&stdout)
    }
}

#[cfg(any(target_os = "windows", test))]
fn parse_rds_role_status(output: &str) -> Result<RdsRoleStatus> {
    let mut states = BTreeMap::new();
    let mut part_of_domain = None;
    for line in output.lines() {
        let mut fields = line.trim().split('|');
        let record_type = fields.next();
        if record_type == Some("SYSTEM") {
            if fields.next() == Some("PartOfDomain") {
                part_of_domain = fields.next().and_then(|value| {
                    match value.trim().to_ascii_lowercase().as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => None,
                    }
                });
            }
            continue;
        }
        if record_type != Some("FEATURE") {
            continue;
        }
        let (Some(name), Some(raw_state)) = (fields.next(), fields.next()) else {
            continue;
        };
        let state = match raw_state.trim().to_ascii_lowercase().as_str() {
            "installed" => RoleInstallState::Installed,
            "installpending" => RoleInstallState::PendingRestart,
            _ => RoleInstallState::Missing,
        };
        states.insert(name.to_ascii_lowercase(), state);
    }
    let state = |name: &str| {
        states
            .get(&name.to_ascii_lowercase())
            .copied()
            .with_context(|| format!("Windows did not return the state of {name}"))
    };
    Ok(RdsRoleStatus {
        session_host: state("RDS-RD-Server")?,
        licensing: state("RDS-Licensing")?,
        licensing_ui: state("RDS-Licensing-UI")?,
        part_of_domain: part_of_domain.context("Windows did not return domain membership")?,
    })
}

pub fn spawn_task(task: Task, config: AppConfig, dry_run: bool, tx: Sender<TaskEvent>) {
    std::thread::spawn(move || {
        let result = execute(task, &config, dry_run, &tx).map_err(|error| format!("{error:#}"));
        let _ = tx.send(TaskEvent::Finished(result));
    });
}

fn execute(
    task: Task,
    config: &AppConfig,
    dry_run: bool,
    tx: &Sender<TaskEvent>,
) -> Result<TaskOutcome> {
    match task {
        Task::BaseAccess(selection) => run_base_access(config, selection, dry_run, tx),
        Task::RdsRoles => run_rds_roles(dry_run, tx),
        Task::RdsLicensing => run_rds_licensing(config, dry_run, tx),
        Task::LocalUsers => run_local_users(config, dry_run, tx),
        Task::Reboot => run_reboot(dry_run, tx),
    }
}

fn run_base_access(
    config: &AppConfig,
    selection: BaseAccessSelection,
    dry_run: bool,
    tx: &Sender<TaskEvent>,
) -> Result<TaskOutcome> {
    if !dry_run {
        config.validate_base_access(selection.kms, selection.ssh, selection.rdp)?;
    }
    if !selection.kms && !selection.ssh && !selection.rdp {
        bail!("select at least one base-access action")
    }

    if selection.kms {
        log(
            tx,
            "Running Windows KMS activation (no inbound firewall rule required)",
        );
        run_script(
            "Windows KMS activation",
            scripts::STAGE1_KMS,
            env_map([
                (
                    "FIRSTSET_WINDOWS_PRODUCT_KEY",
                    config.base.windows_product_key.as_str(),
                ),
                ("FIRSTSET_KMS_SERVER", config.base.kms_server.as_str()),
            ]),
            dry_run,
            tx,
        )?;
    }

    let remote_addresses = config.base.allowed_remote_addresses.join(",");
    if selection.ssh {
        log(
            tx,
            "Installing OpenSSH and enforcing the linked TCP 22 firewall rule",
        );
        run_script(
            "OpenSSH installation",
            scripts::STAGE1_SSH,
            env_map([
                (
                    "FIRSTSET_ADMIN_PUBLIC_KEY",
                    config.base.administrator_public_key.as_str(),
                ),
                (
                    "FIRSTSET_ALLOWED_REMOTE_ADDRESSES",
                    remote_addresses.as_str(),
                ),
            ]),
            dry_run,
            tx,
        )?;
    }

    if selection.rdp {
        log(
            tx,
            "Enabling RDP/NLA and enforcing TCP/UDP 3389 firewall rules",
        );
        run_script(
            "Remote Desktop setup",
            scripts::STAGE1_RDP,
            env_map([(
                "FIRSTSET_ALLOWED_REMOTE_ADDRESSES",
                remote_addresses.as_str(),
            )]),
            dry_run,
            tx,
        )?;
    }

    log(tx, "Current active network adapters:");
    run_script(
        "network-info",
        scripts::NETWORK_INFO,
        BTreeMap::new(),
        dry_run,
        tx,
    )?;

    Ok(TaskOutcome::default())
}

fn run_rds_roles(dry_run: bool, tx: &Sender<TaskEvent>) -> Result<TaskOutcome> {
    log(tx, "Installing RD Session Host and RD Licensing roles");
    let output = run_script(
        "RDS role installation",
        scripts::STAGE2_RDS_ROLES,
        BTreeMap::new(),
        dry_run,
        tx,
    )?;
    let reboot_required = dry_run
        || output
            .lines()
            .any(|line| line.trim() == "RESTART_REQUIRED=1");
    Ok(TaskOutcome {
        reboot_required,
        credential_file: None,
    })
}

fn run_rds_licensing(
    config: &AppConfig,
    dry_run: bool,
    tx: &Sender<TaskEvent>,
) -> Result<TaskOutcome> {
    if !dry_run {
        config.validate_rds_licensing()?;
    }
    log(
        tx,
        "Configuring the RD licensing policy and contacting Microsoft Clearinghouse",
    );

    let normalized_key_pack: String = config
        .rds
        .license_key_pack_id
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-')
        .collect();
    let contact = &config.rds.contact;
    let product_version = config.rds.cal_product_version_id.to_string();
    let cal_count = config.rds.cal_count.to_string();
    run_script(
        "RDS licensing configuration",
        scripts::STAGE3_RDS_LICENSING,
        env_map([
            (
                "FIRSTSET_RDS_LICENSING_MODE",
                config.rds.licensing_mode.as_str(),
            ),
            (
                "FIRSTSET_RDS_LICENSE_SERVER",
                config.rds.license_server.as_str(),
            ),
            (
                "FIRSTSET_RDS_CAL_METHOD",
                config.rds.cal_install_method.as_str(),
            ),
            (
                "FIRSTSET_RDS_AGREEMENT_TYPE",
                config.rds.agreement_type.as_str(),
            ),
            (
                "FIRSTSET_RDS_AGREEMENT_NUMBER",
                config.rds.agreement_number.as_str(),
            ),
            ("FIRSTSET_RDS_KEY_PACK_ID", normalized_key_pack.as_str()),
            ("FIRSTSET_RDS_PRODUCT_VERSION", product_version.as_str()),
            ("FIRSTSET_RDS_CAL_COUNT", cal_count.as_str()),
            (
                "FIRSTSET_RDS_CONTACT_FIRST_NAME",
                contact.first_name.as_str(),
            ),
            ("FIRSTSET_RDS_CONTACT_LAST_NAME", contact.last_name.as_str()),
            ("FIRSTSET_RDS_CONTACT_COMPANY", contact.company.as_str()),
            (
                "FIRSTSET_RDS_CONTACT_COUNTRY",
                contact.country_region.as_str(),
            ),
            ("FIRSTSET_RDS_CONTACT_EMAIL", contact.email.as_str()),
            ("FIRSTSET_RDS_CONTACT_ORG_UNIT", contact.org_unit.as_str()),
            ("FIRSTSET_RDS_CONTACT_ADDRESS", contact.address.as_str()),
            ("FIRSTSET_RDS_CONTACT_CITY", contact.city.as_str()),
            ("FIRSTSET_RDS_CONTACT_STATE", contact.state.as_str()),
            (
                "FIRSTSET_RDS_CONTACT_POSTAL_CODE",
                contact.postal_code.as_str(),
            ),
        ]),
        dry_run,
        tx,
    )?;

    Ok(TaskOutcome::default())
}

fn run_local_users(
    config: &AppConfig,
    dry_run: bool,
    tx: &Sender<TaskEvent>,
) -> Result<TaskOutcome> {
    config.validate_local_users()?;
    log(
        tx,
        "Creating local users and adding them to the built-in Remote Desktop Users SID",
    );
    let user_count = config.users.count.to_string();
    let start_index = config.users.start_index.to_string();
    let number_width = config.users.number_width.to_string();
    let password_length = config.users.password_length.to_string();
    let output = run_script(
        "Local user creation",
        scripts::STAGE4_USERS,
        env_map([
            ("FIRSTSET_USER_COUNT", user_count.as_str()),
            ("FIRSTSET_USER_PREFIX", config.users.prefix.as_str()),
            ("FIRSTSET_USER_START_INDEX", start_index.as_str()),
            ("FIRSTSET_USER_NUMBER_WIDTH", number_width.as_str()),
            ("FIRSTSET_USER_PASSWORD_LENGTH", password_length.as_str()),
            (
                "FIRSTSET_RESET_EXISTING_PASSWORDS",
                if config.users.reset_existing_passwords {
                    "1"
                } else {
                    "0"
                },
            ),
        ]),
        dry_run,
        tx,
    )?;
    let credential_file = output
        .lines()
        .find_map(|line| line.strip_prefix("CREDENTIAL_FILE=").map(str::to_owned));
    Ok(TaskOutcome {
        reboot_required: false,
        credential_file,
    })
}

fn run_reboot(dry_run: bool, tx: &Sender<TaskEvent>) -> Result<TaskOutcome> {
    log(tx, "Requesting an immediate Windows restart");
    run_script(
        "reboot",
        "Restart-Computer -Force",
        BTreeMap::new(),
        dry_run,
        tx,
    )?;
    Ok(TaskOutcome::default())
}

fn run_script(
    name: &str,
    script: &str,
    environment: BTreeMap<String, String>,
    dry_run: bool,
    tx: &Sender<TaskEvent>,
) -> Result<String> {
    if dry_run {
        let keys = environment.keys().cloned().collect::<Vec<_>>().join(", ");
        log(tx, &format!("DRY RUN: {name}; environment keys: {keys}"));
        return Ok(String::new());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, script, environment);
        bail!("system-changing tasks are supported only on Windows Server")
    }

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let encoded = encode_powershell(&prepare_powershell_script(script));
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand",
                encoded.as_str(),
            ])
            .envs(environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW);

        log(tx, &format!("[START] {name} / 开始执行"));
        let started = Instant::now();
        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start PowerShell for {name}"))?;
        let stdout = child
            .stdout
            .take()
            .context("failed to capture PowerShell stdout")?;
        let stderr = child
            .stderr
            .take()
            .context("failed to capture PowerShell stderr")?;

        let stdout_tx = tx.clone();
        let stdout_thread =
            std::thread::spawn(move || collect_process_stream(stdout, "", stdout_tx));
        let stderr_tx = tx.clone();
        let stderr_thread =
            std::thread::spawn(move || collect_process_stream(stderr, "PowerShell: ", stderr_tx));

        let mut next_heartbeat = Duration::from_secs(3);
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .with_context(|| format!("failed while waiting for {name}"))?
            {
                break status;
            }
            let elapsed = started.elapsed();
            if elapsed >= next_heartbeat {
                log(
                    tx,
                    &format!("[RUNNING {:>3}s] {name} / 正在执行", elapsed.as_secs()),
                );
                next_heartbeat += Duration::from_secs(5);
            }
            std::thread::sleep(Duration::from_millis(200));
        };

        let stdout = stdout_thread
            .join()
            .map_err(|_| anyhow!("PowerShell stdout reader panicked"))??;
        let _stderr = stderr_thread
            .join()
            .map_err(|_| anyhow!("PowerShell stderr reader panicked"))??;

        if !status.success() {
            return Err(anyhow!("{name} failed with status {status}"));
        }
        log(
            tx,
            &format!(
                "[DONE {:>3}s] {name} / 执行完成",
                started.elapsed().as_secs()
            ),
        );
        Ok(stdout)
    }
}

#[cfg(target_os = "windows")]
fn collect_process_stream<R: Read>(
    stream: R,
    prefix: &str,
    tx: Sender<TaskEvent>,
) -> Result<String> {
    let mut reader = BufReader::new(stream);
    let mut all_bytes = Vec::new();
    loop {
        let mut bytes = Vec::new();
        let count = reader
            .read_until(b'\n', &mut bytes)
            .context("failed to read PowerShell output")?;
        if count == 0 {
            break;
        }
        all_bytes.extend_from_slice(&bytes);
        let line = String::from_utf8_lossy(&bytes).replace('\0', "");
        let line = line.trim_matches(['\r', '\n']);
        if !line.trim().is_empty() {
            log(&tx, &format!("{prefix}{line}"));
        }
    }
    Ok(String::from_utf8_lossy(&all_bytes).replace('\0', ""))
}

#[allow(dead_code)]
fn prepare_powershell_script(script: &str) -> String {
    format!(
        r#"$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [Console]::OutputEncoding
try {{
    & {{
{script}
    }}
}} catch {{
    [Console]::Error.WriteLine(($_ | Out-String).Trim())
    exit 1
}}
"#
    )
}

#[allow(dead_code)]
fn encode_powershell(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    STANDARD.encode(bytes)
}

fn env_map<const N: usize>(values: [(&str, &str); N]) -> BTreeMap<String, String> {
    values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn log(tx: &Sender<TaskEvent>, message: &str) {
    let _ = tx.send(TaskEvent::Log(message.to_owned()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_encoding_is_utf16le_base64() {
        assert_eq!(encode_powershell("A"), "QQA=");
    }

    #[test]
    fn powershell_wrapper_suppresses_progress_and_emits_plain_errors() {
        let script = prepare_powershell_script("Write-Output 'ok'");
        assert!(script.contains("$ProgressPreference = 'SilentlyContinue'"));
        assert!(script.contains("[Console]::Error.WriteLine"));
        assert!(script.contains("Write-Output 'ok'"));
    }

    #[test]
    fn all_features_remain_available() {
        assert_eq!(Feature::ALL.len(), 4);
    }

    #[test]
    fn default_cal_method_does_not_leak_into_runner() {
        let config = AppConfig::default();
        assert_eq!(
            config.rds.cal_install_method,
            crate::config::CalInstallMethod::None
        );
    }

    #[test]
    fn dry_run_does_not_require_real_cal_entitlement() {
        let config = AppConfig::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        execute(Task::RdsLicensing, &config, true, &tx).unwrap();
    }

    #[test]
    fn parses_rds_role_installation_states() {
        let status = parse_rds_role_status(
            "SYSTEM|PartOfDomain|False\n\
             FEATURE|RDS-RD-Server|Installed\n\
             FEATURE|RDS-Licensing|InstallPending\n\
             FEATURE|RDS-Licensing-UI|Available\n",
        )
        .unwrap();
        assert_eq!(status.session_host, RoleInstallState::Installed);
        assert_eq!(status.licensing, RoleInstallState::PendingRestart);
        assert_eq!(status.licensing_ui, RoleInstallState::Missing);
        assert!(!status.part_of_domain);
        assert!(!status.all_present());
        assert!(status.has_pending_restart());
    }

    #[test]
    fn workgroup_per_user_configuration_warns_instead_of_blocking() {
        assert!(scripts::STAGE3_RDS_LICENSING.contains("WARNING: PerUser mode"));
        assert!(!scripts::STAGE3_RDS_LICENSING.contains("does not permit PerUser"));
    }

    #[test]
    fn rds_contact_country_is_normalized_for_the_windows_locale() {
        assert!(scripts::STAGE3_RDS_LICENSING.contains("$uiCulture = (Get-UICulture).Name"));
        assert!(scripts::STAGE3_RDS_LICENSING.contains("$countryRegion = '中国'"));
        assert!(scripts::STAGE3_RDS_LICENSING.contains("CountryRegion = $countryRegion"));
    }

    #[test]
    fn rds_contact_properties_are_committed_individually() {
        assert!(scripts::STAGE3_RDS_LICENSING.contains("$contactProperties.GetEnumerator()"));
        assert!(scripts::STAGE3_RDS_LICENSING.contains("$property[$entry.Key]"));
        assert!(
            !scripts::STAGE3_RDS_LICENSING
                .contains("Set-CimInstance -InputObject $instance -Property @{")
        );
    }
}
