#!/bin/sh

set -eu

RELEASE_REPO="oliverjessner/BulkPixel"
CHANGELOG_URL="https://raw.githubusercontent.com/$RELEASE_REPO/main/changelog.md"
BUNDLE_DIR="src-tauri/target/release/bundle"
DMG_DIR="$BUNDLE_DIR/dmg"
TMP_CHANGELOG=$(mktemp)
TMP_RELEASE_NOTES=$(mktemp)
LOCAL_CHANGELOG="changelog.md"

cleanup() {
    rm -f "$TMP_CHANGELOG" "$TMP_RELEASE_NOTES"
}

trap cleanup EXIT

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1"
        exit 1
    fi
}

require_command npm
require_command node
require_command gh
require_command codesign
require_command hdiutil
require_command curl

PRODUCT_NAME=$(node -p 'require("./src-tauri/tauri.conf.json").productName')
VERSION=$(node -p 'require("./package.json").version || "0.0.0"')
TAG="v$VERSION"
APP="$BUNDLE_DIR/macos/${PRODUCT_NAME}.app"
OUT="$DMG_DIR/${PRODUCT_NAME}_${VERSION}_aarch64_adhoc.dmg"

echo "Cleaning previous builds..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

echo "Building ${PRODUCT_NAME}..."
npm run tauri -- build --bundles app --no-sign

if [ ! -d "$APP" ]; then
    echo "App not found at $APP"
    exit 1
fi

if [ -f "scripts/bundle-macos-dylibs.mjs" ]; then
    echo "Bundling additional macOS dylibs..."
    node scripts/bundle-macos-dylibs.mjs "$APP"
fi

echo "Codesigning app..."
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

echo "Creating DMG..."
mkdir -p "$DMG_DIR"
rm -f "$OUT"
hdiutil create -volname "$PRODUCT_NAME" -srcfolder "$APP" -ov -format UDZO "$OUT"
echo "Created $OUT"

echo "Preparing release notes..."
if curl -L --fail --silent --show-error "$CHANGELOG_URL" -o "$TMP_CHANGELOG"; then
    :
elif [ -f "$LOCAL_CHANGELOG" ]; then
    cp "$LOCAL_CHANGELOG" "$TMP_CHANGELOG"
else
    echo "Unable to fetch remote changelog and no local $LOCAL_CHANGELOG found."
    exit 1
fi

awk -v version="$VERSION" '
    $0 == "# " version { capture=1 }
    capture && $0 ~ /^# / && $0 != "# " version { exit }
    capture { print }
' "$TMP_CHANGELOG" > "$TMP_RELEASE_NOTES"

if [ ! -s "$TMP_RELEASE_NOTES" ]; then
    cp "$TMP_CHANGELOG" "$TMP_RELEASE_NOTES"
fi

echo "Creating GitHub release $TAG on $RELEASE_REPO..."
gh release create "$TAG" "$OUT" \
    --repo "$RELEASE_REPO" \
    --title "${PRODUCT_NAME} $TAG" \
    --notes-file "$TMP_RELEASE_NOTES"

echo "Build completed successfully."
echo "Opening the ${PRODUCT_NAME} release page..."
open -a "Google Chrome" "https://github.com/$RELEASE_REPO/releases/tag/$TAG"
