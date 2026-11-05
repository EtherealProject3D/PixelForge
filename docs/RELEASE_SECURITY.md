# Windows trust and release integrity

## Evidence collected on 2026-11-04

The installed stable executable and the available local v0.6.1 setup both report
NotSigned through Get-AuthenticodeSignature. A read-only Get-MpThreatDetection
query found zero entries whose resources matched PixelForge / Pixel Forge.
This does not establish what the earlier warning was, nor prove that a file is
safe. The exact warning text, filename, SHA-256, detection name and Windows
Protection History entry are still needed to classify that original report.
No Defender settings, exclusions, SmartScreen settings or quarantine items were
changed. No installed application was replaced.

## Distinguish the warning

SmartScreen's unrecognized application warning concerns download / publisher
reputation. Defender Antivirus detections identify a suspected threat and can
quarantine a file. An unsigned low-volume release can trigger reputation warnings
without a Defender malware detection; signing is not proof of a clean binary.
For an actual detection, keep protection enabled, record its threat name and file
hash, and submit the exact release artifact to Microsoft Security Intelligence:
https://www.microsoft.com/en-us/wdsi/filesubmission
Do not restore or whitelist a detected binary simply to complete testing.

## Integrity tooling

The release workflow already hashed ZIP and setup packages in v0.6.2. v0.6.3
extends that with a reusable local build step: Write-ReleaseIntegrity.ps1 emits
SHA256SUMS.txt and RELEASE-INTEGRITY.json for the installer, portable ZIP and
application executable. The report includes size, hash, Authenticode status and
publisher. Local installer builds now emit the same report. -RequireSigned fails
if an executable is not validly signed. The fixture checks manifest output,
tamper detection, duplicate names and rejection of unsigned executables.
The executable embeds publisher / product / file metadata.

Compare a download's Get-FileHash -Algorithm SHA256 result with its exact entry
in SHA256SUMS.txt obtained from the official GitHub release. A matching hash
detects corruption / changes relative to the manifest. It does not authenticate
a manifest supplied by an attacker; use trusted release provenance and valid
publisher signatures when available. Existing in-app updates already verify
published checksums before staging the executable or setup.

Tag-triggered CI now creates a DRAFT release. It does not mark a development build
as latest or publish it. The user approved final v0.6.3 publication on 2026-11-04. Draft artifacts must still be verified before publishing.

## Signing evaluation and proposed adoption

Microsoft recommends Artifact Signing (formerly Trusted Signing) for distribution
outside the Store. Alternatives include an organizational OV certificate on secure
hardware or Store/MSIX distribution. Self-signed certificates do not establish
public publisher trust. EV certificates no longer automatically remove SmartScreen
warnings, so paying for EV solely for that purpose is not justified. Signed fresh
files can still receive warnings until reputation accumulates.
https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
https://learn.microsoft.com/en-us/azure/artifact-signing/overview

No signing identity or certificate was provisioned; builds remain honestly reported
as unsigned. Adoption requires a verified publisher account / eligibility, budget,
protected CI credentials and a timestamping policy. Do not put private keys in Git.
Sign and RFC3161-timestamp PixelForge.exe before creating the ZIP / setup; sign the
installer and configure Inno Setup's signed uninstaller support as part of the
signing rollout. Verify every shipped executable and chain before generating final
hashes; changing signatures changes hashes. Enable -RequireSigned in production CI
only after the full signing path is configured. Upload those exact hashed artifacts,
submit false positives for review, and retain the preceding stable release.
Signing configuration and an actual fresh-download SmartScreen test remain release
validation tasks, rather than a promise that warnings have been eliminated.
