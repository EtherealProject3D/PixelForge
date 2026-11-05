//! A native loading indicator and per-profile launch coordination, before graphics starts.
#[cfg(windows)]
mod platform {
    use std::{fs::OpenOptions, sync::{atomic::{AtomicIsize, Ordering}, OnceLock}, time::Instant};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::{core::{HSTRING, w}, Win32::{Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*}};

    static SPLASH: AtomicIsize = AtomicIsize::new(0);
    static STARTED: OnceLock<Instant> = OnceLock::new();
    pub struct Guard { window: isize, thread: Option<std::thread::JoinHandle<()>> }
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { let _ = PostMessageW(Some(HWND(self.window as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)); }
            if let Some(thread) = self.thread.take() { let _ = thread.join(); }
        }
    }
    unsafe extern "system" fn procedure(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if message == WM_DESTROY {
            unsafe { PostQuitMessage(0); }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(window,message,wp,lp) }
    }
    fn focus(window: HWND) {
        unsafe {
            if IsIconic(window).as_bool() { let _ = ShowWindow(window,SW_RESTORE); }
            let _ = SetForegroundWindow(window);
        }
    }
    fn class_name(dev: bool) -> HSTRING {
        HSTRING::from(if dev {"PixelForgeDevLaunchCoordinator"} else {"PixelForgeLaunchCoordinator"})
    }
    pub fn begin() -> Option<Guard> {
        STARTED.get_or_init(Instant::now);
        match begin_inner() {
            Ok(value) => value,
            Err(error) => {
                rfd::MessageDialog::new().set_title("Pixel Forge could not start")
                    .set_description(format!("Could not initialize the loading window: {error}"))
                    .set_level(rfd::MessageLevel::Error).show();
                None
            }
        }
    }
    fn begin_inner() -> Result<Option<Guard>,String> {
        let root = crate::profile::settings_root().ok_or("No settings folder available")?;
        std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;
        // Serialize the check and splash creation so simultaneous clicks cannot race.
        let gate = OpenOptions::new().create(true).truncate(false).read(true).write(true)
            .open(root.join("launch.lock")).map_err(|e|e.to_string())?;
        fs2::FileExt::lock_exclusive(&gate).map_err(|e|e.to_string())?;
        let class = class_name(crate::profile::is_dev());
        if let Ok(existing) = unsafe { FindWindowW(&class,None) } {
            if unsafe { IsWindowVisible(existing).as_bool() } {
                focus(existing);
                return Ok(None);
            }
            let result = rfd::MessageDialog::new().set_title("Pixel Forge is already open")
                .set_description("An instance of Pixel Forge is already open.\n\nOpen another instance?\n\nYes: Open another instance\nNo: Show the existing instance")
                .set_buttons(rfd::MessageButtons::YesNo).show();
            if result != rfd::MessageDialogResult::Yes {
                let main = unsafe { GetPropW(existing,w!("PixelForgeMainWindow")) };
                if !main.is_invalid() { focus(HWND(main.0)); }
                return Ok(None);
            }
        }
        let (tx,rx) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || unsafe {
            let result = create_splash(&class);
            match result {
                Ok(window) => {
                    if tx.send(Ok(window.0 as isize)).is_err() { let _ = DestroyWindow(window); return; }
                    let mut message = MSG::default();
                    while GetMessageW(&mut message,None,0,0).0 > 0 {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                Err(error) => {let _ = tx.send(Err(error));}
            }
        });
        let window = rx.recv().map_err(|e|e.to_string())??;
        SPLASH.store(window,Ordering::Release);
        drop(gate);
        Ok(Some(Guard {window,thread:Some(thread)}))
    }
    unsafe fn create_splash(class: &HSTRING) -> Result<HWND,String> {
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(None).map_err(|e|e.to_string())?.into();
            let wc = WNDCLASSW {lpfnWndProc:Some(procedure), hInstance:instance,
                lpszClassName:windows::core::PCWSTR(class.as_ptr()),
                hbrBackground:GetSysColorBrush(COLOR_WINDOW), hCursor:LoadCursorW(None,IDC_ARROW).map_err(|e|e.to_string())?, ..Default::default()};
            if RegisterClassW(&wc) == 0 { return Err(windows::core::Error::from_win32().to_string()); }
            let title = HSTRING::from(if crate::profile::is_dev() {"Pixel Forge Dev is loading…"} else {"Pixel Forge is loading…"});
            let width=380; let height=110;
            let window = CreateWindowExW(WS_EX_TOOLWINDOW,&*class,&title,WS_POPUP|WS_BORDER,
                (GetSystemMetrics(SM_CXSCREEN)-width)/2,(GetSystemMetrics(SM_CYSCREEN)-height)/2,
                width,height,None,None,Some(instance),None).map_err(|e|e.to_string())?;
            let text = CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),&title,
                WS_CHILD|WS_VISIBLE|WINDOW_STYLE(1),20,35,340,40,Some(window),None,Some(instance),None).map_err(|e|e.to_string())?;
            let font = GetStockObject(DEFAULT_GUI_FONT);
            SendMessageW(text,WM_SETFONT,Some(WPARAM(font.0 as usize)),Some(LPARAM(1)));
            let _ = ShowWindow(window,SW_SHOWNORMAL);
            let _ = UpdateWindow(window);
            Ok(window)
        }
    }
    pub fn ready(frame: &eframe::Frame) {
        let splash = SPLASH.swap(0,Ordering::AcqRel);
        if splash == 0 { return; }
        if let Ok(handle) = frame.window_handle() {
            if let RawWindowHandle::Win32(main) = handle.as_raw() {
                unsafe {
                    let window = HWND(splash as *mut _);
                    let _ = SetPropW(window,w!("PixelForgeMainWindow"),Some(HANDLE(main.hwnd.get() as *mut _)));
                    let _ = ShowWindow(window,SW_HIDE);
                }
            }
        }
        if let (Some(started),Some(root)) = (STARTED.get(),crate::profile::settings_root()) {
            let _ = std::fs::write(root.join("startup-timing.txt"),format!("Time to first UI frame: {:.3} seconds\n",started.elapsed().as_secs_f64()));
        }
    }
    #[cfg(test)]
    mod tests {
        #[test]
        fn dev_and_release_have_separate_coordinators() {
            assert_ne!(super::class_name(true),super::class_name(false));
        }
    }
}
#[cfg(not(windows))]
mod platform {
    pub struct Guard;
    pub fn begin() -> Option<Guard> {Some(Guard)}
    pub fn ready(_: &eframe::Frame) {}
}
pub use platform::{begin,ready};
