$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$adapters = @(Get-NetAdapter | Where-Object Status -eq 'Up' | Sort-Object ifIndex)
if ($adapters.Count -eq 0) {
    Write-Output 'No active network adapter was found.'
    exit 0
}

foreach ($adapter in $adapters) {
    $configuration = Get-NetIPConfiguration -InterfaceIndex $adapter.ifIndex
    $addresses = @($configuration.IPv4Address | ForEach-Object {
        "$($_.IPAddress)/$($_.PrefixLength)"
    })
    $gateways = @($configuration.IPv4DefaultGateway | ForEach-Object NextHop)
    $dnsServers = @($configuration.DNSServer.ServerAddresses)
    Write-Output ("Adapter: {0}" -f $adapter.Name)
    Write-Output ("  Description: {0}" -f $adapter.InterfaceDescription)
    Write-Output ("  MAC: {0}" -f $adapter.MacAddress)
    Write-Output ("  IPv4: {0}" -f ($addresses -join ', '))
    Write-Output ("  Gateway: {0}" -f ($gateways -join ', '))
    Write-Output ("  DNS: {0}" -f ($dnsServers -join ', '))
    Write-Output ("  Link speed: {0}" -f $adapter.LinkSpeed)
}
