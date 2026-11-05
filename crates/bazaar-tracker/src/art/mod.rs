//! Card art: extract each card's picture from the player's own install of The Bazaar into
//! `<out>/<category>/<key>.webp`, with a `manifest.json` beside them.
//!
//! A card's `ArtKey` (from the card catalog) is an Addressables key. The catalog
//! ([`addressables`]) names the asset and its bundle; the bundle ([`unity`]) holds the asset, which
//! leads to a texture: an item's card material's `_MainTex`, a skill's or reward's sprite, an
//! encounter's portrait drawn over its background. A reference into another bundle is followed there. How the game stores all this:
//! docs/rea/the-bazaar/card-art.md.
//!
//! Item art is kept square, as the game stores it: the app stretches it to the card's shape when it
//! draws it. It is kept opaque, as the alpha of the game's item art is a cut-out mask. Everything else keeps its transparency.
//!
//! The art is Tempo Storm's: it is for showing in this app only, never to be shared.

pub mod addressables;
pub mod unity;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use image::{DynamicImage, RgbaImage, imageops::FilterType};
use serde_json::{Value as Json, json};

use crate::catalog::{CardCatalog, CardInfo};
use addressables::{Catalog, Location};
use unity::serialized::{ObjectInfo, SerializedFile, Value, class};

/// Bumped when the way images are drawn changes, so a run redraws every image.
pub const RENDER_VERSION: u64 = 4;

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

/// The Bazaar's install folder: `setting` if it names one, else the first Steam library that has
/// it, else the folder of the game while it runs (an install outside Steam).
pub fn find_game_dir(setting: &str) -> Option<PathBuf> {
    let is_game = |path: &Path| path.join("TheBazaar_Data").is_dir();
    if !setting.trim().is_empty() {
        return Some(PathBuf::from(setting.trim())).filter(|path| is_game(path));
    }
    steam_libraries()
        .into_iter()
        .map(|library| library.join("steamapps").join("common").join("The Bazaar"))
        .find(|path| is_game(path))
        .or_else(|| running_game_dir().filter(|path| is_game(path)))
}

