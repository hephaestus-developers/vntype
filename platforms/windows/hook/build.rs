use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_exe() -> Option<PathBuf> {
    if let Ok(output) = Command::new("where.exe").arg("rc.exe").output()
        && output.status.success()
        && let Some(first) = String::from_utf8_lossy(&output.stdout).lines().next()
    {
        let p = PathBuf::from(first.trim());
        if p.exists() {
            return Some(p);
        }
    }
    let kits_bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(entries) = std::fs::read_dir(kits_bin) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path().join("x64").join("rc.exe")))
            .filter(|p| p.exists())
            .collect();
        versions.sort();
        return versions.pop();
    }
    None
}

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let manifest_path = Path::new(&manifest_dir).join("vntype.manifest");
        println!("cargo:rerun-if-changed={}", manifest_path.display());
        println!("cargo:rustc-link-arg-bin=vntype-hook=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bin=vntype-hook=/MANIFESTINPUT:{}", manifest_path.display());

        let win_dir = Path::new(&manifest_dir).parent().unwrap();
        let rc_path = win_dir.join("vntype.rc");
        let ico_path = win_dir.join("vntype.ico");
        println!("cargo:rerun-if-changed={}", rc_path.display());
        println!("cargo:rerun-if-changed={}", ico_path.display());

        if let Some(rc_exe) = find_rc_exe() {
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let res_path = Path::new(&out_dir).join("vntype_icon.res");
            let status =
                Command::new(rc_exe).arg("/nologo").arg("/fo").arg(&res_path).arg(&rc_path).status();
            if status.is_ok_and(|s| s.success()) && res_path.exists() {
                println!("cargo:rustc-link-arg-bin=vntype-hook={}", res_path.display());
            }
        }
    }
}
