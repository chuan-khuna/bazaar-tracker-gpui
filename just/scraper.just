# The Python scrapers (item odds, runs) in app/scraper: `just scraper <recipe>`. Recipes run from app/scraper.

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set working-directory := "../app/scraper"

# List the scraper's recipes
default:
    @just --justfile "{{justfile()}}" --list scraper

# Install the scraper's dependencies into app/scraper/.venv
sync:
    uv sync

# Save item odds to .bazaar-tracker/raw_data/scraper/odds/: with no args it asks what to scrape, with args it runs straight away (see `just scraper help`); needs the items.json the app writes
scrape *args:
    uv run scrape-odds {{args}}

# Print what a scrape would ask for, without asking
list *args:
    uv run scrape-odds --list {{args}}

# Rewrite .bazaar-tracker/raw_data/scraper/odds/index.json from the saved files
index:
    uv run scrape-odds --index-only

# Print every option of the scrape
help:
    uv run scrape-odds --help

# Install Playwright's own Chromium, for `just scraper scrape --browser chromium`
chromium:
    uv run playwright install chromium

# Save Bazaar DB runs to .bazaar-tracker/raw_data/scraper/builds/: with no args it asks which profiles and feeds, with args it runs straight away (see `just scraper runs --help`)
runs *args:
    uv run scrape-runs {{args}}

# Start a real browser (Dia on macOS, else Chrome, Edge or Brave) with the debugging port the scrapers attach to; `--print` only prints the commands
browser *args:
    uv run start-browser {{args}}
