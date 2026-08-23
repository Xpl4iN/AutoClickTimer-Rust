fn main() {
    slint_build::compile("ui/app_window.slint").expect("Slint compilation failed");

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=image.ico");
        println!("cargo:rerun-if-changed=app.manifest");
        let mut res = winres::WindowsResource::new();
        res.set_icon("image.ico");
        res.set_manifest_file("app.manifest");
        res.compile().expect("Failed to compile Windows resource");
    }
}
