#!/usr/bin/env bash
# GAL — Golem Agents Legion installer
# Usage: curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
#
# Integrity model:
#   - SHA-256 check against checksums.txt is MANDATORY (aborts on mismatch)
#   - cosign verify-blob is BEST-EFFORT when cosign or the signature files are
#     unavailable (warns and continues), but a COMPLETED verify-blob run that
#     returns nonzero is FATAL — an observed signature failure aborts install.
#
# Non-interactive: all output goes to stdout/stderr; no interactive prompts.
# stdin may be a pipe (curl|bash), so no read calls.

set -euo pipefail

# Lowercase slug works for GitHub download/API URLs (case-insensitive redirects).
REPO="monkey1wizard/golem-agents-legion"
# Canonical-case slug — the cosign certificate SAN identity is case-SENSITIVE and
# carries the repository's canonical case; a lowercase regexp silently never matches.
REPO_CANONICAL="monkey1wizard/Golem-Agents-Legion"
INSTALL_DIR="${HOME}/.local/bin"
# FHS source payload home: resolve_source_from_exe_dir checks <prefix>/share/gal,
# where <prefix> is the parent of the bin dir (~/.local/bin → ~/.local/share/gal).
SHARE_DIR="${HOME}/.local/share/gal"
TMP_DIR=""

# --- helpers -----------------------------------------------------------------

die() { echo "gal-install: error: $*" >&2; exit 1; }
warn() { echo "gal-install: warning: $*" >&2; }
info() { echo "gal-install: $*"; }

cleanup() {
    [ -n "${TMP_DIR}" ] && rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

# --- detect OS / arch --------------------------------------------------------

detect_platform() {
    local os arch

    case "$(uname -s)" in
        Darwin) os="darwin" ;;
        Linux)  os="linux"  ;;
        *)
            die "Unsupported OS: $(uname -s). Supported: Linux, macOS.
Please download the binary manually from:
  https://github.com/${REPO}/releases"
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)  arch="x64"   ;;
        aarch64|arm64) arch="arm64" ;;
        *)
            die "Unsupported architecture: $(uname -m). Supported: x86_64, aarch64.
Please download the binary manually from:
  https://github.com/${REPO}/releases"
            ;;
    esac

    echo "${os}" "${arch}"
}

# --- detect latest release tag -----------------------------------------------

fetch_latest_version() {
    local tag
    tag=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
    [ -n "${tag}" ] || die "Failed to fetch latest release version from GitHub API."
    echo "${tag}"
}

# --- SHA-256 helper ----------------------------------------------------------

sha256_file() {
    local file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "${file}" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "${file}" | awk '{print $1}'
    else
        die "No SHA-256 tool found (sha256sum or shasum required)."
    fi
}

# --- main --------------------------------------------------------------------

