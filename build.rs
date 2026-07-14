fn main() {
    println!("cargo:rerun-if-changed=assets/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_manifest(include_str!("assets/app.manifest"));
        resource
            .compile()
            .expect("failed to compile Windows application resources");
    }
}
