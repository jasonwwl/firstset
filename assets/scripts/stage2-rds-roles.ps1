$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module ServerManager

$features = @('RDS-RD-Server', 'RDS-Licensing', 'RDS-Licensing-UI')
Write-Output 'Checking current RDS role installation state...'
$featureStates = @(Get-WindowsFeature -Name $features)
$missing = @($featureStates |
    Where-Object InstallState -notin @('Installed', 'InstallPending') |
    Select-Object -ExpandProperty Name)

$restartRequired = @($featureStates | Where-Object InstallState -eq 'InstallPending').Count -gt 0
if ($missing.Count -gt 0) {
    Write-Output "Installing RDS roles: $($missing -join ', ')"
    $result = Install-WindowsFeature -Name $missing -IncludeManagementTools `
        -WarningAction SilentlyContinue
    if (-not $result.Success) {
        throw 'One or more RDS roles failed to install.'
    }
    $restartRequired = $restartRequired -or $result.RestartNeeded -ne 'No'
    Write-Output 'Windows completed the RDS role installation command.'
} else {
    Write-Output 'All requested RDS roles are already installed or pending restart.'
}

Write-Output 'Checking the RD Licensing service...'
$licenseService = Get-Service TermServLicensing -ErrorAction SilentlyContinue
if ($null -ne $licenseService) {
    Set-Service TermServLicensing -StartupType Automatic
    if ($licenseService.Status -ne 'Running') {
        Start-Service TermServLicensing
    }
} else {
    # Windows does not register this service until pending role installation
    # has been completed by a reboot. This is expected, not an install failure.
    $restartRequired = $true
    Write-Output 'RD Licensing service is pending registration and will be available after restart.'
}

$featureStates = @(Get-WindowsFeature -Name $features)
if (@($featureStates | Where-Object InstallState -eq 'InstallPending').Count -gt 0) {
    $restartRequired = $true
}

Write-Output "Installed roles: $($features -join ', ')"
Write-Output "RESTART_REQUIRED=$([int]$restartRequired)"
