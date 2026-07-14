# FirstSet

> **The first setup tool for Windows Server.**

[English](README.md) | [简体中文](README_CN.md)

FirstSet is a multilingual first-run configuration tool for Windows Server. It wraps
error-prone PowerShell operations in a repeatable Rust GUI with live execution logs.
The interface is available in English and Simplified Chinese.

> FirstSet does not bypass Microsoft licensing. Windows activation information, RDS
> CALs, volume licensing agreement numbers, and Key Pack IDs must be obtained legally
> by the operator. A workgroup server can be configured for Per User mode, but RD
> Licensing cannot track or report CAL usage for local users. FirstSet displays a
> warning without blocking the configuration. Per Device remains Microsoft's supported
> licensing mode for workgroup deployments.

## Features

- **Base access**: optionally configure Windows KMS activation, OpenSSH, and RDP;
  automatically maintain the linked TCP 22 and TCP/UDP 3389 Windows Firewall rules;
  and display active adapter IP, MAC, gateway, and DNS information.
- **RDS roles**: detect the current status of RD Session Host, RD Licensing, and the
  licensing management tools; install missing components and request a restart when
  Windows reports that one is required.
- **RDS licensing**: detect domain or workgroup membership, activate the RD License
  Server, configure the licensing mode and server, and install legitimately purchased
  RDS CALs. Per User mode on a workgroup server produces a warning rather than a block.
- **Local users**: create numbered local accounts such as `user01` through `user20`, add
  them to the built-in Remote Desktop Users group, and write the initial credentials to
  the administrator's desktop.
- All four functions are independent and may be used in any order according to the
  current server state.
- Every system-changing function supports dry-run mode. Configuration values are passed
  to PowerShell through the child process environment and are not exposed on the command
  line.
- The selected interface language is saved in `config.toml`.
- On Windows, `config.toml` and generated credential files are restricted to
  Administrators and SYSTEM.

## Getting started

1. Download and extract the Windows x64 artifact from GitHub Actions or a release.
2. Copy `config.example.toml` to `config.toml` beside `FirstSet.exe`, or enter the values
   in the GUI on first launch. Never commit `config.toml`.
3. Double-click `FirstSet.exe` and approve the Windows UAC prompt with an administrator
   account.
4. Keep **Dry run** enabled while reviewing each function. Disable it only after the
   configuration has been verified.
5. If RDS role installation reports that a restart is required, use the restart button
   before configuring licensing.
6. After creating local users, distribute their credentials securely and delete the
   plaintext credential file from the desktop.

An alternative configuration file can be supplied explicitly:

```powershell
.\FirstSet.exe --config D:\secure\config.toml
```

Default file locations:

- Configuration: `config.toml` beside the executable.
- Generated credentials: `firstset-users-*.txt` on the current administrator's desktop.

## RDS CAL licensing

FirstSet supports the two Microsoft licensing installation paths exposed by the Windows
RDS licensing provider:

- `Agreement`: a legitimate seven-digit volume licensing agreement or enrollment
  number, agreement type, CAL quantity, and product version.
- `KeyPackId`: a 35-character Key Pack ID obtained through the Microsoft Clearinghouse
  online or telephone process.

Without legitimate CAL information, leave `cal_install_method` set to `None`. FirstSet
will refuse to install CALs. Installing the Windows Server RDS roles does not grant 20
users the right to run concurrent sessions without the required CAL entitlement.

Product version IDs are `5` for Windows Server 2016, `6` for Windows Server 2019, and
`7` for Windows Server 2022. Match the value to both the purchased CAL version and the
target Windows Server version.

Microsoft references:

- [License Remote Desktop session hosts](https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-license-session-hosts)
- [RDS Client Access Licenses](https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-client-access-license)
- [Win32_TSLicenseKeyPack](https://learn.microsoft.com/windows/win32/termserv/win32-tslicensekeypack)

## Building from source

Rust 1.92 or later is required.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Release builds for Windows embed a `requireAdministrator` application manifest. System
configuration scripts live in `assets/scripts/` and remain compatible with Windows
PowerShell 5.1. The GUI uses Direct3D 12 through WGPU so that it can run with the remote
display driver after RDS roles are enabled, with Windows WARP available as a software
rendering fallback.

## Security boundaries

- Firewall source addresses default to `Any`. For production, restrict them to a VPN or
  VPC subnet, or a fixed administration egress address.
- FirstSet does not modify cloud-provider security groups. Ports 22 and 3389 must still
  be allowed there for the same trusted sources.
- FirstSet does not create RD Gateway, Connection Broker, high availability, or load
  balancing. Actual capacity for 20 or more sessions depends on CPU, memory, storage
  IOPS, and application compatibility with multi-session environments.
- Never commit real product keys, agreement numbers, CAL identifiers, user passwords,
  private keys, or sensitive `config.toml` values to a public repository.

## License

FirstSet is available under the MIT License. See [LICENSE-MIT](LICENSE-MIT).
