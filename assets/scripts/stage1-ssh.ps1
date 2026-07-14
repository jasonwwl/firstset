$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Set-InboundRule {
    param(
        [Parameter(Mandatory)] [string]$Name,
        [Parameter(Mandatory)] [string]$DisplayName,
        [Parameter(Mandatory)] [string]$Protocol,
        [Parameter(Mandatory)] [int]$LocalPort,
        [Parameter(Mandatory)] [string[]]$RemoteAddress
    )

    $rule = Get-NetFirewallRule -Name $Name -ErrorAction SilentlyContinue
    if ($null -eq $rule) {
        New-NetFirewallRule -Name $Name -DisplayName $DisplayName -Enabled True `
            -Profile Any -Direction Inbound -Protocol $Protocol -Action Allow `
            -LocalPort $LocalPort -RemoteAddress $RemoteAddress | Out-Null
        return
    }
    Set-NetFirewallRule -Name $Name -NewDisplayName $DisplayName -Enabled True `
        -Profile Any -Direction Inbound -Action Allow | Out-Null
    $rule | Get-NetFirewallPortFilter |
        Set-NetFirewallPortFilter -Protocol $Protocol -LocalPort $LocalPort | Out-Null
    $rule | Get-NetFirewallAddressFilter |
        Set-NetFirewallAddressFilter -RemoteAddress $RemoteAddress | Out-Null
}

$publicKey = $env:FIRSTSET_ADMIN_PUBLIC_KEY
if ($publicKey -notmatch '^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(256|384|521))\s+\S+') {
    throw 'FIRSTSET_ADMIN_PUBLIC_KEY is not a supported single-line SSH public key.'
}
$remoteAddresses = @($env:FIRSTSET_ALLOWED_REMOTE_ADDRESSES -split ',' | ForEach-Object { $_.Trim() })

$capability = Get-WindowsCapability -Online -Name 'OpenSSH.Server~~~~0.0.1.0'
if ($capability.State -ne 'Installed') {
    Add-WindowsCapability -Online -Name 'OpenSSH.Server~~~~0.0.1.0' | Out-Null
}

Set-Service sshd -StartupType Automatic
Start-Service sshd
Set-InboundRule -Name 'OpenSSH-Server-In-TCP' -DisplayName 'OpenSSH Server (sshd)' `
    -Protocol TCP -LocalPort 22 -RemoteAddress $remoteAddresses

$sshDirectory = Join-Path $env:ProgramData 'ssh'
$authorizedKeys = Join-Path $sshDirectory 'administrators_authorized_keys'
$sshdConfig = Join-Path $sshDirectory 'sshd_config'
New-Item -ItemType Directory -Force -Path $sshDirectory | Out-Null
Set-Content -Path $authorizedKeys -Value $publicKey -Encoding ascii
& icacls.exe $authorizedKeys /inheritance:r | Out-Null
& icacls.exe $authorizedKeys /grant '*S-1-5-32-544:F' '*S-1-5-18:F' | Out-Null

$configText = Get-Content -Raw -Path $sshdConfig
if ($configText -notmatch '(?im)^\s*Match\s+Group\s+administrators\s*$') {
    Add-Content -Path $sshdConfig -Encoding ascii -Value @"

Match Group administrators
    AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys
"@
}

& "$env:WINDIR\System32\OpenSSH\sshd.exe" -t
if ($LASTEXITCODE -ne 0) {
    throw 'OpenSSH configuration validation failed.'
}
Restart-Service sshd
Write-Output 'OpenSSH is running and TCP 22 is allowed by Windows Firewall.'
