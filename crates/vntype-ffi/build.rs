//! Tập lệnh biên dịch cho `vntype-ffi`: kiểm tra sự tồn tại của `include/vntype.h`
//! và thiết lập kịch bản ẩn biểu tượng nội bộ khi liên kết thư viện động (`cdylib`).

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("thiếu CARGO_MANIFEST_DIR"));
    let header = manifest_dir.join("include").join("vntype.h");
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-changed={}", manifest_dir.join("symbols.version").display());
    println!("cargo:rerun-if-changed={}", manifest_dir.join("symbols.exports").display());
    assert!(header.exists(), "Không tìm thấy tệp header C ABI tại {}", header.display());

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        let version_script = manifest_dir.join("symbols.version");
        println!("cargo:rustc-cdylib-link-arg=-Wl,--version-script={}", version_script.display());
    } else if target_os == "macos" {
        let exports = manifest_dir.join("symbols.exports");
        println!("cargo:rustc-cdylib-link-arg=-Wl,-exported_symbols_list,{}", exports.display());
    }
}
