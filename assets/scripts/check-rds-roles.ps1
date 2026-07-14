$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module ServerManager

$computerSystem = Get-CimInstance Win32_ComputerSystem
Write-Output "SYSTEM|PartOfDomain|$($computerSystem.PartOfDomain)"

$featureNames = @('RDS-RD-Server', 'RDS-Licensing', 'RDS-Licensing-UI')
foreach ($featureName in $featureNames) {
    $feature = Get-WindowsFeature -Name $featureName
    if ($null -eq $feature) {
        Write-Output "FEATURE|$featureName|Unknown"
    } else {
        Write-Output "FEATURE|$($feature.Name)|$($feature.InstallState)"
    }
}
