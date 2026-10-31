use super::*;

fn card(card_type: &str, size: Option<&str>, art_key: Option<&str>) -> CardInfo {
    CardInfo { id: "id".into(), name: "Name".into(), card_type: Some(card_type.into()), size: size.map(Into::into), art_key: art_key.map(Into::into), ..CardInfo::default() }
}

#[test]
fn file_keys_are_lower_case_without_png_and_safe_for_a_file_name() {
    assert_eq!(file_key("Icon_Skill_Backroom.png"), "icon_skill_backroom");
    assert_eq!(file_key("Icon_Skill_BlightRage"), "icon_skill_blightrage");
    assert_eq!(file_key("ef57f8899ff555b4d98d03124c1608a7"), "ef57f8899ff555b4d98d03124c1608a7");
    assert_eq!(file_key("Reward_Nice_D.PNG"), "reward_nice_d");
    assert_eq!(file_key("Assets/Art/X:Y.png"), "assets_art_x_y");
}

#[test]
fn each_card_type_has_its_folder() {
    assert_eq!(relative_path(&card("Item", Some("Small"), Some("ABC"))).as_deref(), Some("items/abc.webp"));
    assert_eq!(relative_path(&card("SocketEffect", None, Some("Icon_Skill_Placeholder.png"))).as_deref(), Some("skills/icon_skill_placeholder.webp"));
    assert_eq!(relative_path(&card("EncounterStep", None, Some("Reward_Nice_D.png"))).as_deref(), Some("rewards/reward_nice_d.webp"));
    assert_eq!(relative_path(&card("PlayerEffect", None, Some("x"))), None);
    assert_eq!(relative_path(&card("Item", None, None)), None);
}

#[test]
fn item_cards_are_half_as_wide_square_or_half_again_as_wide() {
    assert_eq!(item_aspect(Some("Small")), 0.5);
    assert_eq!(item_aspect(Some("Medium")), 1.0);
    assert_eq!(item_aspect(Some("Large")), 1.5);
    assert_eq!(item_aspect(None), 1.0);
}

#[test]
fn the_crop_is_centred_and_as_large_as_fits() {
    assert_eq!(centre_crop(1024, 1024, 0.5), (256, 0, 512, 1024));
    assert_eq!(centre_crop(1024, 1024, 1.0), (0, 0, 1024, 1024));
    assert_eq!(centre_crop(1024, 1024, 1.5), (0, 170, 1024, 683));
    assert_eq!(centre_crop(2048, 1024, 1.0), (512, 0, 1024, 1024));
}

#[test]
fn render_crops_shrinks_and_drops_the_alpha_of_items() {
    let source = RgbaImage::from_pixel(1024, 1024, image::Rgba([10, 20, 30, 0]));
    let small = render(&source, true, Some(0.5), 512);
    assert_eq!((small.width(), small.height()), (256, 512));
    assert!(matches!(small, DynamicImage::ImageRgb8(_)));
    let large = render(&source, true, Some(1.5), 512);
    assert_eq!((large.width(), large.height()), (512, 342));
    let icon = render(&RgbaImage::new(256, 256), false, None, 512);
    assert_eq!((icon.width(), icon.height()), (256, 256));
    assert!(matches!(icon, DynamicImage::ImageRgba8(_)));
}

#[test]
fn library_paths_are_read_from_libraryfolders_vdf() {
    let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"H:\\\\SteamLibrary\"\n\t\t\"label\"\t\t\"\"\n\t}\n}\n";
    assert_eq!(library_paths(vdf), [PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"H:\SteamLibrary")]);
}

#[test]
fn times_are_written_as_utc() {
    assert_eq!(iso_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    assert_eq!(iso_utc(UNIX_EPOCH + std::time::Duration::from_secs(1_793_448_000)), "2026-10-31T12:00:00Z");
}