main() {
    read -r OS ARCH <<< "$(detect_platform)"

    info "Detected platform: ${OS}-${ARCH}"

    VERSION="${GAL_VERSION:-$(fetch_latest_version)}"
    info "Installing GAL ${VERSION}"

    ARCHIVE_NAME="gal-${VERSION}-${OS}-${ARCH}.tar.gz"
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

    TMP_DIR=$(mktemp -d)

    # Download archive
    info "Downloading ${ARCHIVE_NAME}..."
    curl -fsSL --retry 3 --retry-delay 2 \
        "${BASE_URL}/${ARCHIVE_NAME}" \
        -o "${TMP_DIR}/${ARCHIVE_NAME}" \
        || die "Download failed: ${BASE_URL}/${ARCHIVE_NAME}"

    # Download checksums.txt
    info "Downloading checksums.txt..."
    curl -fsSL --retry 3 --retry-delay 2 \
        "${BASE_URL}/checksums.txt" \
        -o "${TMP_DIR}/checksums.txt" \
        || die "Download failed: ${BASE_URL}/checksums.txt"

    # --- Mandatory SHA-256 verification --------------------------------------
    info "Verifying SHA-256 integrity..."
    EXPECTED=$(grep "${ARCHIVE_NAME}" "${TMP_DIR}/checksums.txt" | awk '{print $1}')
    [ -n "${EXPECTED}" ] \
        || die "Checksum entry not found for '${ARCHIVE_NAME}' in checksums.txt."

    ACTUAL=$(sha256_file "${TMP_DIR}/${ARCHIVE_NAME}")

    if [ "${EXPECTED}" != "${ACTUAL}" ]; then
        die "SHA-256 mismatch — download may be corrupted or tampered.
  Expected: ${EXPECTED}
  Actual:   ${ACTUAL}
Aborting installation."
    fi
    info "SHA-256 OK (${ACTUAL:0:16}...)"

    # --- cosign best-effort verification -------------------------------------
    if command -v cosign >/dev/null 2>&1; then
        info "cosign found — downloading signature files..."
        SIG_URL="${BASE_URL}/checksums.txt.sig"
        CERT_URL="${BASE_URL}/checksums.txt.pem"

        if curl -fsSL --retry 2 "${SIG_URL}" -o "${TMP_DIR}/checksums.txt.sig" 2>/dev/null \
        && curl -fsSL --retry 2 "${CERT_URL}" -o "${TMP_DIR}/checksums.txt.pem" 2>/dev/null; then
            if cosign verify-blob \
                --signature "${TMP_DIR}/checksums.txt.sig" \
                --certificate "${TMP_DIR}/checksums.txt.pem" \
                --certificate-identity-regexp "https://github.com/${REPO_CANONICAL}/\.github/workflows/release\.yml@.*" \
                --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
                "${TMP_DIR}/checksums.txt"; then
                info "cosign signature verified."
            else
                die "cosign signature verification FAILED — checksums.txt does not match a trusted signature. Aborting installation."
            fi
        else
            warn "Could not download cosign signature files — skipping (SHA-256 passed)."
        fi
    else
        warn "cosign not found in PATH — skipping cosign verification (SHA-256 passed)."
        warn "Install cosign from https://docs.sigstore.dev/cosign/system_config/installation/ for full verification."
    fi

    # --- Extract and install -------------------------------------------------
    # Extract into a clean subdir so the source payload is separable from the
    # downloaded archive + checksum/signature files sitting in TMP_DIR.
    info "Extracting archive..."
    PKG_DIR="${TMP_DIR}/pkg"
    mkdir -p "${PKG_DIR}"
    tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "${PKG_DIR}"

    BINARY="${PKG_DIR}/gal"
    [ -f "${BINARY}" ] || die "Binary 'gal' not found in archive."

    mkdir -p "${INSTALL_DIR}"
    install -m 755 "${BINARY}" "${INSTALL_DIR}/gal"

    # Install the GAL source payload (skills/agents/commands/… + mcp.json) into
    # the FHS share dir so `gal refresh` can resolve a source root without a repo
    # checkout. Binary-only install leaves `gal refresh` failing with
    # "Source root not found". Replace any prior payload for a clean upgrade.
    info "Installing source payload to ${SHARE_DIR}..."
    rm -rf "${SHARE_DIR}"
    mkdir -p "${SHARE_DIR}"
    cp -R "${PKG_DIR}/." "${SHARE_DIR}/"
    # The binary already lives in bin/; a redundant copy under share/gal would be
    # confusing (share is the source root, not a bin dir).
    rm -f "${SHARE_DIR}/gal"

    # --- Verify install ------------------------------------------------------
    GAL_BIN="${INSTALL_DIR}/gal"
    GAL_VERSION_OUT=$("${GAL_BIN}" --version 2>&1) || true

    echo ""
    echo "GAL installed successfully."
    echo "  Location: ${GAL_BIN}"
    echo "  Version:  ${GAL_VERSION_OUT}"
    echo ""

    # PATH reminder if needed
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            echo "Add ${INSTALL_DIR} to your PATH by adding this line to your shell profile:"
            echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
            echo ""
            ;;
    esac

    echo "Next steps:"
    echo "  gal refresh     -- project GAL onto your coding agents (run once after install)"
    echo "  gal init        -- initialize GAL in a new repo"
    echo "  gal --help      -- show all commands"
    echo ""
    echo "Documentation: https://github.com/${REPO}"
}

main "$@"
