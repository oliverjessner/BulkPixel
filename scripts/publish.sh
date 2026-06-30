#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

RELEASE_REPO="oliverjessner/BulkPixel"
CHANGELOG_URL="https://raw.githubusercontent.com/$RELEASE_REPO/main/docs/changelog.md"
BUNDLE_DIR="src-tauri/target/release/bundle"
DMG_DIR="$BUNDLE_DIR/dmg"
HOMEBREW_TAP_DIR=${HOMEBREW_TAP_DIR:-}
TMP_CHANGELOG=$(mktemp)
TMP_RELEASE_NOTES=$(mktemp)
LOCAL_CHANGELOG="docs/changelog.md"
DMG_SOURCE=""

if [ -z "$HOMEBREW_TAP_DIR" ]; then
    HOMEBREW_TAP_DIR="$REPO_ROOT/../homebrew-tap"
fi

cleanup() {
    rm -f "$TMP_CHANGELOG" "$TMP_RELEASE_NOTES"
    if [ -n "$DMG_SOURCE" ]; then
        rm -rf "$DMG_SOURCE"
    fi
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
require_command git
require_command codesign
require_command hdiutil
require_command curl
require_command shasum

update_homebrew_cask() {
    if [ ! -d "$HOMEBREW_TAP_DIR/.git" ]; then
        echo "Homebrew tap not found at $HOMEBREW_TAP_DIR"
        echo "Set HOMEBREW_TAP_DIR to the tap checkout before publishing."
        exit 1
    fi

    if [ -n "$(git -C "$HOMEBREW_TAP_DIR" status --porcelain)" ]; then
        echo "Homebrew tap has uncommitted changes. Commit or stash them before publishing."
        exit 1
    fi

    echo "Updating Homebrew cask..."
    git -C "$HOMEBREW_TAP_DIR" pull --ff-only

    DMG_SHA256=$(shasum -a 256 "$OUT" | awk '{ print $1 }')
    CASK_DIR="$HOMEBREW_TAP_DIR/Casks"
    CASK_PATH="$CASK_DIR/bulkpixel.rb"
    mkdir -p "$CASK_DIR"

    cat > "$CASK_PATH" <<EOF
cask "bulkpixel" do
  version "$VERSION"
  sha256 "$DMG_SHA256"

  url "https://github.com/oliverjessner/BulkPixel/releases/download/v#{version}/BulkPixel_#{version}_aarch64_adhoc.dmg",
      verified: "github.com/oliverjessner/BulkPixel/"
  name "$PRODUCT_NAME"
  desc "Local-first batch image converter"
  homepage "https://github.com/oliverjessner/BulkPixel"

  depends_on arch: :arm64
  depends_on macos: :big_sur

  app "$PRODUCT_NAME.app"
  binary "#{appdir}/$PRODUCT_NAME.app/Contents/MacOS/$APP_EXECUTABLE_NAME", target: "bulkpixel"

  zap trash: [
    "~/Library/Application Support/com.oli.bulkpixel",
    "~/Library/Preferences/com.oli.bulkpixel.plist",
  ]
end
EOF

    if command -v brew >/dev/null 2>&1; then
        brew style "$CASK_PATH"
    fi

    git -C "$HOMEBREW_TAP_DIR" add "Casks/bulkpixel.rb"

    if git -C "$HOMEBREW_TAP_DIR" diff --cached --quiet; then
        echo "Homebrew cask already up to date."
        return
    fi

    git -C "$HOMEBREW_TAP_DIR" commit -m "Update BulkPixel cask to $TAG"
    git -C "$HOMEBREW_TAP_DIR" push
}

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
DMG_SOURCE=$(mktemp -d)
cp -R "$APP" "$DMG_SOURCE/"
APP_EXECUTABLE_NAME=$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist" 2>/dev/null || echo "$PRODUCT_NAME")
APP_EXECUTABLE="$APP/Contents/MacOS/$APP_EXECUTABLE_NAME"
if [ ! -x "$APP_EXECUTABLE" ]; then
    APP_EXECUTABLE_NAME="bulkpixel"
    APP_EXECUTABLE="$APP/Contents/MacOS/$APP_EXECUTABLE_NAME"
fi

if [ ! -x "$APP_EXECUTABLE" ]; then
    echo "Unable to find app executable for CLI symlink."
    exit 1
fi

ln -s "${PRODUCT_NAME}.app/Contents/MacOS/$APP_EXECUTABLE_NAME" "$DMG_SOURCE/bulkpixel"
hdiutil create -volname "$PRODUCT_NAME" -srcfolder "$DMG_SOURCE" -ov -format UDZO "$OUT"
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

update_homebrew_cask

echo "Build completed successfully."
echo "Opening the ${PRODUCT_NAME} release page..."
open -a "Google Chrome" "https://github.com/$RELEASE_REPO/releases/tag/$TAG"
