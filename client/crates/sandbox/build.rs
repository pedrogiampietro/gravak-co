//! Windows builds carry the "G" emblem and the product name in the
//! executable itself, for Explorer, shortcuts and the installer.

fn main() {
    println!("cargo:rerun-if-changed=../../installer/gravak.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("../../installer/gravak.ico")
        .set("ProductName", "Gravak")
        .set("FileDescription", "Gravak");
    // A machine without the Windows SDK still builds, just without the icon.
    if let Err(error) = resource.compile() {
        println!("cargo:warning=Gravak icon not embedded: {error}");
    }
}
