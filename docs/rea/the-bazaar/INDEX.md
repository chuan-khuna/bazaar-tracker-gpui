# The Bazaar — Reverse Engineering Index

These are static reverse-engineering notes on the shipped files of Tempo Storm's **The Bazaar** (Steam, Windows, Unity 6000.3.11f1, Mono scripting backend, Addressables 2.9.1). They cover how the game stores card art, how a card id from `GameData.db` reaches its texture, and how this app could show that art. The game was never launched and nothing in its folder was changed. Each doc marks its claims as verified or inferred and gives the type, field and file names involved.

- [Card art](card-art.md) — where the art lives (931 LZ4HC UnityFS bundles under `StreamingAssets/aa`, a binary Addressables catalog, BC7 textures, no encryption), the `ArtKey` field and how each card type resolves it, five worked card-id-to-texture examples, and the game code that loads it.
- [Card art extraction](card-art-extraction.md) — an offline UnityPy pipeline that writes `<card_id>.webp` into the data folder (3,059 cards in 171 s, 107 MB), a comparison of tools (AssetRipper, AssetStudio, UnityPy, Rust crates), how to redo it after patches, and where the lookup plugs into the catalog and board UI.
