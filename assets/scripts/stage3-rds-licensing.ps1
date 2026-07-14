$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module ServerManager

$requiredFeatures = @('RDS-RD-Server', 'RDS-Licensing')
$missing = @(Get-WindowsFeature -Name $requiredFeatures |
    Where-Object InstallState -ne 'Installed' |
    Select-Object -ExpandProperty Name)
if ($missing.Count -gt 0) {
    throw "RDS roles are not fully installed. Restart Windows first if installation is pending. Missing: $($missing -join ', ')."
}

$mode = $env:FIRSTSET_RDS_LICENSING_MODE
$computerSystem = Get-CimInstance Win32_ComputerSystem
if (-not $computerSystem.PartOfDomain -and $mode -eq 'PerUser') {
    Write-Output 'WARNING: PerUser mode is being configured on a workgroup server. Local users cannot be tracked or reported by RD Licensing; the administrator remains responsible for CAL entitlement.'
}
$modeValue = if ($mode -eq 'PerDevice') { 2 } elseif ($mode -eq 'PerUser') { 4 } else {
    throw "Unsupported RDS licensing mode: $mode"
}
$productType = if ($mode -eq 'PerDevice') { 0 } else { 1 }
$licenseServer = $env:FIRSTSET_RDS_LICENSE_SERVER
if ($licenseServer -eq 'localhost' -or [string]::IsNullOrWhiteSpace($licenseServer)) {
    $licenseServer = $env:COMPUTERNAME
}

$policyPath = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\Terminal Services'
New-Item -Force -Path $policyPath | Out-Null
New-ItemProperty -Path $policyPath -Name LicensingMode -PropertyType DWord `
    -Value $modeValue -Force | Out-Null
New-ItemProperty -Path $policyPath -Name LicenseServers -PropertyType String `
    -Value $licenseServer -Force | Out-Null

$licenseService = Get-Service TermServLicensing -ErrorAction SilentlyContinue
if ($null -eq $licenseService) {
    throw 'RD Licensing service is unavailable. Restart Windows, then try RDS licensing again.'
}
Set-Service TermServLicensing -StartupType Automatic
if ($licenseService.Status -ne 'Running') {
    Start-Service TermServLicensing
}

$activation = Invoke-CimMethod -Namespace root/CIMV2 -ClassName Win32_TSLicenseServer `
    -MethodName GetActivationStatus
if ($activation.ReturnValue -ne 0) {
    throw "Unable to query RD License Server activation. WMI code: $($activation.ReturnValue)."
}

if ($activation.ActivationStatus -ne 0) {
    $requiredContact = @(
        $env:FIRSTSET_RDS_CONTACT_FIRST_NAME,
        $env:FIRSTSET_RDS_CONTACT_LAST_NAME,
        $env:FIRSTSET_RDS_CONTACT_COMPANY,
        $env:FIRSTSET_RDS_CONTACT_COUNTRY
    )
    if (@($requiredContact | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -gt 0) {
        throw 'First name, last name, company, and country are required to activate the license server.'
    }

    # The RD Licensing WMI provider validates CountryRegion against its own
    # localized country list. On zh-CN it rejects both "China" and RegionInfo's
    # "中华人民共和国", but accepts the shorter UI label "中国".
    $countryRegion = $env:FIRSTSET_RDS_CONTACT_COUNTRY.Trim()
    if ($countryRegion -in @('China', 'CN', '中国')) {
        $uiCulture = (Get-UICulture).Name
        if ($uiCulture -in @('zh-CN', 'zh-SG')) {
            $countryRegion = '中国'
        } elseif ($uiCulture -in @('zh-TW', 'zh-HK', 'zh-MO')) {
            $countryRegion = '中國'
        } else {
            $countryRegion = 'China'
        }
    }

    # This legacy dynamic WMI provider can reject a multi-property CIM update
    # with WBEM_E_INVALID_PARAMETER even though every property is writable.
    # Commit one property at a time and omit blank optional fields.
    $contactProperties = [ordered]@{
        FirstName = $env:FIRSTSET_RDS_CONTACT_FIRST_NAME
        LastName = $env:FIRSTSET_RDS_CONTACT_LAST_NAME
        Company = $env:FIRSTSET_RDS_CONTACT_COMPANY
        CountryRegion = $countryRegion
        eMail = $env:FIRSTSET_RDS_CONTACT_EMAIL
        OrgUnit = $env:FIRSTSET_RDS_CONTACT_ORG_UNIT
        Address = $env:FIRSTSET_RDS_CONTACT_ADDRESS
        City = $env:FIRSTSET_RDS_CONTACT_CITY
        State = $env:FIRSTSET_RDS_CONTACT_STATE
        PostalCode = $env:FIRSTSET_RDS_CONTACT_POSTAL_CODE
    }
    foreach ($entry in $contactProperties.GetEnumerator()) {
        if ([string]::IsNullOrWhiteSpace([string]$entry.Value)) {
            continue
        }
        $instance = Get-CimInstance -Namespace root/CIMV2 -ClassName Win32_TSLicenseServer
        $property = @{}
        $property[$entry.Key] = $entry.Value
        try {
            Set-CimInstance -InputObject $instance -Property $property -ErrorAction Stop | Out-Null
        } catch {
            throw "Failed to set RD License Server contact property $($entry.Key): $($_.Exception.Message)"
        }
    }
    Write-Output 'RD License Server contact information configured.'

    $activation = Invoke-CimMethod -Namespace root/CIMV2 -ClassName Win32_TSLicenseServer `
        -MethodName ActivateServerAutomatic
    if ($activation.ReturnValue -ne 0 -or $activation.ActivationStatus -ne 0) {
        throw "RD License Server activation failed. WMI code: $($activation.ReturnValue); status: $($activation.ActivationStatus)."
    }
    Write-Output 'RD License Server activated.'
} else {
    Write-Output 'RD License Server was already activated.'
}

