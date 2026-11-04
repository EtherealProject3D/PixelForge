fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set("CompanyName", "EtherealProject3D");
        resource.set("ProductName", "PixelForge");
        resource.set("FileDescription", "PixelForge image editor");
        resource.set("OriginalFilename", "PixelForge.exe");
        resource.set_icon("assets/pixelforge.ico");
        resource.compile().expect("Failed to embed PixelForge icon");
    }
}
