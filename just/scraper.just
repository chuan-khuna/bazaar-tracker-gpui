# The Python scrapers (item odds, runs) in apps/scraper: `just scraper <recipe>`. Recipes run from apps/scraper.
#
# Recipes that take arguments pass each one on as it was typed, so a quoted feed link with `&` in it stays one
# argument: on Windows through a PowerShell script splatting `$args`, elsewhere through "$@".

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set working-directory := "../apps/scraper"

# List the scraper's recipes
default:
    @just --justfile "{{justfile()}}" --list scraper

# Install the scraper's dependencies into apps/scraper/.venv
sync:
    uv sync

# Save item odds to .bazaar-tracker/raw_data/scraper/odds/: with no args it asks what to scrape, with args it runs straight away (see `just scraper help`); needs the items.json the app writes
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
scrape *args:
    uv run scrape-odds @args
    exit $LASTEXITCODE

# Save item odds to .bazaar-tracker/raw_data/scraper/odds/: with no args it asks what to scrape, with args it runs straight away (see `just scraper help`); needs the items.json the app writes
[unix]
[positional-arguments]
scrape *args:
    uv run scrape-odds "$@"

# Print what a scrape would ask for, without asking
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
list *args:
    uv run scrape-odds --list @args
    exit $LASTEXITCODE

# Print what a scrape would ask for, without asking
[unix]
[positional-arguments]
list *args:
    uv run scrape-odds --list "$@"

# Rewrite .bazaar-tracker/raw_data/scraper/odds/index.json from the saved files
index:
    uv run scrape-odds --index-only

# Print every option of the scrape
help:
    uv run scrape-odds --help

# Install Playwright's own Chromium, for `just scraper scrape --browser chromium`
chromium:
    uv run playwright install chromium

# Save Bazaar DB runs to .bazaar-tracker/raw_data/scraper/builds/: with no args it asks which profiles and feeds, with args it runs straight away (see `just scraper runs --help`); quote a feed link
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
runs *args:
    uv run scrape-runs @args
    exit $LASTEXITCODE

# Save Bazaar DB runs to .bazaar-tracker/raw_data/scraper/builds/: with no args it asks which profiles and feeds, with args it runs straight away (see `just scraper runs --help`); quote a feed link
[unix]
[positional-arguments]
runs *args:
    uv run scrape-runs "$@"

# Start a real browser (Dia on macOS, else Chrome, Edge or Brave) with the debugging port the scrapers attach to; `--print` only prints the commands
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
browser *args:
    uv run start-browser @args
    exit $LASTEXITCODE

# Start a real browser (Dia on macOS, else Chrome, Edge or Brave) with the debugging port the scrapers attach to; `--print` only prints the commands
[unix]
[positional-arguments]
browser *args:
    uv run start-browser "$@"
