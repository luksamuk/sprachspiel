#!/usr/bin/env bash
# =============================================================================
# Ask AI Installer
# =============================================================================
# This script installs ask-ai on Linux or Termux (Android).
#
# Usage:
#   ./install.sh                  # Install with defaults (~/.local/bin)
#   ./install.sh --prefix /usr    # Install to /usr/bin (requires sudo)
#   ./install.sh --bin ~/bin      # Custom binary location
#   ./install.sh --uninstall      # Remove ask-ai
#   ./install.sh --help           # Show help
#
# Platform Detection:
#   - Linux x86_64: Installs to ~/.local/bin by default
#   - Termux: Installs to ~/bin by default
#
# Files Installed:
#   - ask-ai binary
#   - ask-ai.1 manpage
#
# Requirements:
#   - bash
#   - No external dependencies
# =============================================================================

set -e

# Version (will be replaced during tarball creation)
VERSION=""

# Binary and manpage names
BINARY="ask-ai"
MANPAGE="ask-ai.1"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
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

detect_platform() {
    if [[ -n "$TERMUX_VERSION" ]] || [[ "$PREFIX" == *com.termux* ]]; then
        echo "termux"
    elif [[ "$(uname -s)" == "Linux" ]]; then
        echo "linux"
    elif [[ "$(uname -s)" == "Darwin" ]]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

get_default_bin_dir() {
    local platform="$1"
    case "$platform" in
        termux)
            echo "$HOME/bin"
            ;;
        macos)
            echo "$HOME/.local/bin"
            ;;
        *)
            echo "$HOME/.local/bin"
            ;;
    esac
}

get_default_man_dir() {
    local platform="$1"
    case "$platform" in
        termux)
            echo "$HOME/.local/share/man/man1"
            ;;
        macos)
            echo "$HOME/.local/share/man/man1"
            ;;
        *)
            echo "$HOME/.local/share/man/man1"
            ;;
    esac
}

show_help() {
    cat << EOF
Ask AI Installer v${VERSION:-dev}

Usage:
  ./install.sh                  Install with defaults
  ./install.sh --prefix DIR     Install to DIR/bin and DIR/share/man
  ./install.sh --bin DIR        Install binary to DIR
  ./install.sh --man DIR        Install manpage to DIR
  ./install.sh --uninstall      Remove ask-ai
  ./install.sh --help           Show this help

Default Paths:
  Linux:  Binary: ~/.local/bin
          Manpage: ~/.local/share/man/man1

  Termux: Binary: ~/bin
          Manpage: ~/.local/share/man/man1

Examples:
  ./install.sh                          # Install to ~/.local/bin
  ./install.sh --prefix /usr            # Install to /usr/bin (requires sudo)
  ./install.sh --bin ~/bin --man ~/man  # Custom locations
  ./install.sh --uninstall              # Remove ask-ai

After Installation:
  - Add ~/.local/bin (or your chosen dir) to PATH
  - Run 'man -M ~/.local/share/man ask-ai' or set MANPATH

For more information: https://github.com/luksamuk/ask-ai-rs
EOF
}

check_path() {
    local dir="$1"
    if echo "$PATH" | grep -q "$dir"; then
        return 0
    else
        return 1
    fi
}

check_manpath() {
    local man_root="$1"
    local man_root_parent
    man_root_parent=$(dirname "$man_root" 2>/dev/null || echo "")
    
    if [[ -n "$MANPATH" ]] && echo "$MANPATH" | grep -q "$man_root_parent"; then
        return 0
    elif command -v manpath &>/dev/null && manpath 2>/dev/null | grep -q "$man_root_parent"; then
        return 0
    fi
    return 1
}

get_shell_config() {
    local shell_name
    shell_name=$(basename "${SHELL:-bash}")
    
    case "$shell_name" in
        zsh)
            echo "$HOME/.zshrc"
            ;;
        fish)
            echo "$HOME/.config/fish/config.fish"
            ;;
        bash)
            if [[ -f "$HOME/.bashrc" ]]; then
                echo "$HOME/.bashrc"
            else
                echo "$HOME/.bash_profile"
            fi
            ;;
        *)
            echo "$HOME/.profile"
            ;;
    esac
}

