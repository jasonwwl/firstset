$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-CryptoIndex {
    param([Parameter(Mandatory)] [int]$Maximum)
    $bytes = New-Object byte[] 4
    $script:Rng.GetBytes($bytes)
    return [int]([BitConverter]::ToUInt32($bytes, 0) % [uint32]$Maximum)
}

function New-InitialPassword {
    param([Parameter(Mandatory)] [int]$Length)
    $upper = 'ABCDEFGHJKLMNPQRSTUVWXYZ'.ToCharArray()
    $lower = 'abcdefghijkmnopqrstuvwxyz'.ToCharArray()
    $digits = '23456789'.ToCharArray()
    $special = '!@#$%*-_=+'.ToCharArray()
    $all = @($upper + $lower + $digits + $special)
    $characters = New-Object System.Collections.Generic.List[char]
    foreach ($set in @($upper, $lower, $digits, $special)) {
        $characters.Add($set[(Get-CryptoIndex -Maximum $set.Count)])
    }
    while ($characters.Count -lt $Length) {
        $characters.Add($all[(Get-CryptoIndex -Maximum $all.Count)])
    }
    for ($index = $characters.Count - 1; $index -gt 0; $index--) {
        $swap = Get-CryptoIndex -Maximum ($index + 1)
        $value = $characters[$index]
        $characters[$index] = $characters[$swap]
        $characters[$swap] = $value
    }
    return -join $characters
}

$count = [int]$env:FIRSTSET_USER_COUNT
$prefix = $env:FIRSTSET_USER_PREFIX
$start = [int]$env:FIRSTSET_USER_START_INDEX
$width = [int]$env:FIRSTSET_USER_NUMBER_WIDTH
$passwordLength = [int]$env:FIRSTSET_USER_PASSWORD_LENGTH
$resetExisting = $env:FIRSTSET_RESET_EXISTING_PASSWORDS -eq '1'
if ($count -lt 1 -or $count -gt 200) { throw 'User count must be between 1 and 200.' }
if ($passwordLength -lt 14) { throw 'Password length must be at least 14.' }

$script:Rng = New-Object System.Security.Cryptography.RNGCryptoServiceProvider
$rdpGroup = Get-LocalGroup -SID 'S-1-5-32-555'
$rows = New-Object System.Collections.Generic.List[object]

try {
    for ($offset = 0; $offset -lt $count; $offset++) {
        $number = $start + $offset
        $username = '{0}{1}' -f $prefix, $number.ToString("D$width")
        if ($username.Length -gt 20) { throw "Local username exceeds 20 characters: $username" }

        $existing = Get-LocalUser -Name $username -ErrorAction SilentlyContinue
        $password = '<unchanged>'
        $status = 'Existing'
        if ($null -eq $existing -or $resetExisting) {
            $password = New-InitialPassword -Length $passwordLength
            $securePassword = ConvertTo-SecureString $password -AsPlainText -Force
            if ($null -eq $existing) {
                $existing = New-LocalUser -Name $username -Password $securePassword `
                    -AccountNeverExpires -PasswordNeverExpires `
                    -Description 'FirstSet remote desktop user'
                $status = 'Created'
            } else {
                Set-LocalUser -Name $username -Password $securePassword
                $status = 'Password reset'
            }
        }

        $member = @(Get-LocalGroupMember -Group $rdpGroup.Name -ErrorAction SilentlyContinue |
            Where-Object { $_.SID.Value -eq $existing.SID.Value })
        if ($member.Count -eq 0) {
            Add-LocalGroupMember -Group $rdpGroup.Name -Member $username
        }

        $rows.Add([PSCustomObject]@{
            Username = $username
            InitialPassword = $password
            Status = $status
            Group = $rdpGroup.Name
        })
        Write-Output "$status user: $username"
    }
} finally {
    $script:Rng.Dispose()
}

$desktop = [Environment]::GetFolderPath('Desktop')
if ([string]::IsNullOrWhiteSpace($desktop)) { $desktop = Join-Path $env:USERPROFILE 'Desktop' }
New-Item -ItemType Directory -Force -Path $desktop | Out-Null
$file = Join-Path $desktop "firstset-users-$(Get-Date -Format 'yyyyMMdd-HHmmss').txt"
$header = @(
    'FirstSet Initial Credentials',
    "Server: $env:COMPUTERNAME",
    "Generated: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')",
    'SECURITY: Delete this file after credentials are delivered securely.',
    ''
)
$body = $rows | Format-Table -AutoSize | Out-String -Width 220
Set-Content -Path $file -Value ($header + $body) -Encoding UTF8
& icacls.exe $file /inheritance:r | Out-Null
& icacls.exe $file /grant '*S-1-5-32-544:F' '*S-1-5-18:F' | Out-Null
Write-Output "CREDENTIAL_FILE=$file"
