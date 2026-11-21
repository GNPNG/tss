fn main() {
    assert_eq!(std::env::var("CARGO_CFG_TARGET_OS").unwrap(), "macos");
    println!("cargo:rerun-if-changed=observe.c");
    cc::Build::new().file("observe.c").compile("tss_vt_observe");
    println!("cargo:rustc-link-lib=framework=VideoToolbox");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}
