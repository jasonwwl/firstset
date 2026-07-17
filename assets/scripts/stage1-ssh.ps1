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

# The inbox OpenSSH service normally runs as LocalSystem. Merely changing its
# startup type does not repair a service whose logon account was modified, and
# such a service can appear healthy while dropping connections during user
# authentication. Restore the complete known-good service identity every time.
$service = Get-Service -Name 'sshd' -ErrorAction Stop
if ($service.Status -ne 'Stopped') {
    Stop-Service -Name 'sshd' -Force
    $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped,
        [TimeSpan]::FromSeconds(15))
}

$null = & sc.exe config sshd obj= LocalSystem start= auto 2>&1
$scExitCode = $LASTEXITCODE
if ($scExitCode -ne 0) {
    throw "Failed to restore the OpenSSH service account and startup type (sc.exe exit code $scExitCode)."
}
Write-Output 'OpenSSH service identity and startup type restored.'

# Starting once also lets a fresh OpenSSH installation create its host keys and
# default configuration before FirstSet updates and validates that configuration.
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

$service = Get-Service -Name 'sshd'
$service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running,
    [TimeSpan]::FromSeconds(15))
$deadline = [DateTime]::UtcNow.AddSeconds(15)
do {
    $serviceDetails = Get-CimInstance Win32_Service -Filter "Name='sshd'"
    $listener = Get-NetTCPConnection -State Listen -LocalPort 22 -ErrorAction SilentlyContinue |
        Where-Object { $_.OwningProcess -eq $serviceDetails.ProcessId } |
        Select-Object -First 1
    if ($null -ne $listener) {
        break
    }
    Start-Sleep -Milliseconds 250
} while ([DateTime]::UtcNow -lt $deadline)

if ($serviceDetails.State -ne 'Running') {
    throw "OpenSSH service verification failed: unexpected state '$($serviceDetails.State)'."
}
if ($serviceDetails.StartMode -ne 'Auto') {
    throw "OpenSSH service verification failed: unexpected start mode '$($serviceDetails.StartMode)'."
}
if ($serviceDetails.StartName -notin @('LocalSystem', 'NT AUTHORITY\SYSTEM')) {
    throw "OpenSSH service verification failed: unexpected service account '$($serviceDetails.StartName)'."
}
if ($null -eq $listener) {
    throw 'OpenSSH service verification failed: sshd is not listening on TCP 22.'
}

Write-Output "OpenSSH verified: service account=$($serviceDetails.StartName), startup=Automatic, state=Running, TCP 22=Listening."
Write-Output 'The linked TCP 22 Windows Firewall rule is enabled.'