$productVersion = [uint32]$env:FIRSTSET_RDS_PRODUCT_VERSION
$calCount = [uint32]$env:FIRSTSET_RDS_CAL_COUNT
$packs = @(Get-CimInstance -Namespace root/CIMV2 -ClassName Win32_TSLicenseKeyPack |
    Where-Object {
        [int]$_.ProductVersionID -eq $productVersion -and
        [int]$_.ProductType -eq $productType
    })
$installedCount = if ($packs.Count -eq 0) { 0 } else {
    [int](($packs | Measure-Object TotalLicenses -Sum).Sum)
}

if ($installedCount -ge $calCount) {
    Write-Output "Compatible CAL capacity already installed: $installedCount."
    exit 0
}

$method = $env:FIRSTSET_RDS_CAL_METHOD
if ($method -eq 'KeyPackId') {
    $keyPackId = $env:FIRSTSET_RDS_KEY_PACK_ID -replace '[\s-]', ''
    if ($keyPackId -notmatch '^[A-Za-z0-9]{35}$') {
        throw 'Microsoft Clearinghouse Key Pack ID must contain 35 alphanumeric characters.'
    }
    $install = Invoke-CimMethod -Namespace root/CIMV2 -ClassName Win32_TSLicenseKeyPack `
        -MethodName InstallLicenseKeyPack -Arguments @{ sLicenseKeyPackId = $keyPackId }
} elseif ($method -eq 'Agreement') {
    $agreementNumber = $env:FIRSTSET_RDS_AGREEMENT_NUMBER
    if ($agreementNumber -notmatch '^\d{7}$') {
        throw 'The legitimate Microsoft agreement/enrollment number must contain seven digits.'
    }
    $agreementTypes = @{
        Select = 0; Enterprise = 1; Campus = 2; School = 3; ServiceProvider = 4; Other = 5
    }
    $agreementType = $agreementTypes[$env:FIRSTSET_RDS_AGREEMENT_TYPE]
    if ($null -eq $agreementType) {
        throw "Unsupported Microsoft agreement type: $($env:FIRSTSET_RDS_AGREEMENT_TYPE)."
    }
    $install = Invoke-CimMethod -Namespace root/CIMV2 -ClassName Win32_TSLicenseKeyPack `
        -MethodName InstallAgreementLicenseKeyPack -Arguments @{
            AgreementType = [uint32]$agreementType
            sAgreementNumber = $agreementNumber
            ProductVersion = $productVersion
            ProductType = [uint32]$productType
            LicenseCount = $calCount
        }
} else {
    throw "Unsupported RDS CAL install method: $method"
}

if ($install.ReturnValue -ne 0) {
    throw "Microsoft Clearinghouse rejected the CAL request. WMI code: $($install.ReturnValue)."
}
Write-Output 'RDS CAL pack installed successfully.'
Write-Output "Licensing mode: $mode; license server: $licenseServer; requested CALs: $calCount."
