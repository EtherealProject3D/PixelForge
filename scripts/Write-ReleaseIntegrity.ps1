[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string[]]$Files,
    [Parameter(Mandatory=$true)][string]$OutputDir,
    [switch]$RequireSigned
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$records = @()
$names = @{}
foreach ($file in $Files) {
    $path = (Resolve-Path -LiteralPath $file).Path
    $item = Get-Item -LiteralPath $path
    if ($item.PSIsContainer) { throw "Expected a file: $path" }
    if ($names.ContainsKey($item.Name)) { throw "Duplicate artifact name: $($item.Name)" }
    $names[$item.Name] = $true
    $signature = $null
    if ($item.Extension -in @('.exe', '.dll')) {
        $signature = Get-AuthenticodeSignature -LiteralPath $path
        if ($RequireSigned -and $signature.Status -ne 'Valid') { throw "Signature is not valid: $path" }
    }
    $records += [pscustomobject]@{
        name = $item.Name
        size = $item.Length
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        signature = $(if ($signature) { $signature.Status.ToString() } else { 'NotApplicable' })
        publisher = $(if ($signature -and $signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null })
    }
}
if ($records.Count -eq 0) { throw 'No release files supplied' }
$records = @($records | Sort-Object name)
$records | ForEach-Object { "$($_.sha256)  $($_.name)" } | Set-Content -LiteralPath (Join-Path $OutputDir 'SHA256SUMS.txt') -Encoding ascii
[pscustomobject]@{ schemaVersion=1; generatedUtc=[DateTime]::UtcNow.ToString('o'); artifacts=$records } |
    ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutputDir 'RELEASE-INTEGRITY.json') -Encoding utf8
Write-Host "Wrote SHA-256 checksums and signing report for $($records.Count) artifacts."
