# Makefile for ask-ai (ask-ollama-rs)

# Default installation prefix
PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
MANDIR = $(PREFIX)/share/man/man1

# Binary name
BINARY = ask-ollama
TARGET = ask-ai

# Man page
MANPAGE = man/ask-ai.1

# Build configuration
CARGO_FLAGS = --release
BUILD_DIR = target/release

.PHONY: all build install uninstall clean test check

# Default target
all: build

# Build the release binary
build:
	cargo build $(CARGO_FLAGS)

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

# Uninstall binary and man page from PREFIX
uninstall:
	@echo "Removing $(TARGET) from $(BINDIR)..."
	@rm -f $(BINDIR)/$(TARGET)
	@echo "Removing man page from $(MANDIR)..."
	@rm -f $(MANDIR)/ask-ai.1
	@echo "Uninstallation complete!"

# Clean build artifacts
clean:
	cargo clean
	@echo "Build artifacts cleaned"

# Run tests
test:
	cargo test

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

# Install locally (for development)
install-local: build
	@echo "Installing $(TARGET) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@echo "Local installation complete!"
	@echo "Make sure ~/.local/bin is in your PATH"

# Show help
help:
	@echo "Available targets:"
	@echo "  make build         - Build release binary"
	@echo "  make install       - Install to $(PREFIX)/bin (default: /usr/local/bin)"
	@echo "  make install-local - Install to ~/.local/bin"
	@echo "  make uninstall     - Remove from $(PREFIX)/bin"
	@echo "  make clean         - Clean build artifacts"
	@echo "  make test          - Run tests"
	@echo "  make check         - Run cargo check"
	@echo "  make lint          - Run clippy"
	@echo "  make fmt           - Format code"
	@echo "  make debug         - Build debug version"
	@echo "  make help          - Show this help"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=<path>      - Installation prefix (default: /usr/local)"
	@echo ""
	@echo "Examples:"
	@echo "  make install                    # Install to /usr/local/bin"
	@echo "  make install PREFIX=/usr      # Install to /usr/bin"
	@echo "  make install PREFIX=~/.local  # Install to ~/.local/bin"
