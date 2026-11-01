# The Bazaar: Card art storage and the id-to-art mapping

This is a static survey of the shipped Steam build at `H:\SteamLibrary\steamapps\common\The Bazaar`. The game was never launched, and nothing in its folder was written. All bundle reads were made from a scratch directory with UnityPy 1.25.4 under `python -I`. The folder also holds files that are not part of the game (`CLAUDE.md`, `Jules.txt.txt`, `*_dlc.txt`, and a 16 GB `reports/`). They were treated as untrusted data and ignored.

- Engine: Unity **6000.3.11f1** (`UnityPlayer.dll` ProductVersion `6000.3.11f1 (3000ef702840)`, `globalgamemanagers` header). Mono backend (`MonoBleedingEdge/`, `TheBazaar_Data/Managed/*.dll`). `boot.config` `build-guid=d872053b0a07415e997725e142c5722a`.
- Addressables **2.9.1** (`StreamingAssets/aa/settings.json` → `m_AddressablesVersion`).
- `TheBazaarRuntime.dll`: SHA-256 `f9ab344d18572b981d877a59ea7a76b1d379418ff3b1bf1553bd67a24ecbe434`, MVID `16647606-627b-4c58-bce5-39141c8e26ee`.
- `BazaarGameShared.dll`: SHA-256 `c51716ea3ec8c43009ef244a84767f88c14d836efea565b85459e420223a8947`.
- `StreamingAssets/aa/catalog.bin`: SHA-256 `d8cb8303f7723661e63110d7481b29c857365ef384099aa41ba35d2c738e4b34`, build-result hash `d87b6caf8a115a76ed1adae49e471198`.
- Card data: `%USERPROFILE%\AppData\LocalLow\Tempo Storm\The Bazaar\prod\cache\GameData.db` dated 2026-10-28, with 3,386 rows in `cards`.

REA evidence IDs:
- managed artifact (TheBazaarRuntime.dll) `ev_a58c4efb0d7c09e9dc40bcda0385e4d70588faddc3e5e8ce996d2a3db71d5ec9` (summary view `ev_99b5bb51463f2c5aaa125e754a8ab09520b911ad5f3fa257f7bef2f84fee7ceb`)
- managed members, `CardAssetDataSO` + `BaseAssetDataSO` `ev_25cc31ae176048c56820d8261e953dfc73a203c61ea5667ea4f523da218dee3b`
- managed members, `AssetLoader` + `CardArtShaderVariables` + `CardAssetDataSO` + `ItemVisualsController` `ev_ec63bafac0cb81ea2cca25ed4eb1c426a1c8e303e678cb7a0a3dc0c3181aeae9` (summary view `ev_2a8da6d018a7805b20214c791b1807466dd645c676f0eca4442d23db9f20549f`)

How each claim is marked:
- **[V]**: verified from file bytes, a parsed catalog or bundle, decoded textures, metadata, ldstr literals or call edges.
- **[I]**: inferred from names, call shape or Unity/Addressables conventions.

No C# was decompiled. The call edges list callees but do not prove branch order. The whole-assembly REA member inventory is over REA's 64 MB budget, so only the named types were inspected.

## 1. Where the art lives

### 1.1 Layout

| Location | What is there | Card art? |
|---|---|---|
| `TheBazaar_Data/StreamingAssets/aa/StandaloneWindows64/*.bundle` | 931 Addressables asset bundles | **Yes: all card, skill, encounter and reward art** [V] |
| `TheBazaar_Data/StreamingAssets/aa/catalog.bin` (+ `catalog.hash`, `settings.json`) | The Addressables content catalog, in binary format | The address index [V] |
| `StandaloneWindows64/catalog_2026.10.12.23.05.38.{bin,hash}` | The "remote" catalog copy named in `settings.json` | Same build [I] |
| `TheBazaar_Data/sharedassets0.assets(.resS)` (110 MB), `resources.assets(.resS)` (11 MB), `globalgamemanagers.assets` | Scene-0 and `Resources/` data | No card art (no CardData or card texture names) [V] |
| `TheBazaar_Data/StreamingAssets/GameData.db.zip` | A seed copy of the card database | Data only [V] |
| `%LOCALAPPDATA%Low\Tempo Storm\The Bazaar\…` | Logs, the card DB cache, analytics | **No downloaded bundles.** There is no `com.unity.addressables` folder and no `Unity/…/__data` cache [V] |

