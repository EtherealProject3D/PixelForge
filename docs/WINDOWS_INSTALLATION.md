# Windows installation options

## Installer

Run **PixelForge-v0.6.1-windows-x64-setup.exe**. Windows requests administrator permission. The default destination is **C:\Program Files\Pixel Forge** on a standard Windows installation.

Setup adds a **PixelForge** entry to the Start menu and offers a **Create a desktop shortcut** checkbox, initially unchecked. Updating an existing installation keeps its destination and shortcut choices. The finish page can open PixelForge using the original user's permissions.

Use Windows **Installed apps** to uninstall. The uninstaller removes application files and shortcuts and asks whether to remove downloaded AI models and dependencies. Choose No (the default) to keep downloads for reinstalling, or Yes to remove the current user's `%LOCALAPPDATA%\PixelForge\deps` folder, including managed ComfyUI, models, engines, caches, and rollback versions. These downloads are shared with PixelForge Dev and other copies; close them first. Projects, settings, and separately installed ComfyUI are retained. Silent uninstall always keeps downloads. Downloads belonging to other Windows users are not removed.

## Portable ZIP

Extract the whole ZIP into a writable folder and run **PixelForge.exe**. No installer, administrator permission, or automatic shortcuts are required. Create a shortcut manually if desired.

## Updates

Both distributions use **Updates > Check for Updates**. Installed copies verify and open the newer setup package, requesting administrator permission through Windows. Portable copies verify the ZIP and replace their executable directly after shutdown. The previous executable is retained as **PixelForge.exe.previous** for installed copies (or the original executable name plus `.previous` for portable copies).

If administrator permission is cancelled, the current app remains open. A later cancellation in the setup wizard leaves the existing installation usable; reopen it from Start. Save all projects before starting an update.

## Building and testing setup

Install Inno Setup 6 or pass the path to its compiler. Build PixelForge first, then run from the repository:

```powershell
cargo build --locked --release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\Build-Installer.ps1 -Compiler 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
```

The output is under `target\installer`. GitHub Releases builds and publishes both the setup executable and portable ZIP, with a checksum for each.

For isolated installer testing, compile with `-SmokeTest` into a separate output folder, then pass that **smoke-setup.exe** to `scripts\Test-Installer.ps1`. The fixture uses a separate app registration and shortcuts inside the test folder. It checks installation, optional shortcuts, upgrades/backups, and uninstall preserving a project. Real Program Files installation and Windows elevation prompts should also be checked manually before publication.
