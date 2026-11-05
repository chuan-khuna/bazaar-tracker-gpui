set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# List the recipes
default:
    @just --list

# Run the app (a debug build keeps its data in .bazaar-tracker/)
run *args:
    cargo run -- {{args}}

# Test every crate in the workspace
test:
    cargo test --workspace

# Run the memory-reader spike against the running game
spike *args:
    cargo run -p unityspy-rs --example dump-classes -- {{args}}

# Build the app in release mode, into target/release/
[windows]
build:
    cargo build --release

# Build the app in release mode, into target/release/, signed so it may read the game's memory
[macos]
build:
    cargo build --release
    macos/sign.sh target/release/bazaar-tracker-gpui

alias release := build

# Start the release build from `just build` without rebuilding; it keeps its data in .bazaar-tracker/ as `just run` does
[windows]
app *args:
    $env:BAZAAR_TRACKER_DATA_DIR = "{{justfile_directory()}}/.bazaar-tracker"; & "{{justfile_directory()}}/target/release/bazaar-tracker-gpui.exe" {{args}}

# Start the release build from `just build` without rebuilding; it keeps its data in .bazaar-tracker/ as `just run` does
[macos]
app *args:
    BAZAAR_TRACKER_DATA_DIR="{{justfile_directory()}}/.bazaar-tracker" "{{justfile_directory()}}/target/release/bazaar-tracker-gpui" {{args}}

# Delete the build output in target/ (Cargo's cache of compiled crates)
clean:
    cargo clean

# Print a live Snapshot of the running game and the current run's id
snapshot *args:
    cargo run -p bazaar-tracker --example snapshot -- {{args}}

# Extract the game's card art into .bazaar-tracker/art (local use only). Finds the game in Steam, or pass --game <install folder>; --force redraws every image
art *args:
    cargo run --release -p bazaar-tracker --example extract_art -- {{args}}

# Follow the live game headlessly and print each Event as it is saved
track *args:
    cargo run -p bazaar-tracker --example track -- {{args}}

# Print Bazaar DB's shop odds; pass the shop-odds answer saved from a browser if the API refuses, and --json for JSON
shop-odds *args:
    cargo run -p bazaardb --example shop_odds -- {{args}}
