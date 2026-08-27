use std::path::{Path, PathBuf};

const PURE_PRODUCTION_LOC_LIMIT: usize = 250;

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = std::fs::read_dir(directory)
        .expect("read production source directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect production source entries");
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn workspace_rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(&root.join("src"), &mut files);
    let mut crates = std::fs::read_dir(root.join("crates"))
        .expect("read workspace crates")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect workspace crates");
    crates.sort_by_key(|entry| entry.path());
    for entry in crates {
        let source = entry.path().join("src");
        if source.is_dir() {
            collect_rust_files(&source, &mut files);
        }
    }
    files
}

#[test]
fn vendored_dependency_is_explicitly_outside_first_party_module_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vendor = root.join("third_party/portable-pty-psmux");
    assert!(vendor.join("README.rshell-patch.md").is_file());
    assert!(
        workspace_rust_sources(root)
            .iter()
            .all(|source| !source.starts_with(&vendor))
    );
}

fn pure_production_loc(source: &str) -> usize {
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(source)
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with("//")
        })
        .count()
}

#[test]
fn all_production_modules_stay_within_pure_loc_cap() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = workspace_rust_sources(root);
    assert!(
        !files.is_empty(),
        "workspace production source discovery returned zero files"
    );
    let mut violations = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).expect("read production Rust source");
        let count = pure_production_loc(&source);
        if count > PURE_PRODUCTION_LOC_LIMIT {
            violations.push(format!(
                "{} has {count} pure production lines (limit {PURE_PRODUCTION_LOC_LIMIT})",
                file.strip_prefix(root).unwrap_or(&file).display()
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}
