fn main() {
    slint_build::compile("ui/app_window.slint").expect("Slint compilation failed");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("image.ico");
        res.set_manifest_file("app.manifest");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resource: {}", e);
        }
    }
}
