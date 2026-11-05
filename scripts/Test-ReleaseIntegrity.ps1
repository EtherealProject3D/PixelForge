# Isolated integrity fixture; does not install, launch or sign any application.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path $repoRoot ('target\integrity-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$artifact = Join-Path $fixture 'example.zip'
[IO.File]::WriteAllText($artifact, 'fixture')
& (Join-Path $PSScriptRoot 'Write-ReleaseIntegrity.ps1') -Files @($artifact) -OutputDir $fixture
$report = Get-Content -LiteralPath (Join-Path $fixture 'RELEASE-INTEGRITY.json') -Raw | ConvertFrom-Json
$record = $report.artifacts[0]
if ($record.name -ne 'example.zip' -or $record.sha256 -ne (Get-FileHash $artifact -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Integrity report mismatch' }
if ((Get-Content (Join-Path $fixture 'SHA256SUMS.txt')) -ne "$($record.sha256)  example.zip") { throw 'Checksum manifest mismatch' }
[IO.File]::WriteAllText($artifact, 'tampered')
if ($record.sha256 -eq (Get-FileHash $artifact -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Tampering did not change checksum' }
try {
    & (Join-Path $PSScriptRoot 'Write-ReleaseIntegrity.ps1') -Files @($artifact,$artifact) -OutputDir $fixture
    throw 'Duplicate artifact names accepted'
} catch {
    if ($_.Exception.Message -notlike 'Duplicate artifact name:*') { throw }
}
$unsigned = Join-Path $fixture 'unsigned.exe'
[IO.File]::WriteAllText($unsigned, 'unsigned fixture')
try {
    & (Join-Path $PSScriptRoot 'Write-ReleaseIntegrity.ps1') -Files @($unsigned) -OutputDir $fixture -RequireSigned
    throw 'Unsigned executable accepted in signed-only mode'
} catch {
    if ($_.Exception.Message -notlike 'Signature is not valid:*') { throw }
}
Write-Host 'Integrity fixture passed: checksum roundtrip, tamper detection, duplicate rejection, signature requirement.'