# =============================================================================
# Main Installation Logic
# =============================================================================

PLATFORM=$(detect_platform)

if [[ "$PLATFORM" == "unknown" ]]; then
    print_error "Unsupported platform: $(uname -s) $(uname -m)"
    echo "Supported platforms: Linux x86_64, macOS, Termux (Android)"
    exit 1
fi

# Default paths
DEFAULT_BIN=$(get_default_bin_dir "$PLATFORM")
DEFAULT_MAN=$(get_default_man_dir "$PLATFORM")

# Parse arguments
BIN_DIR=""
MAN_DIR=""
UNINSTALL=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            shift
            if [[ -z "$1" ]]; then
                print_error "Missing argument for --prefix"
                exit 1
            fi
            BIN_DIR="$1/bin"
            MAN_DIR="$1/share/man/man1"
            shift
            ;;
        --bin)
            shift
            if [[ -z "$1" ]]; then
                print_error "Missing argument for --bin"
                exit 1
            fi
            BIN_DIR="$1"
            shift
            ;;
        --man)
            shift
            if [[ -z "$1" ]]; then
                print_error "Missing argument for --man"
                exit 1
            fi
            MAN_DIR="$1"
            shift
            ;;
        --uninstall|-u)
            UNINSTALL=true
            shift
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        --version|-v)
            if [[ -n "$VERSION" ]]; then
                echo "ask-ai installer version $VERSION"
            else
                echo "ask-ai installer (dev version)"
            fi
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Run './install.sh --help' for usage"
            exit 1
            ;;
    esac
done

# Apply defaults
BIN_DIR="${BIN_DIR:-$DEFAULT_BIN}"
MAN_DIR="${MAN_DIR:-$DEFAULT_MAN}"

# =============================================================================
# Uninstall
# =============================================================================

if $UNINSTALL; then
    print_info "Uninstalling ask-ai..."
    
    # Find and remove binary
    local found_binary=""
    for dir in "$BIN_DIR" "$HOME/.local/bin" "$HOME/bin" "/usr/local/bin" "/usr/bin"; do
        if [[ -f "$dir/$BINARY" ]]; then
            found_binary="$dir/$BINARY"
            rm -f "$dir/$BINARY"
            print_success "Removed: $dir/$BINARY"
        fi
    done
    
    # Find and remove manpage
    for dir in "$MAN_DIR" "$HOME/.local/share/man/man1" "/usr/local/share/man/man1" "/usr/share/man/man1"; do
        if [[ -f "$dir/$MANPAGE" ]]; then
            rm -f "$dir/$MANPAGE"
            print_success "Removed: $dir/$MANPAGE"
        fi
    done
    
    if [[ -z "$found_binary" ]]; then
        print_warning "ask-ai was not found. Nothing to uninstall."
    else
        print_success "Uninstall complete."
    fi
    
    exit 0
fi

# =============================================================================
# Install
# =============================================================================

print_info "Installing ask-ai on $PLATFORM..."

# Check for existing installation
if [[ -f "$BIN_DIR/$BINARY" ]]; then
    print_warning "ask-ai is already installed at $BIN_DIR/$BINARY"
    read -p "Replace it? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Installation cancelled."
        exit 0
    fi
fi

# Check if binary exists in the current directory (tarball extraction)
if [[ ! -f "$BINARY" ]]; then
    print_error "Binary '$BINARY' not found in current directory"
    echo "Make sure you extracted the tarball correctly:"
    echo "  tar -xzf ask-ai-VERSION-PLATFORM.tar.gz"
    echo "  cd ask-ai-VERSION-PLATFORM"
    echo "  ./install.sh"
    exit 1
fi

# Check if manpage exists
if [[ ! -f "$MANPAGE" ]]; then
    print_warning "Manpage '$MANPAGE' not found. Skipping manpage installation."
    SKIP_MANPAGE=true
else
    SKIP_MANPAGE=false
fi

