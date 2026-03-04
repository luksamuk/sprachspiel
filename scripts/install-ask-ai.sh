#!/usr/bin/env bash
# =============================================================================
# Ask AI Remote Installer
# =============================================================================
# This script downloads and installs ask-ai from GitHub releases.
#
# Usage:
#   curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash
#   curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --version 0.25.0
#   curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --tools all
#   curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --prefix /usr
#
# Platform Detection:
#   - Linux x86_64: Downloads ask-ai-VERSION-linux-x86_64.tar.gz
#   - Termux/Android: Downloads ask-ai-VERSION-termux-aarch64.tar.gz
#   - macOS (ARM): Downloads ask-ai-VERSION-darwin-arm64.tar.gz (future)
#   - macOS (Intel): Downloads ask-ai-VERSION-darwin-x86_64.tar.gz (future)
#
# Requirements:
#   - curl
#   - tar
#   - bash
# =============================================================================

set -e

# Repository information
REPO="anomalyco/ask-ai"
RELEASES_URL="https://github.com/$REPO/releases/download"
LATEST_API_URL="https://api.github.com/repos/$REPO/releases/latest"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# =============================================================================
# Helper Functions
# =============================================================================

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

show_banner() {
    echo -e "${CYAN}"
    echo "    _    ____  ____   _____ ___  _   _  ____ "
    echo "   / \\  |  _ \\|  _ \\ |_   _/ _ \\| \\ | |/ ___|"
    echo "  / _ \\ | |_) | |_) |  | || | | |  \\| | |  _ "
    echo " / ___ \\|  __/|  __/   | || |_| | |\\  | |_| |"
    echo "/_/   \\_\\_|   |_|      |_| \\___/|_| \\_|\\____|"
    echo -e "${NC}"
    echo ""
}

get_latest_version() {
    local version
    if command -v curl &>/dev/null; then
        version=$(curl -sL "$LATEST_API_URL" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    elif command -v wget &>/dev/null; then
        version=$(wget -qO- "$LATEST_API_URL" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    else
        print_error "Neither curl nor wget is available"
        exit 1
    fi
    
    if [[ -z "$version" ]]; then
        print_error "Could not determine latest version"
        exit 1
    fi
    
    echo "$version"
}

detect_platform() {
    local os
    local arch
    
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)
    
    # Detect Termux
    if [[ -n "$TERMUX_VERSION" ]] || [[ "$PREFIX" == *com.termux* ]]; then
        echo "termux-$(dpkg --print-architecture 2>/dev/null || echo 'aarch64')"
        return
    fi
    
    # Detect platform
    case "$os:$arch" in
        linux:x86_64|linux:amd64)
            echo "linux-x86_64"
            ;;
        linux:aarch64|linux:arm64)
            echo "linux-arm64"
            ;;
        darwin:x86_64|darwin:amd64)
            echo "darwin-x86_64"
            ;;
        darwin:arm64|darwin:aarch64)
            echo "darwin-arm64"
            ;;
        *)
            print_error "Unsupported platform: $os $arch"
            echo "Supported platforms:"
            echo "  - Linux x86_64"
            echo "  - Linux ARM64"
            echo "  - macOS x86_64"
            echo "  - macOS ARM64"
            echo "  - Termux (Android)"
            exit 1
            ;;
    esac
}

show_help() {
    cat << EOF
Ask AI Remote Installer

Usage:
  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash
  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- [OPTIONS]

Options:
  --version VERSION   Install specific version (default: latest)
  --tools all         Install with all tools enabled (larger binary)
  --prefix DIR        Install to DIR/bin and DIR/share/man (default: ~/.local)
  --bin DIR           Install binary to DIR
  --man DIR           Install manpage to DIR
  --list-versions     List available versions
  --help              Show this help

Examples:
  # Install latest version
  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash

  # Install specific version
  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --version 0.25.0

  # Install with all tools
  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --tools all

  # Install system-wide (requires sudo for /usr)
  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --prefix /usr

For more information: https://github.com/anomalyco/ask-ai
EOF
}

