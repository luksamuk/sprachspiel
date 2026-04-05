# Makefile for ask-ai

# Default installation prefix
PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
MANDIR = $(PREFIX)/share/man/man1

# Binary name
BINARY = ask-ai
TARGET = ask-ai

# Man page
MANPAGE = man/ask-ai.1

# Build configuration
CARGO_FLAGS = --release
BUILD_DIR = target/release

# Feature flags
FEATURE_POKEMON = --features pokemon-tools
FEATURE_ALL = --features all-tools
FEATURE_ALL_NO_SANDBOX = --features all-tools-no-sandbox
FEATURE_TERMUX = --features all-tools-no-sandbox

# Cross-compilation targets
TERMUX_TARGET = aarch64-linux-android
TERMUX_BUILD_DIR = target/$(TERMUX_TARGET)/release

# Distribution
VERSION ?= $(shell grep '^version =' Cargo.toml | head -1 | cut -d'"' -f2)
DIST_DIR = dist
TARBALL_BASE = $(BINARY)-$(VERSION)

# Scripts
SCRIPTS_DIR = scripts
INSTALL_SCRIPT = $(SCRIPTS_DIR)/install.sh
UNINSTALL_SCRIPT = $(SCRIPTS_DIR)/uninstall.sh

.PHONY: all build install uninstall clean test check build-pokemon build-all-tools install-pokemon install-all-tools install-local-pokemon install-local-all-tools test-all termux termux-all-tools tarball tarball-linux tarball-linux-all-tools tarball-termux tarball-termux-all-tools all-tarballs help clean-dist

# =============================================================================
# Build Targets
# =============================================================================

# Default target
all: build

# Build the release binary (default features: weather, file-tools)
build:
	cargo build $(CARGO_FLAGS)

# Build with Pokémon tools (adds 8 Pokémon-related tools)
build-pokemon:
	cargo build $(CARGO_FLAGS) $(FEATURE_POKEMON)

# Build with all tools (includes sandbox on Linux)
build-all-tools:
	cargo build $(CARGO_FLAGS) $(FEATURE_ALL)

# =============================================================================
# Installation Targets
# =============================================================================

# Install binary and man page to PREFIX
install: build
	@echo "Installing $(TARGET) to $(BINDIR)..."
	@mkdir -p $(BINDIR)
	@cp $(BUILD_DIR)/$(BINARY) $(BINDIR)/$(TARGET)
	@chmod +x $(BINDIR)/$(TARGET)
	@echo "Installing man page to $(MANDIR)..."
	@mkdir -p $(MANDIR)
	@cp $(MANPAGE) $(MANDIR)/
	@echo "Installation complete!"
	@echo "Binary installed at: $(BINDIR)/$(TARGET)"
	@echo "Man page installed at: $(MANDIR)/ask-ai.1"
	@echo "Make sure $(BINDIR) is in your PATH"

# Install with Pokémon tools
install-pokemon: build-pokemon
	@echo "Installing $(TARGET) (with Pokémon tools) to $(BINDIR)..."
	@mkdir -p $(BINDIR)
	@cp $(BUILD_DIR)/$(BINARY) $(BINDIR)/$(TARGET)
	@chmod +x $(BINDIR)/$(TARGET)
	@echo "Installing man page to $(MANDIR)..."
	@mkdir -p $(MANDIR)
	@cp $(MANPAGE) $(MANDIR)/
	@echo "Installation complete! (includes Pokémon tools)"

# Install with all tools
install-all-tools: build-all-tools
	@echo "Installing $(TARGET) (with all tools) to $(BINDIR)..."
	@mkdir -p $(BINDIR)
	@cp $(BUILD_DIR)/$(BINARY) $(BINDIR)/$(TARGET)
	@chmod +x $(BINDIR)/$(TARGET)
	@echo "Installing man page to $(MANDIR)..."
	@mkdir -p $(MANDIR)
	@cp $(MANPAGE) $(MANDIR)/
	@echo "Installation complete! (includes all tools)"

