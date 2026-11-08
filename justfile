set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# The gpui app and its crates: `just app <recipe>`
mod app 'just/app.just'

# The Python scrapers (item odds, runs): `just scraper <recipe>`
mod scraper 'just/scraper.just'

# List the recipes
default:
    @just --list --list-submodules
