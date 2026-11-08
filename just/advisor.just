# The Build Advisor in apps/advisor (Python, uv): `just advisor <recipe>`. Recipes run from apps/advisor.
#
# Recipes that take arguments pass each one on as it was typed, so `-m "two words"` stays one argument:
# on Windows through a PowerShell script splatting `$args`, elsewhere through "$@".

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set working-directory := "../apps/advisor"

# List the Advisor's recipes
default:
    @just --justfile "{{justfile()}}" --list advisor

# Install the Advisor's dependencies into apps/advisor/.venv
sync:
    uv sync

# Ask the Advisor one question in the terminal: -m "…" [--situation file.json] [--model id] [--fake]
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
run *args:
    uv run advisor @args
    exit $LASTEXITCODE

# Ask the Advisor one question in the terminal: -m "…" [--situation file.json] [--model id] [--fake]
[unix]
[positional-arguments]
run *args:
    uv run advisor "$@"

# Run the service on 127.0.0.1:8420 (ADVISOR_PORT) for the tracker's Advisor tab: [--fake]
serve *args:
    uv run advisor-serve {{args}}

# Run the Advisor's tests
test *args:
    uv run pytest {{args}}

# Import Scraped runs into history.db and recompute their stats: [--db PATH] [--builds DIR] [--min-support N]
[windows]
[positional-arguments]
[script("powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
[extension(".ps1")]
import *args:
    uv run advisor-import @args
    exit $LASTEXITCODE

# Import Scraped runs into history.db and recompute their stats: [--db PATH] [--builds DIR] [--min-support N]
[unix]
[positional-arguments]
import *args:
    uv run advisor-import "$@"
