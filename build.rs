fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/pixelforge.ico");
        resource.compile().expect("Failed to embed PixelForge icon");
    }
}
