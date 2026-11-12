//! Embeds the application manifest, icon and version info (res/winhelper.rc).

fn main() {
    println!("cargo:rerun-if-changed=res/winhelper.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
    println!("cargo:rerun-if-changed=res/winhelper.ico");

    let ver = |key: &str| std::env::var(key).unwrap_or_else(|_| "0".into());
    let macros = [
        format!("VER_MAJOR={}", ver("CARGO_PKG_VERSION_MAJOR")),
        format!("VER_MINOR={}", ver("CARGO_PKG_VERSION_MINOR")),
        format!("VER_PATCH={}", ver("CARGO_PKG_VERSION_PATCH")),
        format!("VER_STR=\"{}\"", ver("CARGO_PKG_VERSION")),
    ];

    embed_resource::compile("res/winhelper.rc", &macros)
        .manifest_required()
        .unwrap();
}
