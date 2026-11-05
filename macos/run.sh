#!/bin/sh
# Cargo's runner on macOS (.cargo/config.toml): sign the binary it built, then run it with its arguments.
set -eu
"$(dirname "$0")/sign.sh" "$1"
exec "$@"
