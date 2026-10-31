//! Card art: extract each card's picture from the player's own install of The Bazaar into
//! `<out>/<category>/<key>.webp`, with a `manifest.json` beside them.
//!
//! A card's `ArtKey` (from the card catalog) is an Addressables key. The catalog
//! ([`addressables`]) names the asset and its bundle; the bundle ([`unity`]) holds the asset, which
//! leads to a texture: an item's card material's `_MainTex`, a skill's or reward's sprite, an
//! encounter's portrait or else its background. How the game stores all this:
//! docs/rea/the-bazaar/card-art.md.
//!
//! Item art is kept square, as the game stores it: the app stretches it to the card's shape when it
//! draws it. It is kept opaque, as the alpha of the game's item art is a cut-out mask. Everything else keeps its transparency.
//!
//! The art is Tempo Storm's: it is for showing in this app only, never to be shared.

pub mod addressables;
pub mod unity;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use image::{DynamicImage, RgbaImage, imageops::FilterType};
use serde_json::{Value as Json, json};

use crate::catalog::{CardCatalog, CardInfo};
use addressables::{Catalog, Location};
use unity::serialized::{SerializedFile, Value, class};

/// Bumped when the way images are drawn changes, so a run redraws every image.
pub const RENDER_VERSION: u64 = 3;

pub const MANIFEST: &str = "manifest.json";

/// The folder a card's art goes in, by its `Type`. Types not listed have no art.
pub fn category(card_type: &str) -> Option<&'static str> {
    Some(match card_type {
        "Item" => "items",
        "Skill" | "SocketEffect" => "skills",
        "EncounterStep" => "rewards",
        "CombatEncounter" => "monsters",
        "EventEncounter" => "events",
        "PedestalEncounter" => "pedestals",
        _ => return None,
    })
}

/// The file stem for an `ArtKey`: lower-cased, a trailing `.png` removed, and characters a file
/// name cannot hold replaced with `_`. The app finds a card's file by the same rule.
pub fn file_key(art_key: &str) -> String {
    let key = art_key.trim();
    let key = if key.to_ascii_lowercase().ends_with(".png") { &key[..key.len() - 4] } else { key };
    key.chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect::<String>().to_lowercase()
}

/// The path of a card's art under the art folder, if it can have any: `items/<key>.webp`.
pub fn relative_path(card: &CardInfo) -> Option<String> {
    let category = category(card.card_type.as_deref()?)?;
    Some(format!("{category}/{}.webp", file_key(card.art_key.as_deref()?)))
}

/// How a run is set up.
#[derive(Clone, Debug)]
pub struct Options {
    /// The Bazaar's install folder (holding `TheBazaar_Data`).
    pub game_dir: PathBuf,
    /// The folder to write into.
    pub out_dir: PathBuf,
    /// The longest side of an image, in pixels.
    pub max_size: u32,
    /// WebP quality, 0–100.
    pub quality: f32,
    /// Bundles read at once. Each holds a bundle (up to a few hundred MB) in memory.
    pub workers: usize,
    /// Redraw images that are already there.
    pub force: bool,
}

impl Options {
    pub fn new(game_dir: PathBuf, out_dir: PathBuf) -> Self {
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1)).clamp(1, 4);
        Self { game_dir, out_dir, max_size: 512, quality: 82.0, workers, force: false }
    }
}

/// What a run reports as it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// The plan: images for every card with art, how many of them are to be written, from how many bundles.
    Planned { images: usize, to_write: usize, bundles: usize },
    /// A bundle is done.
    Bundle { done: usize, total: usize, name: String, written: usize, of: usize },
}

/// How a run went.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub written: usize,
    pub bytes: u64,
    /// Cards that have art now.
    pub cards: usize,
    /// Cards with an `ArtKey` whose art could not be had, by why.
    pub failures: BTreeMap<String, String>,
    pub seconds: f32,
}

/// The Bazaar's install folder: `setting` if it names one, else the first Steam library that has it.
pub fn find_game_dir(setting: &str) -> Option<PathBuf> {
    let is_game = |path: &Path| path.join("TheBazaar_Data").is_dir();
    if !setting.trim().is_empty() {
        return Some(PathBuf::from(setting.trim())).filter(|path| is_game(path));
    }
    steam_libraries().into_iter().map(|library| library.join("steamapps").join("common").join("The Bazaar")).find(|path| is_game(path))
}

