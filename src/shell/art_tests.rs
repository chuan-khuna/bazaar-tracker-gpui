use super::*;

#[test]
fn manifest_counts_cards_and_failures() {
    let text = r#"{"generated_at": "2026-10-31T12:00:00Z", "cards": {"a": "items/x.webp", "b": "skills/y.webp"}, "failures": {"c": "why"}}"#;
    let info = ArtInfo::parse(text).unwrap();
    assert_eq!(info, ArtInfo { cards: 2, missing: 1, generated_at: "2026-10-31T12:00:00Z".into() });
    assert_eq!(info.date(), "2026-10-31");
}

#[test]
fn a_broken_or_missing_manifest_is_none() {
    assert_eq!(ArtInfo::parse("not json"), None);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(ArtInfo::load(dir.path()), None);
}

#[test]
fn progress_counts_bundles() {
    let mut progress = Progress::default();
    assert!(progress.describe().starts_with("Reading"));
    progress.apply(&Step::Planned { images: 10, to_write: 4, bundles: 3 });
    assert_eq!(progress.describe(), "Extracting… 0 of 3 bundles");
    progress.apply(&Step::Bundle { done: 2, total: 3, name: "skills_assets_all.bundle".into(), written: 5, of: 5 });
    assert_eq!(progress.bundles, Some((2, 3)));
    progress.apply(&Step::Planned { images: 10, to_write: 0, bundles: 0 });
    assert_eq!(progress.describe(), "Finishing…");
}

#[test]
fn clearing_removes_the_art_folder_and_minds_no_missing_one() {
    let dir = tempfile::tempdir().unwrap();
    let art = art_dir(dir.path());
    fs::create_dir_all(art.join("items")).unwrap();
    fs::write(art.join("items").join("x.webp"), b"art").unwrap();
    fs::write(art.join(extract::MANIFEST), "{}").unwrap();

    clear(&art).unwrap();
    assert!(!art.exists());
    assert_eq!(ArtInfo::load(&art), None);
    clear(&art).unwrap();
}
