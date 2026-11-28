use serde::Serialize;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

// Matches MAX_PACK_BYTES in the editor's manifest parser.
const MAX_PACK_BYTES: u64 = 256 * 1024;
const MAX_CATALOG_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogFile {
    file_name: String,
    text: String,
}

fn catalog_dir(executable: &Path) -> Option<PathBuf> {
    let dir = executable.parent()?;
    if dir.ends_with("Contents/MacOS") {
        let bundle = dir.parent()?.parent()?;
        if bundle.extension().is_some_and(|ext| ext == "app") {
            return Some(bundle.parent()?.join("legend-packs"));
        }
    }
    Some(dir.join("legend-packs"))
}

fn catalog_files(dir: &Path) -> io::Result<Vec<CatalogFile>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut files = Vec::new();
    let mut total_bytes = 0;
    for entry in entries {
        let entry = entry?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !file_name.ends_with(".tsslegends.json") || !entry.file_type()?.is_file() {
            continue;
        }
        let file = fs::File::open(entry.path())?;
        if file.metadata()?.len() > MAX_PACK_BYTES {
            continue;
        }
        let mut text = String::new();
        match file.take(MAX_PACK_BYTES + 1).read_to_string(&mut text) {
            Err(e) if e.kind() == io::ErrorKind::InvalidData => continue,
            Err(e) => return Err(e),
            Ok(_) if text.len() as u64 > MAX_PACK_BYTES => continue,
            Ok(_) => (),
        }
        total_bytes += text.len();
        if total_bytes > MAX_CATALOG_BYTES {
            return Err(io::Error::other(
                "The adjacent pack folder exceeds 8 MiB. Individual packs can still be imported.",
            ));
        }
        files.push(CatalogFile { file_name, text });
    }
    files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(files)
}

#[tauri::command]
pub(crate) async fn available_legend_packs() -> Result<Vec<CatalogFile>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = catalog_dir(&exe).ok_or("Could not locate the adjacent Legends pack folder.")?;
    catalog_files(&dir).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_catalog_lives_beside_bundle_or_executable() {
        for exe in ["review/TSS.app/Contents/MacOS/tss", "review/tss.exe"] {
            assert_eq!(
                catalog_dir(Path::new(exe)).unwrap(),
                Path::new("review/legend-packs")
            );
        }
    }

    #[test]
    fn discovery_only_reads_regular_pack_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("legend-packs");
        assert!(catalog_files(&dir).unwrap().is_empty());
        fs::create_dir(&dir).unwrap();
        for name in ["b.tsslegends.json", "a.tsslegends.json", "notes.txt"] {
            fs::write(dir.join(name), "{}").unwrap();
        }
        fs::write(dir.join("bad.tsslegends.json"), [255]).unwrap();
        fs::write(
            dir.join("big.tsslegends.json"),
            vec![0; MAX_PACK_BYTES as usize + 1],
        )
        .unwrap();
        fs::create_dir(dir.join("nested.tsslegends.json")).unwrap();
        let files = catalog_files(&dir).unwrap();
        assert_eq!(
            files
                .iter()
                .map(|f| f.file_name.as_str())
                .collect::<Vec<_>>(),
            ["a.tsslegends.json", "b.tsslegends.json"]
        );
        assert!(files.iter().all(|f| f.text == "{}"));
    }
}
