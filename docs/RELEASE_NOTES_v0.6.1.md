# PixelForge v0.6.1

## Improvements

- Two Windows download options: a portable ZIP and a setup executable. Setup defaults to **Program Files\Pixel Forge**, adds a Start menu entry, offers a desktop shortcut, and registers an uninstaller.

- PixelForge always uses its charcoal-and-blue dark theme, including on Windows systems set to light mode.
- **Updates** in the top menu checks published stable GitHub releases, displays release notes, downloads and verifies the Windows package, and offers **Install & Restart**.
- Update installation waits until open projects are saved and image processing/model or dependency installation has finished. The previous executable is retained as a `.previous` backup.
- Dependency installation progress and cancellation now appear beneath the component being installed, rather than at the bottom of the window.
- PixelForge-owned ComfyUI backends explicitly disable automatic browser launching, including portable installations.

## Update behavior

Update checks are manual. Development pushes and prereleases are not offered. Downloads must come from the project's release assets and pass the published SHA-256 checksum and executable version checks. Portable updates require a writable application folder. Installed copies download the setup package and request administrator permission through Windows; cancelling that permission leaves PixelForge open. Existing projects and model/dependency folders are not replaced.

For source checkouts, continue using Git pull and rebuilding: the development desktop launcher rebuilds checked-out source, so it can overwrite an executable-only update with that source version. The in-app installer is intended for extracted Windows release builds.

v0.6.0 users must download v0.6.1 manually to get the new Updates menu. Later published versions can then be installed from within the app.