/// The folder of the running game's executable, if the game is running.
#[cfg(any(windows, target_os = "macos"))]
fn running_game_dir() -> Option<PathBuf> {
    let process = unityspy_rs::mem::process::Process::open_by_name(crate::memory::game::PROCESS_NAME).ok()?;
    process.image_path().ok()?.parent().map(Path::to_path_buf)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn running_game_dir() -> Option<PathBuf> {
    None
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
    let index = CabIndex::new();

    std::thread::scope(|scope| {
        for _ in 0..options.workers.max(1) {
            scope.spawn(|| {
                loop {
                    let Some((bundle, bundle_jobs)) = queue.lock().expect("no worker panics holding the queue").pop() else { break };
                    let results = extract_bundle(&bundles_dir, &bundle, &bundle_jobs, options, &index, &addressables);
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

/// Which bundle holds each serialized file (`CAB-<hash>`, lower-cased), read from the bundles'
/// tables. Built when a reference first leads out of a bundle.
type CabIndex = OnceLock<HashMap<String, String>>;

fn cab_index(dir: &Path) -> HashMap<String, String> {
    let mut index = HashMap::new();
    let Ok(listing) = fs::read_dir(dir) else { return index };
    for path in listing.flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|ext| ext == "bundle")) {
        let Some(bundle) = path.file_name().map(|name| name.to_string_lossy().into_owned()) else { continue };
        let mut head = vec![0; 64 * 1024];
        let names = fs::File::open(&path)
            .and_then(|mut file| {
                let read = file.read(&mut head)?;
                head.truncate(read);
                Ok(())
            })
            .ok()
            .and_then(|()| unity::bundle::file_names(&head).ok())
            .or_else(|| fs::read(&path).ok().and_then(|bytes| unity::bundle::read(&bytes).ok()).map(|entries| entries.into_iter().map(|entry| entry.path).collect()));
        for name in names.unwrap_or_default() {
            if !is_stream(&name) {
                index.insert(name.rsplit('/').next().unwrap_or(&name).to_lowercase(), bundle.clone());
            }
        }
    }
    index
}

fn is_stream(path: &str) -> bool {
    path.ends_with(".resS") || path.ends_with(".resource")
}

/// A bundle's files, unpacked.
struct Opened {
    entries: Vec<unity::bundle::Entry>,
}

impl Opened {
    fn read(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|why| format!("could not read {}: {why}", path.display()))?;
        Ok(Self { entries: unity::bundle::read(&bytes).map_err(|why| format!("bundle: {why}"))? })
    }

    fn main(&self) -> Result<&unity::bundle::Entry, String> {
        self.entries.iter().find(|entry| !is_stream(&entry.path)).ok_or_else(|| "bundle has no serialized file".into())
    }

    fn stream(&self, name: &str) -> Option<&[u8]> {
        self.entries.iter().find(|entry| entry.path.rsplit('/').next() == Some(name)).map(|entry| entry.data.as_slice())
    }
}

/// Opens the bundles that references lead to, each once per bundle read.
struct Others<'a> {
    dir: &'a Path,
    index: &'a CabIndex,
    /// Resolves the Addressables references in assets (an item's card material).
    catalog: &'a Catalog,
    open: RefCell<HashMap<String, Rc<Opened>>>,
    /// The same, by bundle file name, for references through the catalog.
    bundles: RefCell<HashMap<String, Rc<Opened>>>,
}

impl Others<'_> {
    /// The bundle called `name`, in the bundles folder.
    fn bundle(&self, name: &str) -> Result<Rc<Opened>, String> {
        if let Some(opened) = self.bundles.borrow().get(name) {
            return Ok(opened.clone());
        }
        let opened = Rc::new(Opened::read(&self.dir.join(name))?);
        self.bundles.borrow_mut().insert(name.to_owned(), opened.clone());
        Ok(opened)
    }

    fn get(&self, cab: &str) -> Result<Rc<Opened>, String> {
        let cab = cab.to_lowercase();
        if let Some(opened) = self.open.borrow().get(&cab) {
            return Ok(opened.clone());
        }
        let bundle = self.index.get_or_init(|| cab_index(self.dir)).get(&cab).ok_or_else(|| format!("no bundle holds {cab}"))?;
        let opened = Rc::new(Opened::read(&self.dir.join(bundle))?);
        self.open.borrow_mut().insert(cab, opened.clone());
        Ok(opened)
    }
}

/// A serialized file being read: which one (its `CAB-<hash>`), and its bundle for the streams.
#[derive(Clone, Copy)]
struct Here<'a> {
    cab: &'a str,
    file: &'a SerializedFile<'a>,
    opened: &'a Opened,
}

/// Decoded textures, by file and path id: many cards share one.
type Decoded = HashMap<(String, i64), Rc<RgbaImage>>;

/// Run `then` on the object a PPtr `(file id, path id)` points at, in whichever bundle holds it.
fn at<R>(others: &Others, here: Here, (file_id, path_id): (i32, i64), then: impl for<'b> FnOnce(Here<'b>, &'b ObjectInfo) -> Result<R, String>) -> Result<R, String> {
    if file_id == 0 {
        let object = here.file.object(path_id).ok_or("reference is not in its bundle")?;
        return then(here, object);
    }
    let cab = here.file.external(file_id).ok_or("reference names no file")?.to_owned();
    let opened = others.get(&cab)?;
    let file = SerializedFile::parse(&opened.main()?.data).map_err(|why| format!("serialized file: {why}"))?;
    let object = file.object(path_id).ok_or_else(|| format!("{cab} has no object {path_id}"))?;
    then(Here { cab: &cab, file: &file, opened: &opened }, object)
}

/// A PPtr, or None if it is empty.
fn pptr(value: Option<&Value>) -> Option<(i32, i64)> {
    value.and_then(Value::as_pptr).filter(|(_, path_id)| *path_id != 0)
}

/// The pixels of a texture, or of the texture a sprite is cut from.
fn texture(others: &Others, here: Here, target: (i32, i64), decoded: &mut Decoded) -> Result<Rc<RgbaImage>, String> {
    at(others, here, target, |here, object| match here.file.class_id(object) {
        class::TEXTURE_2D => {
            let key = (here.cab.to_lowercase(), object.path_id);
            if let Some(image) = decoded.get(&key) {
                return Ok(image.clone());
            }
            let value = here.file.read(object).map_err(|why| why.to_string())?;
            let image = Rc::new(unity::texture::decode(&value, |name| here.opened.stream(name)).map_err(|why| why.to_string())?);
            decoded.insert(key, image.clone());
            Ok(image)
        }
        class::SPRITE => {
            let sprite = here.file.read(object).map_err(|why| why.to_string())?;
            let target = pptr(sprite.at(&["m_RD", "texture"])).ok_or("sprite has no texture")?;
            texture(others, here, target, decoded)
        }
        other => Err(format!("a class {other} object is not a picture")),
    })
}

/// The `_MainTex` of `object`, a material.
fn main_texture(others: &Others, here: Here, object: &ObjectInfo, decoded: &mut Decoded) -> Result<Rc<RgbaImage>, String> {
    let material = here.file.read(object).map_err(|why| why.to_string())?;
    let envs = material.at(&["m_SavedProperties", "m_TexEnvs"]).and_then(Value::as_list).unwrap_or_default();
    let main = envs.iter().find_map(|env| match env {
        Value::Pair(name, texture) if name.as_str().or_else(|| name.get("name").and_then(Value::as_str)) == Some("_MainTex") => Some(texture.get("m_Texture")),
        _ => None,
    });
    let main = pptr(main.ok_or("material has no _MainTex")?).ok_or("material's _MainTex is empty")?;
    texture(others, here, main, decoded)
}

/// Run `then` on the object an Addressables location names, in its own bundle: the first of the
/// objects its container lists under the asset's path that `then` takes.
fn at_address<R>(others: &Others, location: &Location, mut then: impl for<'b> FnMut(Here<'b>, &'b ObjectInfo) -> Result<R, String>) -> Result<R, String> {
    let opened = others.bundle(&location.bundle)?;
    let main = opened.main()?;
    let cab = main.path.rsplit('/').next().unwrap_or(&main.path).to_owned();
    let file = SerializedFile::parse(&main.data).map_err(|why| format!("serialized file: {why}"))?;
    let path_ids = container(&file)?.remove(&location.asset.to_lowercase()).ok_or_else(|| format!("{} is not in {}", location.asset, location.bundle))?;
    let here = Here { cab: &cab, file: &file, opened: &opened };
    let mut first_error = None;
    for path_id in path_ids {
        let Some(object) = file.object(path_id) else { continue };
        match then(here, object) {
            Ok(found) => return Ok(found),
            Err(why) => {
                first_error.get_or_insert(why);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| format!("{} has no objects", location.asset)))
}

/// A card's picture from the asset its `ArtKey` names, and whether to drop its alpha. Every layer
/// of the picture is merged into one image; the enchantment and effect masks are not part of it.
fn art(others: &Others, here: Here, path_id: i64, decoded: &mut Decoded) -> Result<(RgbaImage, bool), String> {
    let object = here.file.object(path_id).ok_or("asset is missing from its bundle")?;
    match here.file.class_id(object) {
        class::TEXTURE_2D | class::SPRITE => Ok(((*texture(others, here, (0, path_id), decoded)?).clone(), false)),
        class::MONO_BEHAVIOUR => {
            let value = here.file.read(object).map_err(|why| why.to_string())?;
            // Items: the card material's `_MainTex`, whose alpha is a mask. The material's other
            // textures are the enchantment and effect masks and the effect layers. Before the
            // 2026-11-05 patch the material was linked directly (`cardMaterial`); since, it is an
            // Addressables reference (`cardMaterialReference`) whose GUID the catalog resolves.
            if let Some(material) = value.get("cardMaterial") {
                let material = pptr(Some(material)).ok_or("item has no card material")?;
                let image = at(others, here, material, |here, object| main_texture(others, here, object, decoded))?;
                return Ok(((*image).clone(), true));
            }
            if let Some(reference) = value.get("cardMaterialReference") {
                let guid = reference.get("m_AssetGUID").and_then(Value::as_str).filter(|guid| !guid.is_empty()).ok_or("item's card material reference is empty")?;
                let location = others.catalog.resolve(guid).ok_or_else(|| format!("card material {guid} is not in the Addressables catalog"))?;
                let image = at_address(others, location, |here, object| main_texture(others, here, object, decoded))?;
                return Ok(((*image).clone(), true));
            }
            // Encounters: the portrait drawn over the background, which share one square frame.
            // The carpet (`encounterCarpet`) is the board's floor, not part of the picture.
            let mut layers = Vec::new();
            for field in ["backgroundTextureReference", "portraitTextureReference"] {
                if let Some(target) = pptr(value.get(field)) {
                    layers.push(texture(others, here, target, decoded).map_err(|why| format!("{field}: {why}"))?);
                }
            }
            merge(&layers).map(|image| (image, false)).ok_or_else(|| "no portrait or background".into())
        }
        other => Err(format!("asset is a class {other} object, not art")),
    }
}

/// Layers drawn bottom to top, each stretched to the first one's size.
fn merge(layers: &[Rc<RgbaImage>]) -> Option<RgbaImage> {
    let (base, rest) = layers.split_first()?;
    let mut image = (**base).clone();
    for layer in rest {
        if layer.dimensions() == image.dimensions() {
            image::imageops::overlay(&mut image, &**layer, 0, 0);
        } else {
            let layer = image::imageops::resize(&**layer, image.width(), image.height(), FilterType::Lanczos3);
            image::imageops::overlay(&mut image, &layer, 0, 0);
        }
    }
    Some(image)
}

/// Write each job's image from one bundle. One result per job: the bytes written, or why not.
fn extract_bundle(dir: &Path, bundle: &str, jobs: &[Job], options: &Options, index: &CabIndex, catalog: &Catalog) -> Vec<Result<u64, String>> {
    let fail_all = |why: String| jobs.iter().map(|_| Err(why.clone())).collect();
    let opened = match Opened::read(&dir.join(bundle)) {
        Ok(opened) => opened,
        Err(why) => return fail_all(why),
    };
    let main = match opened.main() {
        Ok(main) => main,
        Err(why) => return fail_all(why),
    };
    let cab = main.path.rsplit('/').next().unwrap_or(&main.path).to_owned();
    let file = match SerializedFile::parse(&main.data) {
        Ok(file) => file,
        Err(why) => return fail_all(format!("serialized file: {why}")),
    };
    let container = match container(&file) {
        Ok(container) => container,
        Err(why) => return fail_all(format!("container: {why}")),
    };
    let here = Here { cab: &cab, file: &file, opened: &opened };
    let others = Others { dir, index, catalog, open: RefCell::new(HashMap::new()), bundles: RefCell::new(HashMap::new()) };

    let mut decoded = Decoded::new();
    jobs.iter()
        .map(|job| {
            let path_ids = container.get(&job.asset.to_lowercase()).ok_or("asset is not in its bundle")?;
            // An address can list several objects (a texture and its sprite): the first that is art.
            let mut first_error = None;
            let (picture, opaque) = path_ids
                .iter()
                .find_map(|id| match art(&others, here, *id, &mut decoded) {
                    Ok(found) => Some(found),
                    Err(why) => {
                        first_error.get_or_insert(why);
                        None
                    }
                })
                .ok_or_else(|| first_error.unwrap_or_else(|| "asset has no objects".into()))?;
            let image = render(&picture, opaque || job.item, options.max_size);
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
