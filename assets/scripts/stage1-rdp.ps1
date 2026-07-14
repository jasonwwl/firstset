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

$remoteAddresses = @($env:FIRSTSET_ALLOWED_REMOTE_ADDRESSES -split ',' | ForEach-Object { $_.Trim() })
$terminalServer = 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server'
$rdpTcp = Join-Path $terminalServer 'WinStations\RDP-Tcp'
Set-ItemProperty -Path $terminalServer -Name fDenyTSConnections -Type DWord -Value 0
Set-ItemProperty -Path $rdpTcp -Name UserAuthentication -Type DWord -Value 1

Set-InboundRule -Name 'FirstSet-RDP-In-TCP' -DisplayName 'FirstSet Remote Desktop (TCP-In)' `
    -Protocol TCP -LocalPort 3389 -RemoteAddress $remoteAddresses
Set-InboundRule -Name 'FirstSet-RDP-In-UDP' -DisplayName 'FirstSet Remote Desktop (UDP-In)' `
    -Protocol UDP -LocalPort 3389 -RemoteAddress $remoteAddresses

Start-Service TermService
Write-Output 'RDP and NLA are enabled; TCP/UDP 3389 are allowed by Windows Firewall.'
