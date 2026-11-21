fn main() {
    #[cfg(feature = "cimg-optics")]
    {
        let include = std::env::var_os("CIMG_INCLUDE")
            .expect("set CIMG_INCLUDE to the directory containing the pinned CImg.h");
        println!("cargo:rerun-if-env-changed=CIMG_INCLUDE");
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&include).join("CImg.h").display()
        );
        println!("cargo:rerun-if-changed=../slide-look-study/optical-distance.hpp");
        println!("cargo:rerun-if-changed=optical-bridge.cpp");
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .flag_if_supported("/EHsc")
            .include(include)
            .file("optical-bridge.cpp")
            .compile("tss_live_optical_bridge");
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            // CImg resolves the Windows Program Files directory through Shell32.
            println!("cargo:rustc-link-lib=shell32");
        }
    }
}
