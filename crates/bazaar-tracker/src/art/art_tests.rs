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
fn render_keeps_the_shape_shrinks_and_drops_the_alpha_of_items() {
    let source = RgbaImage::from_pixel(1024, 1024, image::Rgba([10, 20, 30, 0]));
    let item = render(&source, true, 512);
    assert_eq!((item.width(), item.height()), (512, 512));
    assert!(matches!(item, DynamicImage::ImageRgb8(_)));
    let wide = render(&RgbaImage::new(1024, 512), true, 512);
    assert_eq!((wide.width(), wide.height()), (512, 256));
    let icon = render(&RgbaImage::new(256, 256), false, 512);
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

#[test]
fn layers_are_drawn_bottom_to_top() {
    let background = Rc::new(RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 255, 255])));
    let mut portrait = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
    portrait.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
    let merged = merge(&[background, Rc::new(portrait)]).expect("two layers");
    assert_eq!(merged.dimensions(), (4, 4));
    assert_eq!(merged.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(merged.get_pixel(3, 3).0, [0, 0, 255, 255]);
    assert!(merge(&[]).is_none());
}