/// Steam's own folder and every library its `libraryfolders.vdf` lists.
fn steam_libraries() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = std::process::Command::new("reg").args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"]).creation_flags(CREATE_NO_WINDOW).output();
        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = text.lines().find_map(|line| line.split_once("REG_SZ").map(|(_, path)| path.trim().to_owned())) {
                roots.push(PathBuf::from(path));
            }
        }
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in roots {
        if let Ok(vdf) = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
            libraries.extend(library_paths(&vdf));
        }
        libraries.push(root);
    }
    let mut seen = std::collections::HashSet::new();
    libraries.retain(|path| seen.insert(path.to_string_lossy().to_lowercase()));
    libraries
}

/// The `"path"` values of a `libraryfolders.vdf`.
pub fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| {
            let mut quoted = line.split('"').skip(1).step_by(2);
            (quoted.next()? == "path").then(|| quoted.next()).flatten().map(|path| PathBuf::from(path.replace(r"\\", r"\")))
        })
        .collect()
}

/// One image to write.
struct Job {
    rel: String,
    asset: String,
    /// Whether it is an item's: their alpha is a cut-out mask, so it is dropped.
    item: bool,
}

/// Extract the art of every card in `catalog`. `progress` hears how it goes, from worker threads.
pub fn extract(options: &Options, catalog: &CardCatalog, progress: &(dyn Fn(Progress) + Sync)) -> Result<Summary, String> {
    let started = Instant::now();
    let aa = options.game_dir.join("TheBazaar_Data").join("StreamingAssets").join("aa");
    let bundles_dir = aa.join("StandaloneWindows64");
    let catalog_path = aa.join("catalog.bin");
    let addressables = Catalog::parse(&fs::read(&catalog_path).map_err(|why| format!("could not read {}: {why}", catalog_path.display()))?)?;
    if catalog.is_empty() {
        return Err("the card catalog is empty: start the game once so it writes GameData.db".into());
    }
    fs::create_dir_all(&options.out_dir).map_err(|why| format!("could not create {}: {why}", options.out_dir.display()))?;

    let previous = fs::read_to_string(options.out_dir.join(MANIFEST)).ok().and_then(|text| serde_json::from_str::<Json>(&text).ok());
    let format = json!({ "type": "webp", "quality": options.quality, "max_size": options.max_size, "render": RENDER_VERSION });
    let redraw = options.force || previous.as_ref().and_then(|manifest| manifest.get("format")) != Some(&format);

    let mut cards: BTreeMap<String, String> = BTreeMap::new();
    let mut failures: BTreeMap<String, String> = BTreeMap::new();
    let mut jobs: HashMap<String, (Location, Job)> = HashMap::new();
    for card in catalog.cards() {
        let (Some(rel), Some(art_key)) = (relative_path(card), card.art_key.as_deref()) else { continue };
        let id = card.id.to_lowercase();
        let Some(location) = addressables.resolve(art_key) else {
            failures.insert(id, format!("ArtKey {art_key:?} is not in the Addressables catalog"));
            continue;
        };
        let item = card.card_type.as_deref() == Some("Item");
        jobs.entry(rel.clone()).or_insert_with(|| (location.clone(), Job { rel: rel.clone(), asset: location.asset.clone(), item }));
        cards.insert(id, rel);
    }

    let mut by_bundle: HashMap<String, Vec<Job>> = HashMap::new();
    let total_images = jobs.len();
    for (rel, (location, job)) in jobs {
        if redraw || !options.out_dir.join(&rel).is_file() {
            by_bundle.entry(location.bundle).or_default().push(job);
        }
    }
    let to_write = by_bundle.values().map(Vec::len).sum();
    progress(Progress::Planned { images: total_images, to_write, bundles: by_bundle.len() });

    // The biggest bundles first, so a slow one does not finish last on its own.
    let mut queue: Vec<(String, Vec<Job>)> = by_bundle.into_iter().collect();
    queue.sort_by_key(|(bundle, _)| std::cmp::Reverse(fs::metadata(bundles_dir.join(bundle)).map_or(0, |meta| meta.len())));
    let total_bundles = queue.len();
    let queue = Mutex::new(queue);
    let done = AtomicUsize::new(0);
    let failed: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    let written = AtomicUsize::new(0);
    let bytes = std::sync::atomic::AtomicU64::new(0);

    std::thread::scope(|scope| {
        for _ in 0..options.workers.max(1) {
            scope.spawn(|| {
                loop {
                    let Some((bundle, bundle_jobs)) = queue.lock().expect("no worker panics holding the queue").pop() else { break };
                    let results = extract_bundle(&bundles_dir.join(&bundle), &bundle_jobs, options);
                    let mut ok = 0;
                    for (job, result) in bundle_jobs.iter().zip(results) {
                        match result {
                            Ok(size) => {
                                ok += 1;
                                bytes.fetch_add(size, Ordering::Relaxed);
                            }
                            Err(why) => {
                                failed.lock().expect("no worker panics holding the failures").insert(job.rel.clone(), why);
                            }
                        }
                    }
                    written.fetch_add(ok, Ordering::Relaxed);
                    let done = done.fetch_add(1, Ordering::Relaxed) + 1;
                    progress(Progress::Bundle { done, total: total_bundles, name: bundle, written: ok, of: bundle_jobs.len() });
                }
            });
        }
    });

    let failed = failed.into_inner().expect("the workers have finished");
    cards.retain(|id, rel| match failed.get(rel.as_str()) {
        Some(why) => {
            failures.insert(id.clone(), why.clone());
            false
        }
        None => true,
    });

    let boot = fs::read_to_string(options.game_dir.join("TheBazaar_Data").join("boot.config")).unwrap_or_default();
    let manifest = json!({
        "version": 1,
        "generated_at": iso_utc(SystemTime::now()),
        "build_guid": boot.lines().find_map(|line| line.strip_prefix("build-guid=")).map(str::trim),
        "catalog_hash": fs::read_to_string(aa.join("catalog.hash")).ok().map(|hash| hash.trim().to_owned()),
        "game_data": catalog.source().map(|path| path.display().to_string()),
        "format": format,
        "key_rule": "ArtKey, lower-cased, trailing .png removed, \\/:*?\"<>| replaced with _",
        "cards": cards,
        "failures": failures,
    });
    let text = serde_json::to_string_pretty(&manifest).expect("the manifest is plain JSON");
    write_atomic(&options.out_dir.join(MANIFEST), text.as_bytes()).map_err(|why| format!("could not write the manifest: {why}"))?;

    Ok(Summary { written: written.into_inner(), bytes: bytes.into_inner(), cards: cards.len(), failures, seconds: started.elapsed().as_secs_f32() })
}

/// Write each job's image from one bundle. One result per job: the bytes written, or why not.
fn extract_bundle(path: &Path, jobs: &[Job], options: &Options) -> Vec<Result<u64, String>> {
    let fail_all = |why: String| jobs.iter().map(|_| Err(why.clone())).collect();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(why) => return fail_all(format!("could not read {}: {why}", path.display())),
    };
    let entries = match unity::bundle::read(&bytes) {
        Ok(entries) => entries,
        Err(why) => return fail_all(format!("bundle: {why}")),
    };
    drop(bytes);
    let Some(main) = entries.iter().find(|entry| !entry.path.ends_with(".resS") && !entry.path.ends_with(".resource")) else {
        return fail_all("bundle has no serialized file".into());
    };
    let file = match SerializedFile::parse(&main.data) {
        Ok(file) => file,
        Err(why) => return fail_all(format!("serialized file: {why}")),
    };
    let container = match container(&file) {
        Ok(container) => container,
        Err(why) => return fail_all(format!("container: {why}")),
    };
    let stream = |name: &str| entries.iter().find(|entry| entry.path.rsplit('/').next() == Some(name)).map(|entry| entry.data.as_slice());

    let mut decoded: HashMap<i64, RgbaImage> = HashMap::new();
    jobs.iter()
        .map(|job| {
            let path_ids = container.get(&job.asset.to_lowercase()).ok_or("asset is not in its bundle")?;
            let (texture_id, opaque) = path_ids.iter().find_map(|id| texture_of(&file, *id).ok()).map_or_else(|| texture_of(&file, path_ids[0]), Ok)?;
            if let std::collections::hash_map::Entry::Vacant(slot) = decoded.entry(texture_id) {
                let object = file.object(texture_id).ok_or("texture is missing")?;
                let value = file.read(object).map_err(|why| why.to_string())?;
                slot.insert(unity::texture::decode(&value, stream).map_err(|why| why.to_string())?);
            }
            let image = render(&decoded[&texture_id], opaque || job.item, options.max_size);
            let encoded = match &image {
                DynamicImage::ImageRgb8(rgb) => webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(options.quality),
                other => {
                    let rgba = other.to_rgba8();
                    webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height()).encode(options.quality)
                }
            };
            let out = options.out_dir.join(&job.rel);
            if let Some(dir) = out.parent() {
                fs::create_dir_all(dir).map_err(|why| why.to_string())?;
            }
            write_atomic(&out, &encoded[..]).map_err(|why| why.to_string())?;
            Ok(encoded.len() as u64)
        })
        .collect()
}

