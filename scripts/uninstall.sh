#!/usr/bin/env bash
# =============================================================================
# Sprachspiel Uninstaller
# =============================================================================
# This script removes sprachspiel from your system.
#
# Usage:
#   ./uninstall.sh    # Remove sprachspiel
#   ./uninstall.sh --help  # Show help
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/install.sh" --uninstall "$@"