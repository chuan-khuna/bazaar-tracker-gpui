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

# Build the app in release mode
release:
    cargo build --release

# Print a live Snapshot of the running game and the current run's id
snapshot *args:
    cargo run -p bazaar-tracker --example snapshot -- {{args}}

# Follow the live game headlessly and print each Event as it is saved
track *args:
    cargo run -p bazaar-tracker --example track -- {{args}}
