#!/usr/bin/env bash
set -euo pipefail

DMG_PATH="${1:-target/release/bundle/dmg/findr_0.1.0_aarch64.dmg}"
TEMP_DMG="$(mktemp -t fixdmg).dmg"

echo "Converting to writable DMG..."
hdiutil convert "$DMG_PATH" -format UDRW -o "$TEMP_DMG"

echo "Mounting..."
DEV_NAME=$(hdiutil attach -readwrite -noverify -noautoopen -nobrowse "$TEMP_DMG" \
  | grep -E '^/dev/' | head -1 | awk '{print $1}')
MOUNT_DIR=$(hdiutil info | grep -A1 "$DEV_NAME" | grep '/Volumes/' | awk '{$1=$2=""; print $0}' | xargs)

echo "Mounted at: $MOUNT_DIR"

if [[ -f "$MOUNT_DIR/.VolumeIcon.icns" ]]; then
  chflags hidden "$MOUNT_DIR/.VolumeIcon.icns"
  echo "Hidden .VolumeIcon.icns via chflags"
fi

for f in .background .fseventsd .Trashes; do
  [[ -e "$MOUNT_DIR/$f" ]] && chflags hidden "$MOUNT_DIR/$f"
done

echo "Unmounting..."
attempts=0
until hdiutil detach "$DEV_NAME" 2>/dev/null; do
  ((attempts++))
  [[ $attempts -ge 5 ]] && { echo "Failed to detach"; exit 1; }
  sleep $((attempts * 2))
done

echo "Converting back to compressed DMG..."
rm -f "$DMG_PATH"
hdiutil convert "$TEMP_DMG" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH"
rm -f "$TEMP_DMG"

echo "Done. Fixed: $DMG_PATH"