# Uninstall binary and man page from PREFIX
uninstall:
	@echo "Removing $(TARGET) from $(BINDIR)..."
	@rm -f $(BINDIR)/$(TARGET)
	@echo "Removing man page from $(MANDIR)..."
	@rm -f $(MANDIR)/ask-ai.1
	@echo "Uninstallation complete!"

# =============================================================================
# Local Installation (~/.local)
# =============================================================================

# Install locally (for development)
install-local: build
	@echo "Installing $(TARGET) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@mkdir -p ~/.local/share/man/man1
	@cp $(MANPAGE) ~/.local/share/man/man1/
	@echo "Local installation complete!"
	@echo "Binary: ~/.local/bin/$(TARGET)"
	@echo "Manpage: ~/.local/share/man/man1/ask-ai.1"
	@echo "Make sure ~/.local/bin is in your PATH"

# Install locally with Pokémon tools
install-local-pokemon: build-pokemon
	@echo "Installing $(TARGET) (with Pokémon tools) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@mkdir -p ~/.local/share/man/man1
	@cp $(MANPAGE) ~/.local/share/man/man1/
	@echo "Local installation complete! (includes Pokémon tools)"

# Install locally with all tools
install-local-all-tools: build-all-tools
	@echo "Installing $(TARGET) (with all tools) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@mkdir -p ~/.local/share/man/man1
	@cp $(MANPAGE) ~/.local/share/man/man1/
	@echo "Local installation complete! (includes all tools)"

# =============================================================================
# Development Targets
# =============================================================================

# Clean build artifacts
clean:
	cargo clean
	@echo "Build artifacts cleaned"

# Run tests
test:
	cargo test

# Run tests with all features
test-all:
	cargo test --features all-tools

# Run cargo check
check:
	cargo check

# Run clippy
lint:
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt

# Build debug version
debug:
	cargo build

# =============================================================================
# Termux/Android Cross-Compilation
# =============================================================================

# Requires: cargo install cross --git https://github.com/cross-rs/cross
# Requires: Docker or Podman running
# See Cross.toml for configuration

# Build for Termux (Android aarch64) - no sandbox (Android provides isolation)
termux:
	@echo "Building for Termux (aarch64-linux-android)..."
	@echo "Note: Requires 'cross' and Docker/Podman. Run: cargo install cross --git https://github.com/cross-rs/cross"
	cross build --target $(TERMUX_TARGET) $(CARGO_FLAGS)
	@echo "Binary: $(TERMUX_BUILD_DIR)/$(BINARY)"

# Build for Termux with all tools (no sandbox - Android provides isolation)
termux-all-tools:
	@echo "Building for Termux with all tools (no sandbox)..."
	cross build --target $(TERMUX_TARGET) $(CARGO_FLAGS) $(FEATURE_ALL_NO_SANDBOX)
	@echo "Binary: $(TERMUX_BUILD_DIR)/$(BINARY)"

# =============================================================================
# Distribution Tarballs
# =============================================================================