The runtime can download Addressables content: `AddressablesManager` has the strings `"AddressableCache"`, `"Insufficient disk space for addressable download."` and `"Applied custom Addressables cache root: "`, and `https://data.playthebazaar.com/static` appears in the runtime [V]. This install has no downloaded content, and every card's art resolves to a local bundle (§2.3). So the shipped bundles are the source today [V]. A future patch could move art to remote bundles [I].

### 1.2 Size and count

- `StandaloneWindows64/`: **933 files, 6,016,498,437 bytes (5.6 GiB)**, made up of 931 `.bundle` files and one catalog `.bin`/`.hash` pair [V].
- The bundles that hold the art referenced from `cards` [V]:

| Bundle | Bytes | Holds |
|---|---|---|
| `card_{adventure,common,dooley,jules,karnok,mak,neutral,pygmalien,stelle,thedragons,vanessa}_assets_all.bundle` (11) | 1,337,080,466 | Item `CardAssetDataSO`, card materials, `_D`/`_Mask` textures |
| `skills_assets_all.bundle` | 58,048,666 | Skill icon sprites (256×256) |
| `rewards_assets_all.bundle` | 74,104,321 | Encounter-step reward sprites (1024×1024) |
| `quest_assets_all.bundle` | 148,666,908 | Event `QuestAssetDataSO` + backgrounds |
| `combat_assets_all.bundle` | 152,720,388 | Monster `CombatAssetDataSO` + portraits |
| `merchant_assets_all.bundle` | 116,857,573 | Merchant `MerchantAssetDataSO` + portraits |
| `achievements_…`, `encounter_…` | 11,521,641 | A handful of cards |
| **Total** | **≈1.90 GB** | |

`card_vfx_assets_all.bundle` and `cardframes_assets_all.bundle` hold particle FX and the 3D frame meshes, not card art [V].

### 1.3 Bundle format

The header of every bundle checked is `UnityFS\0`, format version 8, then `"5.x.x"` and **`"0.0.0"`**: the engine revision is stripped [V]. The flags are `0x243`:
- `0x03` means **LZ4HC** block compression [V].
- `0x40` means the blocks and directory info are combined [V].
- `0x200` means the block info needs padding at the start [V].

Each bundle holds one CAB (`CAB-<md5>`) serialized file plus a `CAB-<md5>.resS` stream file holding the texture pixels [V]. Texture offsets in the `.resS` are contiguous. For example, `card_common`'s `.resS` is 48,698,896 bytes, and its 84 textures tile it exactly [V].

The engine version is stripped from the bundle header and from the serialized-file header. A reader must be told `6000.3.11f1`, so UnityPy needs `UnityPy.config.FALLBACK_UNITY_VERSION = "6000.3.11f1"` [V]. Typetrees are present, so `read_typetree()` works on MonoBehaviours without the game's DLLs [V].

### 1.4 Texture formats

These counts come from `card_common` and spot checks of the hero bundles [V]:

| `m_TextureFormat` | Name | Where |
|---|---|---|
| 25 | **BC7** | Nearly all card `_D` art (1024×1024, some 2048×2048 for medium/large), masks, reward sprites, most skill icons |
| 29 | DXT5Crunched | Some skill icons (for example `Icon_Skill_BlightRage`) |
| 12 | DXT5 | Occasional |
| 4 | RGBA32 | Small masks |

All card textures have one mip (`m_MipCount 1`), and their `m_ColorSpace` is 1 (sRGB) [V]. There is no sprite atlas for card art: `m_SpriteAtlas` is null on the skill sprites, and the only `SpriteAtlas` object is in `cardframes` [V]. The only atlas bundle is `ui_sprite_atlas_assets_all.bundle`, which holds UI chrome [I].

### 1.5 Encryption and obfuscation

There is none [V]:
- The bundles are stock UnityFS.
- The CABs parse with stock typetrees.
- BC7 blocks decode to sensible images.
- `TheBazaarRuntime.dll` has no member references to `CreateDecryptor`, `TransformFinalBlock`, `LoadRawTextureData` or `LoadImage`.

The only hurdle is the stripped version string in §1.3.

## 2. How a card id maps to its art

### 2.1 The `ArtKey` field

Every card JSON in `GameData.db` (`SELECT Id, Data FROM cards`) and in `cards.json` has a top-level **`ArtKey`** string [V]. Its counts [V]:
- 3,386 of 3,386 rows have the key.
- 60 of those are empty.
- There are 2,306 distinct values.

The property is `ArtKey` on the shared card model in `BazaarGameShared.dll` (`get_ArtKey`/`set_ArtKey`, `<ArtKey>k__BackingField`). Cosmetics use `OtherArtKeys` and `ECosmeticArtKeys` [V].

