fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let def_path = std::path::Path::new(&manifest_dir).join("vntype_tsf.def");
        println!("cargo:rerun-if-changed={}", def_path.display());
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_path.display());
    }
}