/// The bundle's container: each asset path (lower-cased) to the objects listed under it.
fn container(file: &SerializedFile) -> Result<HashMap<String, Vec<i64>>, String> {
    let bundle = file.objects().find(|object| file.class_id(object) == class::ASSET_BUNDLE).ok_or("no AssetBundle object")?;
    let value = file.read(bundle).map_err(|why| why.to_string())?;
    let mut map: HashMap<String, Vec<i64>> = HashMap::new();
    for entry in value.get("m_Container").and_then(Value::as_list).unwrap_or_default() {
        if let Value::Pair(path, info) = entry
            && let (Some(path), Some((0, path_id))) = (path.as_str(), info.get("asset").and_then(Value::as_pptr))
        {
            map.entry(path.to_lowercase()).or_default().push(path_id);
        }
    }
    Ok(map)
}

/// The texture an addressable asset shows, and whether to drop its alpha.
fn texture_of(file: &SerializedFile, path_id: i64) -> Result<(i64, bool), String> {
    let object = file.object(path_id).ok_or("asset is missing from its bundle")?;
    match file.class_id(object) {
        class::TEXTURE_2D => Ok((path_id, false)),
        class::SPRITE => Ok((sprite_texture(file, path_id)?, false)),
        class::MONO_BEHAVIOUR => {
            let value = file.read(object).map_err(|why| why.to_string())?;
            if let Some(material) = value.get("cardMaterial") {
                // Items: the card material's `_MainTex`, whose alpha is a mask.
                let material = follow(file, material)?.ok_or("item has no card material")?;
                let material = file.read(file.object(material).ok_or("material is missing")?).map_err(|why| why.to_string())?;
                let envs = material.at(&["m_SavedProperties", "m_TexEnvs"]).and_then(Value::as_list).unwrap_or_default();
                let main = envs.iter().find_map(|env| match env {
                    Value::Pair(name, texture) if name.as_str().or_else(|| name.get("name").and_then(Value::as_str)) == Some("_MainTex") => texture.get("m_Texture"),
                    _ => None,
                });
                let texture = follow(file, main.ok_or("material has no _MainTex")?)?.ok_or("material's _MainTex is empty")?;
                return Ok((texture, true));
            }
            // Encounters: the portrait, else the background.
            let mut why = "no portrait or background".to_owned();
            for field in ["portraitTextureReference", "backgroundTextureReference"] {
                match value.get(field).map(|pptr| follow(file, pptr)) {
                    Some(Ok(Some(target))) => {
                        let object = file.object(target).ok_or("portrait is missing")?;
                        let texture = if file.class_id(object) == class::SPRITE { sprite_texture(file, target)? } else { target };
                        return Ok((texture, false));
                    }
                    Some(Err(error)) => why = error,
                    _ => {}
                }
            }
            Err(why)
        }
        other => Err(format!("asset is a class {other} object, not art")),
    }
}

