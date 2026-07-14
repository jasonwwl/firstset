$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$productKey = $env:FIRSTSET_WINDOWS_PRODUCT_KEY
$kmsServer = $env:FIRSTSET_KMS_SERVER
if ([string]::IsNullOrWhiteSpace($productKey) -or [string]::IsNullOrWhiteSpace($kmsServer)) {
    throw 'Both FIRSTSET_WINDOWS_PRODUCT_KEY and FIRSTSET_KMS_SERVER are required.'
}

$slmgr = Join-Path $env:WINDIR 'System32\slmgr.vbs'
foreach ($arguments in @(
    @('/ipk', $productKey),
    @('/skms', $kmsServer),
    @('/ato'),
    @('/xpr')
)) {
    & cscript.exe //Nologo $slmgr @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "slmgr failed with exit code $LASTEXITCODE."
    }
}

Write-Output 'Windows activation completed.'
