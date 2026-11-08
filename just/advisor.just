# The Build Advisor in apps/advisor (Python, uv): `just advisor <recipe>`. Recipes run from apps/advisor.

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set working-directory := "../apps/advisor"

# List the Advisor's recipes
default:
    @just --justfile "{{justfile()}}" --list advisor

# Install the Advisor's dependencies into apps/advisor/.venv
sync:
    uv sync

# Ask the Advisor one question in the terminal: -m "…" [--situation file.json] [--model id] [--fake]
run *args:
    uv run advisor {{args}}

# Run the Advisor's tests
test *args:
    uv run pytest {{args}}
