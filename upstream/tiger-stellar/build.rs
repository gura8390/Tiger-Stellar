#[path = "src/hash.rs"]
mod hash;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
fn collect(base: &Path, out: &mut BTreeSet<PathBuf>) {
    for entry in std::fs::read_dir(base).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            collect(&p, out);
        } else if matches!(
            p.extension().and_then(|s| s.to_str()),
            Some("rs" | "lalrpop")
        ) {
            out.insert(p);
        }
    }
}
fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap().parent().unwrap();
    let mut files = BTreeSet::new();
    for rel in [
        "upstream/tiger-stellar/src",
        "upstream/stellaris-parser/src",
    ] {
        let dir = root.join(rel);
        println!("cargo:rerun-if-changed={}", dir.display());
        collect(&dir, &mut files);
    }
    for rel in [
        "upstream/tiger-stellar/build.rs",
        "upstream/tiger-stellar/Cargo.toml",
        "upstream/stellaris-parser/build.rs",
        "upstream/stellaris-parser/Cargo.toml",
        "upstream/Cargo.toml",
        "upstream/Cargo.lock",
        "rules/stellaris-4.5.2.json",
        "rules/declarations-4.5.2.json",
    ] {
        let p = root.join(rel);
        println!("cargo:rerun-if-changed={}", p.display());
        files.insert(p);
    }
    let mut material = Vec::new();
    for p in files {
        material.extend_from_slice(
            p.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
                .as_bytes(),
        );
        material.push(0);
        material.extend_from_slice(&std::fs::read(p).unwrap());
        material.push(0);
    }
    println!(
        "cargo:rustc-env=TIGER_STELLAR_SOURCE_SHA256={}",
        hash::sha256(&material)
    );
}