list_versions() {
    print_info "Available versions:"
    echo ""
    
    local versions
    if command -v curl &>/dev/null; then
        versions=$(curl -sL "https://api.github.com/repos/$REPO/releases" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    elif command -v wget &>/dev/null; then
        versions=$(wget -qO- "https://api.github.com/repos/$REPO/releases" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
    fi
    
    if [[ -n "$versions" ]]; then
        echo "$versions" | head -10
        echo ""
        print_info "See all releases at: https://github.com/$REPO/releases"
    else
        print_error "Could not fetch versions from GitHub"
    fi
}

cleanup() {
    if [[ -n "$TMPDIR" ]] && [[ -d "$TMPDIR" ]]; then
        rm -rf "$TMPDIR"
    fi
}

# =============================================================================
# Main Script
# =============================================================================

trap cleanup EXIT

# Parse arguments
INSTALL_VERSION=""
TOOLS_SUFFIX=""
BIN_DIR=""
MAN_DIR=""
LIST_VERSIONS=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            shift
            INSTALL_VERSION="$1"
            shift
            ;;
        --tools)
            shift
            if [[ "$1" == "all" ]]; then
                TOOLS_SUFFIX="-all-tools"
            fi
            shift
            ;;
        --prefix)
            shift
            PREFIX="$1"
            BIN_DIR="$PREFIX/bin"
            MAN_DIR="$PREFIX/share/man/man1"
            shift
            ;;
        --bin)
            shift
            BIN_DIR="$1"
            shift
            ;;
        --man)
            shift
            MAN_DIR="$1"
            shift
            ;;
        --list-versions)
            LIST_VERSIONS=true
            shift
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Run with --help for usage"
            exit 1
            ;;
    esac
done

# Show banner
show_banner

# List versions if requested
if $LIST_VERSIONS; then
    list_versions
    exit 0
fi

# Get latest version if not specified
if [[ -z "$INSTALL_VERSION" ]]; then
    print_info "Detecting latest version..."
    INSTALL_VERSION=$(get_latest_version)
fi

print_info "Installing ask-ai version $INSTALL_VERSION"

# Detect platform
PLATFORM=$(detect_platform)
print_info "Platform: $PLATFORM"

# Build tarball name
TARBALL="ask-ai-${INSTALL_VERSION}-${PLATFORM}${TOOLS_SUFFIX}.tar.gz"
DOWNLOAD_URL="${RELEASES_URL}/v${INSTALL_VERSION}/${TARBALL}"

print_info "Downloading: $DOWNLOAD_URL"

# Create temp directory
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Download tarball
if command -v curl &>/dev/null; then
    if ! curl -sL -f "$DOWNLOAD_URL" -o "$TARBALL"; then
        print_error "Failed to download: $DOWNLOAD_URL"
        print_error "Version $INSTALL_VERSION may not have a release for platform $PLATFORM"
        print_info "Check available releases at: https://github.com/$REPO/releases"
        exit 1
    fi
elif command -v wget &>/dev/null; then
    if ! wget -q "$DOWNLOAD_URL" -O "$TARBALL"; then
        print_error "Failed to download: $DOWNLOAD_URL"
        print_error "Version $INSTALL_VERSION may not have a release for platform $PLATFORM"
        print_info "Check available releases at: https://github.com/$REPO/releases"
        exit 1
    fi
else
    print_error "Neither curl nor wget is available"
    exit 1
fi

print_success "Download complete"

# Extract tarball
print_info "Extracting..."
if ! tar -xzf "$TARBALL"; then
    print_error "Failed to extract tarball"
    exit 1
fi

# Find the extracted directory (may be nested)
EXTRACT_DIR=$(find . -maxdepth 2 -name "install.sh" -printf "%h\n" | head -1)
if [[ -n "$EXTRACT_DIR" ]] && [[ "$EXTRACT_DIR" != "." ]]; then
    cd "$EXTRACT_DIR"
fi

# Build install arguments
INSTALL_ARGS=()
if [[ -n "$BIN_DIR" ]]; then
    INSTALL_ARGS+=("--bin" "$BIN_DIR")
fi
if [[ -n "$MAN_DIR" ]]; then
    INSTALL_ARGS+=("--man" "$MAN_DIR")
fi

# Run installer
print_info "Running installer..."
chmod +x install.sh

if [[ ${#INSTALL_ARGS[@]} -gt 0 ]]; then
    ./install.sh "${INSTALL_ARGS[@]}"
else
    ./install.sh
fi

# Cleanup is automatic via trap
echo ""
print_success "ask-ai $INSTALL_VERSION installed successfully!"