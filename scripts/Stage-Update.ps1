$ErrorActionPreference = 'Stop'
$stream = [IO.File]::OpenRead($env:PF_UPDATE_ZIP)
try {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $actual = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant() }
    finally { $sha.Dispose() }
} finally { $stream.Dispose() }
if ($actual -ne $env:PF_UPDATE_HASH) { throw 'Update checksum mismatch; nothing was installed.' }
if ($env:PF_UPDATE_SETUP -eq '1') {
    [IO.File]::Copy($env:PF_UPDATE_ZIP, $env:PF_UPDATE_STAGED, $false)
} else {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($env:PF_UPDATE_ZIP)
    try {
        $entries = @($zip.Entries | Where-Object { $_.FullName -eq 'PixelForge.exe' })
        if ($entries.Count -ne 1 -or $entries[0].Length -gt 200MB) { throw 'Unexpected update package contents.' }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entries[0], $env:PF_UPDATE_STAGED, $false)
    } finally { $zip.Dispose() }
}
$version = ([Diagnostics.FileVersionInfo]::GetVersionInfo($env:PF_UPDATE_STAGED).ProductVersion).Trim()
if ($version -ne $env:PF_UPDATE_VERSION) { throw 'Update executable version does not match the release.' }
