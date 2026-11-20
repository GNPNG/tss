use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let hook_path = "../experiments/slide-look-study/optics.hook";
    println!("cargo:rerun-if-changed={hook_path}");
    println!("cargo:rerun-if-changed=src/optics-header.glsl");
    println!("cargo:rerun-if-changed=src/footprint.glsl");
    println!("cargo:rerun-if-changed=src/film-material.glsl");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
    println!("cargo:rerun-if-env-changed=GLSLC");
    let sdk = env::var_os("VULKAN_SDK").map(PathBuf::from);
    let exe = if cfg!(windows) { "glslc.exe" } else { "glslc" };
    let compiler = env::var_os("GLSLC")
        .map(PathBuf::from)
        .or_else(|| {
            sdk.and_then(|dir| {
                ["Bin", "bin"]
                    .iter()
                    .map(|sub| dir.join(sub).join(exe))
                    .find(|p| p.is_file())
            })
        })
        .unwrap_or_else(|| PathBuf::from(exe));
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let hook = fs::read_to_string(hook_path).expect("read the shared wheel optics");
    let header = fs::read_to_string("src/optics-header.glsl").unwrap();
    let footprint = fs::read_to_string("src/footprint.glsl").unwrap();
    let compile = |name: &str, sampler: &str, material: &str, entry: &str| {
        let src = format!("{header}\n{sampler}\n{material}\n{hook}\n{entry}\n");
        let file = out.join(format!("{name}.frag"));
        fs::write(&file, src).unwrap();
        let result = Command::new(&compiler)
            .args(["--target-env=vulkan1.1", "-O"])
            .arg(&file)
            .arg("-o")
            .arg(out.join(format!("{name}.spv")))
            .output()
            .expect("live optics need glslc from the Vulkan SDK at build time");
        assert!(
            result.status.success(),
            "glslc: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    compile(
        "film-material",
        &footprint,
        "",
        &fs::read_to_string("src/film-material.glsl").unwrap(),
    );
    for (name, material) in [
        ("violet-blue", ""),
        (
            "wheel",
            r#"#define TSS_WHEEL_PROFILE 1
#define pinkAbsorption (mat.rear.rgb)
#define cyanAbsorption (mat.front.rgb)
#define pairedDensityCorrection (mat.joint.rgb)
#define rearRimDelta (mat.rim.rgb)
#define wheelCarrier (mat.carrier.rgb)
#define depthVariation (mat.carrier.w)"#,
        ),
    ] {
        for (suffix, sampler) in [("", ""), ("-footprint", footprint.as_str())] {
            compile(
                &format!("{name}{suffix}"),
                sampler,
                material,
                "void main() { result = hook(); }",
            );
        }
    }
}
