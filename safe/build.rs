use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let abi_dir = manifest_dir.join("abi");

    println!(
        "cargo:rerun-if-changed={}",
        abi_dir.join("libbz2.map").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        abi_dir.join("libbz2.def").display()
    );
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "linux" {
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
            abi_dir.join("libbz2.map").display()
        );
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libbz2.so.1.0");
    }

    if target_os == "windows" {
        let def_file = abi_dir.join("libbz2.def");
        if target_env == "msvc" {
            println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_file.display());
        } else {
            println!("cargo:rustc-cdylib-link-arg={}", def_file.display());
        }
    }
}
