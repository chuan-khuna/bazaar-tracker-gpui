set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# The gpui app in apps/tracker and its crates: `just tracker <recipe>`
mod tracker 'just/tracker.just'

# The Python scrapers (item odds, runs): `just scraper <recipe>`
mod scraper 'just/scraper.just'

# List the recipes
default:
    @just --list --list-submodules
