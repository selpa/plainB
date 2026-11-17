fn main() {
    println!("cargo:rerun-if-changed=assets/plain-browser.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/plain-browser.ico")
            .set("ProductName", "Plain Browser")
            .set("FileDescription", "Plain Browser")
            .compile()
            .expect("could not embed the Plain Browser icon");
    }
}
