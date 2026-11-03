#ifndef AppVersion
  #error AppVersion must be supplied by Build-Installer.ps1
#endif
#ifndef SourceDir
  #error SourceDir must be supplied by Build-Installer.ps1
#endif
#ifndef OutputDir
  #define OutputDir "..\target\installer"
#endif
#ifndef InstallerId
  #define InstallerId "EtherealProject3D.PixelForge"
#endif

[Setup]
AppId={#InstallerId}
AppName=PixelForge
AppVersion={#AppVersion}
AppPublisher=EtherealProject3D
AppPublisherURL=https://github.com/EtherealProject3D/PixelForge
AppSupportURL=https://github.com/EtherealProject3D/PixelForge/issues
AppUpdatesURL=https://github.com/EtherealProject3D/PixelForge/releases
DefaultDirName={autopf}\Pixel Forge
DefaultGroupName=PixelForge
DisableProgramGroupPage=yes
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#OutputDir}
#ifdef SmokeTest
OutputBaseFilename=PixelForge-v{#AppVersion}-windows-x64-smoke-setup
#else
OutputBaseFilename=PixelForge-v{#AppVersion}-windows-x64-setup
#endif
SetupIconFile=..\assets\pixelforge.ico
UninstallDisplayIcon={app}\PixelForge.exe
VersionInfoVersion={#AppVersion}
VersionInfoProductVersion={#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
WizardSizePercent=110
CloseApplications=yes
RestartApplications=no
UsePreviousTasks=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceDir}\PixelForge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "installed.txt"; DestDir: "{app}"; DestName: "pixelforge-installed.txt"; Flags: ignoreversion

[Icons]
#ifdef SmokeTest
Name: "{app}\Test Shortcuts\Start Menu\PixelForge"; Filename: "{app}\PixelForge.exe"; WorkingDir: "{app}"
Name: "{app}\Test Shortcuts\Desktop\PixelForge"; Filename: "{app}\PixelForge.exe"; WorkingDir: "{app}"; Tasks: desktopicon
#else
Name: "{group}\PixelForge"; Filename: "{app}\PixelForge.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\PixelForge"; Filename: "{app}\PixelForge.exe"; WorkingDir: "{app}"; Tasks: desktopicon
#endif

[Run]
Filename: "{app}\PixelForge.exe"; Description: "Open PixelForge"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallDelete]
Type: files; Name: "{app}\PixelForge.exe.previous"

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if FileExists(ExpandConstant('{app}\PixelForge.exe')) then
    if not FileCopy(ExpandConstant('{app}\PixelForge.exe'), ExpandConstant('{app}\PixelForge.exe.previous'), False) then
      Result := 'Could not retain the previous version. Close PixelForge and try again.';
end;
