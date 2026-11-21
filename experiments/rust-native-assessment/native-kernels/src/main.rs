use aho_corasick::AhoCorasick;
use naga::back::spv;
use naga::front::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::{env, error::Error, fs, path::Path};

fn timed_cues() -> Result<(), Box<dyn Error>> {
    let matcher = AhoCorasick::new(["<turn><the><wheel>", "<kick><hat><snare>"])?;
    println!("source,hit,start_sample,end_sample,gesture_at_75_percent");
    for (id, (source, words)) in [
        ("timed-lyrics", ["turn", "the", "wheel"]),
        ("audio-events", ["kick", "hat", "snare"]),
    ].iter().enumerate() {
        let tokens = [
            ("préface", 0, 18_000), (words[0], 48_000, 60_000),
            (words[1], 64_500, 70_000), (words[2], 71_000, 96_000),
            (words[0], 120_000, 135_000), ("decoy", 136_000, 150_000),
            (words[0], 192_000, 215_000), (words[1], 216_000, 235_000),
            (words[2], 238_000, 264_000),
        ];
        let mut bytes = String::new();
        let mut sample_spans = Vec::new();
        for (label, start, end) in tokens {
            bytes.push_str(&format!("<{label}>"));
            sample_spans.resize(bytes.len(), (start, end));
        }
        for (i, hit) in matcher.find_overlapping_iter(bytes.as_bytes()).enumerate() {
            assert_eq!(hit.pattern().as_usize(), id);
            let start = sample_spans[hit.start()].0;
            let end = sample_spans[hit.end() - 1].1;
            let trigger = start + 3 * (end - start) / 4;
            println!("{source},{},{start},{end},{trigger}", i + 1);
        }
    }
    Ok(())
}

fn compile_shader(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let options = glsl::Options::from(naga::ShaderStage::Fragment);
    let module = glsl::Frontend::default().parse(&options, &source)
        .map_err(|errors| format!("GLSL frontend: {errors:?}"))?;
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module).map_err(|e| format!("validation: {e:?}"))?;
    let options = spv::Options { lang_version: (1, 3), ..Default::default() };
    let words = spv::write_vec(&module, &info, &options, None)
        .map_err(|e| format!("SPIR-V writer: {e}"))?;
    let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    fs::write(path.with_extension("naga.spv"), bytes).map_err(|e| e.to_string())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [arg] if arg == "cues" => timed_cues(),
        [arg, path] if arg == "shader" => compile_shader(Path::new(path)).map_err(Into::into),
        _ => Err("expected cues or shader <file.frag>".into()),
    }
}
