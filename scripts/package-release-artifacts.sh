#!/usr/bin/env bash
#
# Packages release artifacts for GAL, ensuring all fallbacks wrap the same canonical binary.
# Part of the GAL release flow (T-003).

set -euo pipefail

usage() {
    echo "Usage: $0 --source-binary <path> --version <version> --target-platform <windows|darwin|linux> --target-arch <x64|arm64> [--output-dir <path>]"
    exit 1
}

SOURCE_BINARY=""
VERSION=""
TARGET_PLATFORM=""
TARGET_ARCH=""
OUTPUT_DIR="$HOME/.gal/dist/release"

while [[ $# -gt 0 ]]; do
    case $1 in
        --source-binary)
            SOURCE_BINARY="$2"
            shift 2
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --target-platform)
            TARGET_PLATFORM="$2"
            shift 2
            ;;
        --target-arch)
            TARGET_ARCH="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1"
            usage
            ;;
    esac
done

if [[ -z "$SOURCE_BINARY" || -z "$VERSION" || -z "$TARGET_PLATFORM" || -z "$TARGET_ARCH" ]]; then
    usage
fi

if [[ ! -f "$SOURCE_BINARY" ]]; then
    echo "Error: Source binary not found at $SOURCE_BINARY"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

LICENSE_FILE="LICENSE"
if [[ ! -f "$LICENSE_FILE" ]]; then
    echo "Warning: LICENSE file not found in root. Artifacts must include a LICENSE."
fi

NORMALIZED_VERSION="${VERSION}"

if [[ "$TARGET_PLATFORM" == "windows" ]]; then
    BINARY_NAME="gal-${NORMALIZED_VERSION}-${TARGET_PLATFORM}-${TARGET_ARCH}.exe"
    ARCHIVE_EXT=".zip"
else
    BINARY_NAME="gal-${NORMALIZED_VERSION}-${TARGET_PLATFORM}-${TARGET_ARCH}"
    ARCHIVE_EXT=".tar.gz"
fi

ARCHIVE_NAME="gal-${NORMALIZED_VERSION}-${TARGET_PLATFORM}-${TARGET_ARCH}${ARCHIVE_EXT}"
ARCHIVE_PATH="${OUTPUT_DIR}/${ARCHIVE_NAME}"
STAGING_DIR="${OUTPUT_DIR}/staging-${TARGET_PLATFORM}-${TARGET_ARCH}"

echo "Packaging $ARCHIVE_NAME for $VERSION..."

rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"

cp "$SOURCE_BINARY" "${STAGING_DIR}/${BINARY_NAME}"
if [[ -f "$LICENSE_FILE" ]]; then
    cp "$LICENSE_FILE" "${STAGING_DIR}/${LICENSE_FILE}"
fi

if [[ "$ARCHIVE_EXT" == ".zip" ]]; then
    (cd "$STAGING_DIR" && zip -q -r "../${ARCHIVE_NAME}" .)
else
    (cd "$STAGING_DIR" && tar -czf "../${ARCHIVE_NAME}" .)
fi

echo "Created archive at $ARCHIVE_PATH"

rm -rf "$STAGING_DIR"

CHECKSUM_FILE="${OUTPUT_DIR}/checksums.txt"

if [[ -f "$CHECKSUM_FILE" ]]; then
    grep -v -E "  ${ARCHIVE_NAME}$|  ${BINARY_NAME}$" "$CHECKSUM_FILE" > "${CHECKSUM_FILE}.tmp" || true
    mv "${CHECKSUM_FILE}.tmp" "$CHECKSUM_FILE"
fi

if command -v shasum >/dev/null 2>&1; then
    HASH=$(shasum -a 256 "$ARCHIVE_PATH" | awk '{print $1}')
else
    HASH=$(sha256sum "$ARCHIVE_PATH" | awk '{print $1}')
fi

echo "$HASH  $ARCHIVE_NAME" >> "$CHECKSUM_FILE"

BARE_BINARY_PATH="${OUTPUT_DIR}/${BINARY_NAME}"
cp "$SOURCE_BINARY" "$BARE_BINARY_PATH"

if command -v shasum >/dev/null 2>&1; then
    BINARY_HASH=$(shasum -a 256 "$BARE_BINARY_PATH" | awk '{print $1}')
else
    BINARY_HASH=$(sha256sum "$BARE_BINARY_PATH" | awk '{print $1}')
fi

echo "$BINARY_HASH  $BINARY_NAME" >> "$CHECKSUM_FILE"

echo "Updated $CHECKSUM_FILE"
echo "Packaging complete for ${TARGET_PLATFORM}-${TARGET_ARCH}."
