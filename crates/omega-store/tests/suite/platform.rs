use omega_store::Platform;

#[test]
fn every_platform_has_a_directory() {
    let names: Vec<&str> = Platform::ALL.iter().map(|p| p.dir_name()).collect();
    assert_eq!(names, vec!["GBA", "GB", "GBC", "SNES", "PS1", "PSP"]);
}

#[test]
fn every_platform_is_a_shelf_of_its_own() {
    assert_eq!(
        Platform::ALL,
        [
            Platform::Gba,
            Platform::Gb,
            Platform::Gbc,
            Platform::Snes,
            Platform::Ps1,
            Platform::Psp
        ]
    );
    for (i, p) in Platform::ALL.iter().enumerate() {
        assert!(
            !Platform::ALL[..i].contains(p),
            "{p:?} is named twice in the ring, so two shelves would hold one platform"
        );
    }
}

#[test]
fn each_platform_takes_only_its_own_extensions() {
    assert!(Platform::Gba.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(!Platform::Gba.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gbc")));
    assert!(!Platform::Gb.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(Platform::Gba.accepts(std::path::Path::new("Shrek.GBA")));
}

/// A PSP game is a UMD's image, compressed or not, or an eboot; it rewinds nothing, and held L2
/// is its analog stick.
#[test]
fn the_psp_takes_its_images_and_holds_l2_for_the_stick() {
    for name in ["Daxter.iso", "Lumines.CSO", "Patapon.chd", "Echochrome.pbp"] {
        assert!(Platform::Psp.accepts(std::path::Path::new(name)), "{name}");
    }
    assert!(!Platform::Psp.accepts(std::path::Path::new("Crash.cue")));
    assert_eq!(Platform::Psp.picture(), (480, 272));
    assert!(!Platform::Psp.rewinds());
    assert!(Platform::Psp.stick_on_l2());
    assert!(!Platform::Ps1.stick_on_l2());
    assert_eq!(Platform::Psp.name(), "PlayStation Portable");
}
