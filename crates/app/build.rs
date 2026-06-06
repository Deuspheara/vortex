fn main() {
    let fonts_dir = std::path::Path::new("assets/fonts");
    println!("cargo:rerun-if-changed={}", fonts_dir.display());
    if fonts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(fonts_dir) {
            for entry in entries.flatten() {
                println!("cargo:rerun-if-changed={}", entry.path().display());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let plist = std::path::Path::new("Info.plist");
        println!("cargo:rerun-if-changed=Info.plist");
        if plist.exists() {
            let path = plist.canonicalize().expect("canonicalize Info.plist");
            // Each token must be its own link-arg; a single combined string is rejected by clang.
            println!("cargo:rustc-link-arg=-sectcreate");
            println!("cargo:rustc-link-arg=__TEXT");
            println!("cargo:rustc-link-arg=__info_plist");
            println!("cargo:rustc-link-arg={}", path.display());
        }
    }
}
