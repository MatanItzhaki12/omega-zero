//! Builds rcheevos, RetroAchievements' own client library (MIT, vendored in rcheevos/), and
//! omega-zero's shim over it. Only what the handhelds' games need is built: a cartridge is
//! identified by hashing its ROM and a PSP game by the files on its disc, so the hashers for
//! zips and encrypted games are left out.

use std::path::Path;

fn sources(dir: &Path, skip: &[&str]) -> Vec<std::path::PathBuf> {
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            !skip.contains(&name)
        })
        .collect();
    found.sort();
    found
}

fn main() {
    let rc = Path::new("rcheevos/src");
    let mut build = cc::Build::new();
    build
        .include("rcheevos/include")
        .include(rc)
        .include("c/include")
        .define("RC_CLIENT_SUPPORTS_HASH", None)
        .define("RC_HASH_NO_ZIP", None)
        .define("RC_HASH_NO_ENCRYPTED", None)
        .warnings(false)
        .file("c/omega_cheevos.c");
    build.files(sources(rc, &["rc_client_external.c"]));
    build.files(sources(&rc.join("rapi"), &[]));
    build.files(sources(&rc.join("rcheevos"), &[]));
    build.files(sources(
        &rc.join("rhash"),
        &["aes.c", "hash_encrypted.c", "hash_zip.c"],
    ));
    build.compile("omega_cheevos");
    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-changed=rcheevos");
}
