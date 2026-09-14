#!/bin/bash
# Quick auth check for PR review workflow
# Run this before starting any PR review to determine authentication path.

set -euo pipefail

if gh auth status &>/dev/null; then
    echo "OK: gh CLI authenticated"
    exit 0
fi

echo "WARN: gh CLI not authenticated"

# Check if we have a PAT in env
if [ -n "${GITHUB_TOKEN:-}" ]; then
    echo "OK: GITHUB_TOKEN is set (will use curl fallback)"
    exit 0
fi

echo "FAIL: No GitHub authentication available."
echo "Options:"
echo "  1. Run 'gh auth login --web' and open the link in a browser"
echo "  2. Provide a Personal Access Token (scope: repo, pull_requests:write)"
echo "  3. Set GITHUB_TOKEN in your environment"
exit 1