`ArtKey` is an **Addressables key**. Its shape depends on the card `Type` [V]:

| Card `Type` | `ArtKey` shape | Addressable resolves to | Art object |
|---|---|---|---|
| `Item` (1,430) | 32-hex Unity **asset GUID**, e.g. `ef57f8899ff555b4d98d03124c1608a7` | `…/Cards/<Name>/<Name>_CardData.asset`, a `TheBazaar.Assets.Scripts.ScriptableObjectsScripts.CardAssetDataSO` | `cardMaterial` → Material `_MainTex` (Texture2D) |
| `Skill` (580), `SocketEffect` (9) | Sprite address, with or without `.png`, e.g. `Icon_Skill_Backroom.png`, `Icon_Skill_BlightRage` | `Assets/TheBazaar/Art/UI/Skills/Shared/<key>.png` (Sprite) | `Sprite.m_RD.texture` |
| `EncounterStep` (584) | Sprite address, e.g. `Reward_Nice_D.png` | `Assets/TheBazaar/Art/UI/Rewards/<key>` (Sprite) | `Sprite.m_RD.texture` |
| `CombatEncounter` (181) | Asset GUID | `…/Monsters/Monster_<X>/Monster_<X>._CombatData.asset` (`CombatAssetDataSO`) | `portraitTextureReference` (Sprite), `backgroundTextureReference` |
| `EventEncounter` (555), `PedestalEncounter` (33) | Asset GUID | `…_QuestData.asset` (`QuestAssetDataSO`) or `MerchantAssetDataSO` | `portraitTextureReference`, which is often empty for events; then `backgroundTextureReference` |
| `PlayerEffect` (14) | Always empty | — | — |

The JSON also has a `Presentation` field (e.g. `"BoardTransition_TempleExpedition_Data_SO.asset"`) on encounters. It is a board-transition key, not card art [V].

`CardAssetDataSO.cardGUID` holds the card's own `Id` (`39f47d29-…` in `CosmicAmulet_CardData`). So the link runs both ways: `ArtKey` → asset, and asset → card id [V]. The fields of `CardAssetDataSO` are `cardMaterial` (Material), `cardGUID` (string), `dailyWeeklyImageReference` (AssetReference), and, from `CollectibleAssetDataSO`/`BaseAssetDataSO`, `InternalName`, `Name`, `Rarity`, `Season`, `CustomAddressableLabels`, `InspectionZoomFactor` and `MintCap` [V, typetree and REA fields].

### 2.2 The Addressables binary catalog

`catalog.bin` is the Addressables 2.x binary `ContentCatalogData`. The format below was reverse-engineered and checked by resolving every card [V]:

- Header (8 × u32 LE): `magic 0x0DE38942`, `version 2`, `keysOffset 0x24`, `idOffset`, `instanceProviderOffset`, `sceneProviderOffset`, `initObjectsArrayOffset`, `buildResultHashOffset`.
- **An offset points at data, and the u32 before it is that data's byte length** (`BinaryStorageBuffer`).
- The keys are an array of `{u32 keyNameOffset, u32 locationSetOffset}`. This catalog has **24,219 keys**.
  - A key name is an object `{u32 typeId, u32 objectId}`.
  - `typeId` points to `{assemblyId, classId}` strings.
  - For `System.String` keys, `objectId` points to a u32 string id.
- A string id is an offset with flags:
  - Bit 31 set means UTF-16; clear means ASCII.
  - **Bit 30 set means a dynamic string: a linked list of `{u32 partId, u32 nextId}` nodes, stored last part first.** Reverse the parts, then join them with the separator. The separator is `/` for ids and keys and `.` for provider and type names.
- A location set is a u32 array of location offsets. A location is `{primaryKey, internalId, providerId, dependencySet, i32 dependencyHash, extraData, typeId}`.
  - For a bundled asset, `internalId` is the asset path, and `dependencySet[0]` is the location of its own bundle. Its `internalId` is `{UnityEngine.AddressableAssets.Addressables.RuntimePath}\StandaloneWindows64\<bundle>.bundle`.
- Keys with underscores come back with `/` in place of `_` when joined with `/`. So `Icon_Skill_Backroom.png` is stored as `Icon/Skill/Backroom.png`. Normalise `/`→`_` before matching, or match on the GUID or primary key [V].

### 2.3 Coverage

The catalog was matched against `ArtKey`, `ArtKey` without its extension, and `ArtKey + ".png"`. Results by card type [V]:

