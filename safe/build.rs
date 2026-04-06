use std::env;
use std::path::PathBuf;

const LINUX_SONAME: &str = "libbz2.so.1.0";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let abi_dir = manifest_dir.join("abi");
    let version_script = abi_dir.join("libbz2.map");
    let def_file = abi_dir.join("libbz2.def");

    println!(
        "cargo:rerun-if-changed={}",
        version_script.display()
    );
    println!("cargo:rerun-if-changed={}", def_file.display());
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "linux" {
        // Keep the staged shared object on the same SONAME and export contract
        // that safe/scripts/check-abi.sh validates against upstream.
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,{LINUX_SONAME}");
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
            version_script.display()
        );
    }

    if target_os == "windows" {
        if target_env == "msvc" {
            println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_file.display());
        } else {
            println!("cargo:rustc-cdylib-link-arg={}", def_file.display());
        }
    }
}