# Create tarball for current platform
tarball: build
	@echo "Creating distribution tarball..."
	@mkdir -p $(DIST_DIR)
	@cd $(BUILD_DIR) && tar -czvf $(CURDIR)/$(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64.tar.gz $(BINARY) -C $(CURDIR) man/ask-ai.1 README.md LICENSE.txt
	@echo "Created: $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64.tar.gz"

# Create Linux x86_64 tarball with installation scripts
tarball-linux: build
	@echo "Creating Linux x86_64 tarball..."
	@mkdir -p $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64
	@cp $(BUILD_DIR)/$(BINARY) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/$(BINARY)
	@cp $(MANPAGE) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/$(BINARY).1
	@cp README.md $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/
	@cp LICENSE.txt $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/ 2>/dev/null || cp LICENSE $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/ || true
	@cp $(INSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/
	@cp $(UNINSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/
	@sed -i 's/^VERSION=""/VERSION="$(VERSION)"/' $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64/install.sh
	@cd $(DIST_DIR) && tar -czvf $(TARBALL_BASE)-linux-x86_64.tar.gz $(TARBALL_BASE)-linux-x86_64
	@rm -rf $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64
	@echo "Created: $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64.tar.gz"

# Create Linux x86_64 tarball with all tools
tarball-linux-all-tools: build-all-tools
	@echo "Creating Linux x86_64 tarball (all tools)..."
	@mkdir -p $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools
	@cp $(BUILD_DIR)/$(BINARY) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/$(BINARY)
	@cp $(MANPAGE) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/$(BINARY).1
	@cp README.md $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/
	@cp LICENSE.txt $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/ 2>/dev/null || cp LICENSE $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/ || true
	@cp $(INSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/
	@cp $(UNINSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/
	@sed -i 's/^VERSION=""/VERSION="$(VERSION)"/' $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools/install.sh
	@cd $(DIST_DIR) && tar -czvf $(TARBALL_BASE)-linux-x86_64-all-tools.tar.gz $(TARBALL_BASE)-linux-x86_64-all-tools
	@rm -rf $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools
	@echo "Created: $(DIST_DIR)/$(TARBALL_BASE)-linux-x86_64-all-tools.tar.gz"

# Create Termux tarball with installation scripts
tarball-termux: termux
	@echo "Creating Termux tarball..."
	@mkdir -p $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)
	@cp $(TERMUX_BUILD_DIR)/$(BINARY) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/$(BINARY)
	@cp $(MANPAGE) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/ask-ai.1
	@cp README.md $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/
	@cp LICENSE.txt $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/ 2>/dev/null || cp LICENSE $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/ || true
	@cp README-TERMUX.txt $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/
	@cp $(INSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/
	@cp $(UNINSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/
	@sed -i 's/^VERSION=""/VERSION="$(VERSION)"/' $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)/install.sh
	@cd $(DIST_DIR) && tar -czvf $(TARBALL_BASE)-termux-$(TERMUX_TARGET).tar.gz $(TARBALL_BASE)-termux-$(TERMUX_TARGET)
	@rm -rf $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)
	@echo "Created: $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET).tar.gz"

# Create Termux tarball with all tools
tarball-termux-all-tools: termux-all-tools
	@echo "Creating Termux tarball (all tools)..."
	@mkdir -p $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools
	@cp $(TERMUX_BUILD_DIR)/$(BINARY) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/$(BINARY)
	@cp $(MANPAGE) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/ask-ai.1
	@cp README.md $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/
	@cp LICENSE.txt $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/ 2>/dev/null || cp LICENSE $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/ || true
	@cp README-TERMUX.txt $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/
	@cp $(INSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/
	@cp $(UNINSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/
	@sed -i 's/^VERSION=""/VERSION="$(VERSION)"/' $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools/install.sh
	@cd $(DIST_DIR) && tar -czvf $(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools.tar.gz $(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools
	@rm -rf $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools
	@echo "Created: $(DIST_DIR)/$(TARBALL_BASE)-termux-$(TERMUX_TARGET)-all-tools.tar.gz"

# Create tarball with scripts for current platform
tarball-with-scripts: build
	@mkdir -p $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)
	@cp $(BUILD_DIR)/$(BINARY) $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/
	@cp $(MANPAGE) $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/ask-ai.1
	@cp README.md LICENSE.txt $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/ 2>/dev/null || cp README.md LICENSE $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/ || true
	@cp $(INSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/
	@cp $(UNINSTALL_SCRIPT) $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/
	@sed -i 's/^VERSION=""/VERSION="$(VERSION)"/' $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)/install.sh
	@cd $(DIST_DIR) && tar -czvf $(TARBALL_BASE)-linux-$(shell uname -m).tar.gz $(TARBALL_BASE)-$(shell uname -m)
	@rm -rf $(DIST_DIR)/$(TARBALL_BASE)-$(shell uname -m)
	@echo "Created: $(DIST_DIR)/$(TARBALL_BASE)-linux-$(shell uname -m).tar.gz"

# Create all distribution tarballs
all-tarballs: tarball-linux tarball-linux-all-tools tarball-termux tarball-termux-all-tools
	@echo ""
	@echo "All tarballs created in $(DIST_DIR)/:"
	@ls -lh $(DIST_DIR)/*.tar.gz
	@echo ""
	@echo "Installation instructions:"
	@echo "  Linux:   tar -xzf ask-ai-$(VERSION)-linux-x86_64.tar.gz && cd ask-ai-$(VERSION)-linux-x86_64 && ./install.sh"
	@echo "  Termux:  tar -xzf ask-ai-$(VERSION)-termux-aarch64.tar.gz && cd ask-ai-$(VERSION)-termux-aarch64 && ./install.sh"

# Clean distribution directory
clean-dist:
	@rm -rf $(DIST_DIR)
	@echo "Distribution directory cleaned"

# =============================================================================
# Help
# =============================================================================

help:
	@echo "Available targets:"
	@echo ""
	@echo "Build targets:"
	@echo "  make build              - Build release binary (default: weather, file-tools)"
	@echo "  make build-pokemon      - Build with Pokémon tools (adds 8 Pokémon tools)"
	@echo "  make build-all-tools    - Build with all tools + sandbox (Linux)"
	@echo "  make debug              - Build debug version"
	@echo ""
	@echo "Installation targets:"
	@echo "  make install            - Install binary and man page (default: /usr/local)"
	@echo "  make install-pokemon    - Install with Pokémon tools"
	@echo "  make install-all-tools  - Install with all tools"
	@echo "  make install-local      - Install to ~/.local/bin"
	@echo "  make install-local-pokemon - Install to ~/.local/bin with Pokémon tools"
	@echo "  make install-local-all-tools - Install to ~/.local/bin with all tools"
	@echo "  make uninstall          - Remove from PREFIX"
	@echo ""
	@echo "Termux/Android builds:"
	@echo "  make termux             - Build for Termux (aarch64, no sandbox)"
	@echo "  make termux-all-tools   - Build for Termux with all tools (no sandbox)"
	@echo "  make tarball-linux      - Create Linux x86_64 tarball with install scripts"
	@echo "  make tarball-linux-all-tools - Create Linux x86_64 tarball (all tools + sandbox)"
	@echo "  make tarball-termux     - Create Termux tarball with install scripts"
	@echo "  make tarball-termux-all-tools - Create Termux tarball (all tools, no sandbox)"
	@echo ""
	@echo "Development targets:"
	@echo "  make clean              - Clean build artifacts"
	@echo "  make test               - Run tests"
	@echo "  make test-all           - Run tests with all features"
	@echo "  make check              - Run cargo check"
	@echo "  make lint               - Run clippy"
	@echo "  make fmt                - Format code"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=<path>           - Installation prefix (default: /usr/local)"
	@echo "  VERSION=<version>       - Version for tarball (default: from Cargo.toml)"
	@echo ""
	@echo "Examples:"
	@echo "  make install                           # Install to /usr/local"
	@echo "  make install PREFIX=/usr               # Install to /usr"
	@echo "  make install PREFIX=~/.local           # Install to ~/.local"
	@echo "  make install-local-pokemon             # Install locally with Pokémon tools"
	@echo "  make install-local-all-tools           # Install locally with all tools"
	@echo "  make termux                            # Build for Android/Termux"
	@echo "  make tarball-termux                    # Create Termux distribution tarball"
	@echo "  make all-tarballs                      # Create all distribution tarballs"
	@echo ""
	@echo "Remote Installation:"
	@echo "  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash"
	@echo "  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --version 0.25.0"
	@echo "  curl -sL https://raw.githubusercontent.com/anomalyco/ask-ai/main/scripts/install-ask-ai.sh | bash -s -- --tools all"