| Type | Resolved | Empty key | Unresolved |
|---|---|---|---|
| Item | 1,407 | 20 | 3 (`Placeholder`) |
| Skill | 574 | 5 | 1 |
| EncounterStep | 564 | 13 | 7 (`Reward_Trap.png`, `Reward_Core.png`, a full-path key, …) |
| EventEncounter | 545 | 5 | 5 |
| CombatEncounter | 178 | 3 | 0 |
| PedestalEncounter | 33 | 0 | 0 |
| SocketEffect | 9 | 0 | 0 (all `Icon_Skill_Placeholder.png`) |
| **All** | **3,310 / 3,386** | 60 | 16 |

### 2.4 Worked examples

Each chain below was resolved from the DB, through the catalog, into the bundle and on to the texture, and the texture was decoded to a file [V]:

| Card id | Title | `ArtKey` | Catalog internalId → bundle | Art |
|---|---|---|---|---|
| `0022c409-c839-41e8-8022-65a407457dfe` | Eagle Talisman (Item) | `ef57f8899ff555b4d98d03124c1608a7` | `Assets/TheBazaar/Art/Heroes/Adventure/Cards/EagleTalisman/EagleTalisman_CardData.asset` → `card_adventure_assets_all.bundle` | `cardMaterial` = `CF_S_NTR_EagleTalisman` → `_MainTex` = `CF_S_NTR_EagleTalisman_D`, 1024², BC7 |
| `39f47d29-2ef3-4057-bb60-33acd3dd12a2` | Cosmic Amulet (Item) | `c071a2553d11f644c9470418fb14f078` | `…/Heroes/Common/Cards/CosmicAmulet/CosmicAmulet_CardData.asset` → `card_common_assets_all.bundle` | `CF_S_ADV_CosmicAmulet` → `CF_S_ADV_CosmicAmulet1`, 1024², BC7 |
| `004cb876-1ed2-4a4b-88d4-475cea76a03d` | Airplane Glue (Item) | `12f812675deb51c4a9a3d05e536ebc06` | `…/Heroes/Stelle/Cards/Glue/Glue_CardData.asset` → `card_stelle_assets_all.bundle` | `CF_S_STE_Glue` → `CF_S_STE_Glue_D`, 1024², BC7 |
| `0045d15e-ac74-436f-a200-eeedab7baf90` | Backroom Dealings (Skill) | `Icon_Skill_Backroom.png` | `Assets/TheBazaar/Art/UI/Skills/Shared/Icon_Skill_Backroom.png` → `skills_assets_all.bundle` | Sprite → Texture2D `Icon_Skill_Backroom`, 256², BC7 |
| `0012231c-bc8a-4367-a652-378440652e11` | Blight Rage (Skill) | `Icon_Skill_BlightRage` (no extension) | `…/Skills/Shared/Icon_Skill_BlightRage.png` → `skills_assets_all.bundle` | Sprite → `Icon_Skill_BlightRage`, 256², DXT5Crunched |
| `01982309-095f-4d37-b550-634ab6d983f0` | Return It (EncounterStep) | `Reward_Nice_D.png` | `Assets/TheBazaar/Art/UI/Rewards/Reward_Nice_D.png` → `rewards_assets_all.bundle` | Texture2D `Reward_Nice_D`, 1024², BC7 |
| `035ace94-61e5-4343-8383-83044ea4e08e` | Haunted Kimono (CombatEncounter) | `787a82f15dd4cf54fa2c69de42f2bb67` | `…/Monsters/Monster_HauntedKimono/Monster_HauntedKimono._CombatData.asset` → `combat_assets_all.bundle` | `portraitTextureReference` → Sprite → texture |

### 2.5 Item textures: use RGB, not alpha

The alpha of an item `_D` texture is **not** transparency [V]:
- Across 556 items in four hero bundles, a mean of 64% of pixels have alpha 0.
- In `CF_S_VAN_Grenade_D`, alpha is 1 only on the pin ring, while the RGB holds the whole grenade.
- The raw BC7 data agrees: the centre blocks of `CF_S_STE_Glue_D` are mode 6 with both alpha endpoints 0.

The material floats `_ItemCutoutBias` (≈0.39 on every card checked) and the enchantment keywords (`_ENCHANTMENTSTATUS_*`) suggest that alpha is the *item cut-out mask* that drives enchantment, pop-out and FX layers [I]. **Display the RGB channels with alpha forced to opaque.** A random 25-item sample rendered this way looked right in every case [V]. Two items, Eagle Talisman and the `Glue_CardData` asset that Airplane Glue points at, have smeared colour under the mask. Unity's alpha-is-transparency dilation produces exactly this pattern, so their full art is not in the RGB [V for the pixels, I for the cause]. How common this is is unknown (§5).

