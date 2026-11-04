# PixelForge v0.6.2

This update polishes model and dependency management and improves window placement persistence.

- Model Library cards now use a consistent full width.
- Dependencies are grouped by use case, with consistent full-width cards.
- Window size and position now use the framework's native persistence. Place the window once after updating to establish its saved position.
- Uninstall now offers to remove PixelForge-managed downloaded models, ComfyUI, restoration engines, caches, and retained rollback versions. Keeping downloads is the default; silent uninstall keeps them. Projects, settings, and separately installed ComfyUI are retained. Managed downloads are shared with development and portable copies.
- Development shortcuts use a separate window identity and settings folder and do not install stable updates into the source checkout.

## Updating

In an installed or portable v0.6.1 copy, open **Updates > Check for Updates** to download and install v0.6.2. Installed copies use the setup package; portable copies update using the ZIP package. Save your work before updating.

Both the Windows installer and portable ZIP are available below, with SHA-256 checksums.
