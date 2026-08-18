#!/bin/bash
set -e

SERVER="root@188.34.196.228"
DEST_DIR="/opt/findr-website"
DMG_SRC="../src-tauri/target/release/bundle/dmg/findr_0.1.2_aarch64.dmg"

echo "Copying DMG to website directory..."
cp "$DMG_SRC" ./findr_0.1.2_aarch64.dmg

echo "Syncing files to server..."
rsync -avz \
  --exclude '.DS_Store' \
  ./ $SERVER:$DEST_DIR

echo "Building and starting container..."
ssh $SERVER "cd $DEST_DIR && docker compose up --build -d"

echo "Cleaning up local DMG copy..."
rm -f ./findr_0.1.2_aarch64.dmg

echo "Done. https://findr.beautiful-apps.com/"