Skill icons, reward sprites and monster portraits use real transparency. Keep their alpha [V].

## 3. How the game resolves it

| Step | Type and member | Evidence |
|---|---|---|
| Card model exposes the key | `BazaarGameShared` card model `ArtKey` (and `OtherArtKeys`) | [V] metadata strings |
| Loader | `AssetLoader` (global namespace): `ConstructAndInstantiateCard`, `…Skill`, `…Encounter`, `…SocketEffect`, `ConstructInstantiateReward`, `PreloadCard`, `DoesAddressExist`, `LoadAssetAsyncByAddress`, `LoadAssetAsyncByReference`, `LoadSpriteAsyncByReference`, `LoadAndCache`, `GetScopedKey` | [V] REA methods |
| Address → asset | `AssetLoader.LoadAssetHandleByReference` → `AssetReference.get_AssetGUID`, `RuntimeKeyIsValid` → `Addressables.LoadAssetAsync` | [V] REA call edges |
| Sprite sub-asset | ldstr `"::Sprite"` in `AssetLoader`. Unity's `address[SubObject]`/sprite addressing for keys like `Icon_Skill_X.png` | [V] literal, [I] use |
| Missing key | `"No ArtKeys for thisCard {0} {1}"`, `"[Portrait] Failed to load portrait data for artKey='{0}' (CombatantId={1})."` | [V] #US strings |
| Item visuals | `TheBazaar.Game.CardFrames.ItemVisualsController.GetCardAssetData` (async) → `SetCardFrameMaterial` → `Object.Instantiate` + `Renderer.set_sharedMaterial` | [V] REA call edges |
| Enchantments | `TheBazaar.Utilities.Shaders.CardArtShaderVariables.ApplyEnchantment`/`ClearEnchantmentKeywords` → `Material.EnableKeyword`/`DisableKeyword` with `_ENCHANTMENTSTATUS_{UNENCHANTED,GOLDEN,PRISMATIC,ICY,SHINY,TURBO,FIERY,…}`, `_ISPREMIUM_ON` | [V] REA literals and edges |

So the game turns `ArtKey` into an Addressables load of the `CardAssetDataSO`. It instantiates that asset's `cardMaterial` onto the card-frame renderer, and the card shader samples `_MainTex` [I for the order, since the async bodies live in state-machine types that were not inspected].

## 4. Porting notes for Rust/GPUI [I]

- Keep the game's key, not our own: add `art_key: Option<String>` to `CardInfo` (`crates/bazaar-tracker/src/catalog/mod.rs`, `parse_card` reads `json["ArtKey"]`). It is the stable join to the catalog. The file name can still be the card id.
- Do not parse bundles in the UI process. Use the offline extraction in [card-art-extraction.md](card-art-extraction.md), and have the app only open `<data dir>/card-art/<id>.webp`.
- Item art is square (1024²). The board tiles are 1, 2 or 3 slots wide and the art is not size-specific, so draw it with `object_fit(Cover)` under the tier border.

## 5. Unknowns

- **Items whose RGB is damaged under the mask.** Two were found (Eagle Talisman `0022c409…`, Airplane Glue `004cb876…` via `Glue_CardData`). A gradient heuristic does not separate them cleanly, so their number is unknown. The game must draw these subjects from somewhere, but a name scan of every bundle except `trailers_assets_all.bundle` (466 MB, the only one over 200 MB) found no other Texture2D, Sprite or Material named after them [V]. Grenade-named assets turned up only as each hero's own `_D`/`_Mask` set plus projectile FX. A runtime capture would settle it, but that was out of scope.
- Encounter cards: 250 `QuestAssetDataSO`/`MerchantAssetDataSO`/`CombatAssetDataSO` have no local `portraitTextureReference`. Their `backgroundTextureReference` is set, but extracting it was not run.
- Which call sites read `get_ArtKey`: the member reference has 2 uses in `TheBazaarRuntime.dll` [V], but the callers sit in async state machines and were not named.
- Whether the "remote" catalog path (`catalog_2026.10.12.23.05.38.*`) is ever fetched over the network, and whether future patches will move art to `data.playthebazaar.com`.
- The 16 unresolved `ArtKey`s (`Placeholder`, `Reward_Trap.png`, `Reward_Core.png`, `Reward_Reagent.png`, `Reward_Shield_D.png`, a full `Assets/…` path, three GUIDs) were not traced further.