# Create directories
print_info "Creating directories..."
mkdir -p "$BIN_DIR"
if [[ "$SKIP_MANPAGE" == "false" ]]; then
    mkdir -p "$MAN_DIR"
fi

# Install binary
print_info "Installing binary to $BIN_DIR..."
cp "$BINARY" "$BIN_DIR/$BINARY"
chmod +x "$BIN_DIR/$BINARY"
print_success "Binary installed: $BIN_DIR/$BINARY"

# Install manpage
if [[ "$SKIP_MANPAGE" == "false" ]]; then
    print_info "Installing manpage to $MAN_DIR..."
    cp "$MANPAGE" "$MAN_DIR/$MANPAGE"
    print_success "Manpage installed: $MAN_DIR/$MANPAGE"
fi

echo ""
print_success "Installation complete!"
echo ""

# =============================================================================
# Post-Installation Instructions
# =============================================================================

echo "Installation Details:"
echo "  Binary:   $BIN_DIR/$BINARY"
if [[ "$SKIP_MANPAGE" == "false" ]]; then
    echo "  Manpage:  $MAN_DIR/$MANPAGE"
fi
echo ""

# PATH instructions
if ! check_path "$BIN_DIR"; then
    print_warning "$BIN_DIR is not in your PATH"
    echo ""
    
    SHELL_CONFIG=$(get_shell_config)
    
    echo "Add this to your shell config ($SHELL_CONFIG):"
    echo ""
    echo "  export PATH=\"$BIN_DIR:\$PATH\""
    echo ""
    
    if [[ "$PLATFORM" == "termux" ]]; then
        print_info "For Termux, you can also add this to ~/.termux/boot/autoload.sh"
        echo ""
    fi
    
    print_info "After editing, run: source $SHELL_CONFIG"
    echo ""
fi

# MANPATH instructions
if [[ "$SKIP_MANPAGE" == "false" ]]; then
    MAN_ROOT_PARENT=$(dirname "$(dirname "$MAN_DIR")")
    if ! check_manpath "$MAN_DIR"; then
        print_info "To read manpages, use one of:"
        echo ""
        echo "  man -M $MAN_ROOT_PARENT ask-ai"
        echo ""
        echo "Or add to your shell config ($SHELL_CONFIG):"
        echo ""
        echo "  export MANPATH=\"$MAN_ROOT_PARENT:\$MANPATH\""
        echo ""
    fi
fi

# =============================================================================
# Verification
# =============================================================================

# Try to run the binary
if command -v "$BINARY" &>/dev/null; then
    # Binary is in PATH
    if "$BINARY" --version &>/dev/null; then
        print_success "Installation verified!"
    else
        print_warning "Binary installed but --version failed. Check if Ollama is running."
    fi
else
    # Binary not in PATH yet
    if "$BIN_DIR/$BINARY" --version &>/dev/null; then
        print_success "Binary verification: OK"
        print_info "Add $BIN_DIR to PATH to use '$BINARY' command"
    else
        print_warning "Binary installed but verification failed."
        echo "This might be normal if Ollama is not running."
    fi
fi

# =============================================================================
# Termux-Specific Notes
# =============================================================================

if [[ "$PLATFORM" == "termux" ]]; then
    echo ""
    echo "📱 Termux Notes:"
    echo "  - Ollama must run on a separate machine (desktop/server)"
    echo "  - Configure OLLAMA_HOST in ~/.config/ask-ai/config.toml"
    echo "  - Example configuration:"
    echo ""
    echo '    host = "192.168.1.100:11434"'
    echo ""
    print_info "See README-TERMUX.txt for detailed instructions (if included)"
fi

# =============================================================================
# Next Steps
# =============================================================================

echo ""
echo "Next Steps:"
echo "  1. Add $BIN_DIR to PATH (see above)"
if [[ "$SKIP_MANPAGE" == "false" ]]; then
    echo "  2. Run 'man ask-ai' or 'man -M $MAN_ROOT_PARENT ask-ai' for documentation"
    echo "  3. Run 'ask-ai --help' for usage"
else
    echo "  2. Run '$BIN_DIR/$BINARY --help' for usage"
fi
echo ""

exit 0