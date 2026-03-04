#!/usr/bin/env bash
# =============================================================================
# Ask AI Uninstaller
# =============================================================================
# This script removes ask-ai from your system.
#
# Usage:
#   ./uninstall.sh    # Remove ask-ai
#   ./uninstall.sh --help  # Show help
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/install.sh" --uninstall "$@"