# Contributor guidance

- This is a Windows Server provisioning tool. Keep destructive operations explicit and visible in the UI.
- Never add public KMS endpoints, shared licensing agreement numbers, CAL bypasses, or real credentials.
- Every system-changing action must support dry-run and produce an audit log.
- Address built-in Windows groups by SID, not localized display name.
- Keep PowerShell scripts compatible with Windows PowerShell 5.1 and validate them before release.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` before merging.
- Do not log product keys, CAL identifiers, passwords, or private key material.
