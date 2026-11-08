#!/bin/sh
# Ad-hoc sign a binary with the entitlement that lets it read the game's memory (apps/tracker/macos/entitlements.plist).
# Usage: apps/tracker/macos/sign.sh <binary>
set -eu
here=$(cd "$(dirname "$0")" && pwd)
codesign --force --sign - --entitlements "$here/entitlements.plist" "$1" 2>/dev/null