fn sprite_texture(file: &SerializedFile, path_id: i64) -> Result<i64, String> {
    let sprite = file.read(file.object(path_id).ok_or("sprite is missing")?).map_err(|why| why.to_string())?;
    follow(file, sprite.at(&["m_RD", "texture"]).ok_or("sprite has no texture")?)?.ok_or_else(|| "sprite's texture is empty".into())
}

/// The object a PPtr points at in this file: None if it is empty. References into other bundles
/// are not followed.
fn follow(file: &SerializedFile, pptr: &Value) -> Result<Option<i64>, String> {
    match pptr.as_pptr() {
        Some((_, 0)) | None => Ok(None),
        Some((0, path_id)) if file.object(path_id).is_some() => Ok(Some(path_id)),
        Some((0, _)) => Err("reference is not in its bundle".into()),
        Some(_) => Err("texture is in another bundle".into()),
    }
}

/// Shrink to `max_size`, keeping the shape, and drop the alpha if `opaque`.
fn render(source: &RgbaImage, opaque: bool, max_size: u32) -> DynamicImage {
    let mut image = source.clone();
    let longest = image.width().max(image.height());
    if longest > max_size {
        let scale = max_size as f32 / longest as f32;
        let (w, h) = (((image.width() as f32 * scale).round() as u32).max(1), ((image.height() as f32 * scale).round() as u32).max(1));
        image = image::imageops::resize(&image, w, h, FilterType::Lanczos3);
    }
    let image = DynamicImage::ImageRgba8(image);
    if opaque { DynamicImage::ImageRgb8(image.to_rgb8()) } else { image }
}

/// Write beside the file and move over it, so a crash never leaves half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = path.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, path)
}

/// `2026-10-31T12:00:00Z`.
pub fn iso_utc(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs()) as i64;
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

#[cfg(test)]
#[path = "art_tests.rs"]
mod tests;
