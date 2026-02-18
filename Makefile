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

# Feature flags
FEATURE_POKEMON = --features pokemon-tools
FEATURE_ALL = --features all-tools

.PHONY: all build install uninstall clean test check build-pokemon build-all-tools install-pokemon install-all-tools install-local-pokemon install-local-all-tools test-all

# Default target
all: build

# Build the release binary (default features: weather, file-tools)
build:
	cargo build $(CARGO_FLAGS)

# Build with Pokémon tools (adds 8 Pokémon-related tools)
build-pokemon:
	cargo build $(CARGO_FLAGS) $(FEATURE_POKEMON)

# Build with all tools (weather, file, web-search, pokemon)
build-all-tools:
	cargo build $(CARGO_FLAGS) $(FEATURE_ALL)

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

# Install locally (for development)
install-local: build
	@echo "Installing $(TARGET) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@echo "Local installation complete!"
	@echo "Make sure ~/.local/bin is in your PATH"

# Install locally with Pokémon tools
install-local-pokemon: build-pokemon
	@echo "Installing $(TARGET) (with Pokémon tools) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@echo "Local installation complete! (includes Pokémon tools)"

# Install locally with all tools
install-local-all-tools: build-all-tools
	@echo "Installing $(TARGET) (with all tools) to ~/.local/bin..."
	@mkdir -p ~/.local/bin
	@cp $(BUILD_DIR)/$(BINARY) ~/.local/bin/$(TARGET)
	@chmod +x ~/.local/bin/$(TARGET)
	@echo "Local installation complete! (includes all tools)"

# Show help
help:
	@echo "Available targets:"
	@echo ""
	@echo "Build targets:"
	@echo "  make build              - Build release binary (default: weather, file-tools)"
	@echo "  make build-pokemon      - Build with Pokémon tools (adds 8 Pokémon tools)"
	@echo "  make build-all-tools    - Build with all tools (weather, file, web-search, pokemon)"
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
	@echo "Development targets:"
	@echo "  make clean              - Clean build artifacts"
	@echo "  make test               - Run tests"
	@echo "  make test-all           - Run tests with all features"
	@echo "  make check              - Run cargo check"
	@echo "  make lint               - Run clippy"
	@echo "  make fmt                - Format code"
	@echo ""
	@echo "Model Installation (run from modelfiles/):"
	@echo "  cd modelfiles && make models-essential   - Install required models"
	@echo "  cd modelfiles && make models-optional    - Install recommended models"
	@echo "  cd modelfiles && make models-all        - Install all local models"
	@echo "  cd modelfiles && make models-cloud       - Install cloud models"
	@echo "  cd modelfiles && make help                - Show model help"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=<path>           - Installation prefix (default: /usr/local)"
	@echo ""
	@echo "Examples:"
	@echo "  make install                           # Install to /usr/local"
	@echo "  make install PREFIX=/usr               # Install to /usr"
	@echo "  make install PREFIX=~/.local           # Install to ~/.local"
	@echo "  make install-local-pokemon             # Install locally with Pokémon tools"
	@echo "  make install-local-all-tools           # Install locally with all tools"
