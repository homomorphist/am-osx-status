#!/usr/bin/env bash
set -euo pipefail

METADATA=$(cargo metadata --no-deps --format-version=1 | jq -r '.packages[] | select(.name=="am-osx-status")')
if [ -z "$1" ]; then
  echo "Usage: $0 <output-dir>"
  exit 1
fi

cat > "$1/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>network.goop.am-osx-status</string>
  <key>CFBundleName</key>
  <string>am-osx-status</string>
  <key>CFBundleVersion</key>
  <string>$(echo "$METADATA" | jq -r .version)</string>
  <key>CFBundleExecutable</key>
  <string>am-osx-status</string>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright 2025 Katini. Licensed under $(echo "$METADATA" | jq -r .license).</string>
</dict>
</plist>
EOF

echo "Wrote Info.plist into $1"
