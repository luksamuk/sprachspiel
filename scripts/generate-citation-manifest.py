#!/usr/bin/env python3
"""Generates the citation manifest from the arXiv API.

This script is a MAINTAINER TOOL, not part of `cargo test`. Network access in the
test suite is not viable: this audit alone hit HTTP 429 on 19 of 58 requests from
a single IP, with 3s spacing between calls. A flaky test is worse than no test —
it trains people to re-run until green, which is the opposite of a guard.

The manifest it writes (`tests/data/citation-manifest.json`) is committed, so the
test suite can verify citations offline and deterministically. Refresh the
manifest only when a citation is added:

    python3 scripts/generate-citation-manifest.py

Then review the diff: a changed author list or year on an unchanged ID is either a
correction or a new misattribution, and both are worth seeing.
"""

import json
import os
import re
import subprocess
import sys
import time
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "tests" / "data" / "citation-manifest.json"
NS = {"a": "http://www.w3.org/2005/Atom", "arxiv": "http://arxiv.org/schemas/atom"}

# Directories whose Markdown/Rust may cite a paper.
SCAN = ["doc", "src", ".opencode", "AGENTS.md", "README.md"]
# Must match the scanner in tests/citation_integrity.rs, which accepts the URL form
# too: `https://arxiv.org/abs/2406.11931` is a citation and was missed here while
# the sensor saw it, so a citation could be invisible to the manifest.
ID_RE = re.compile(r"(?:arXiv:?\s*|arxiv\.org/abs/|(?<=\())(\d{4}\.\d{4,5})")


def cited_ids() -> list[str]:
    """Every arXiv ID cited anywhere we consider authoritative.

    Three forms appear in these documents: `arXiv:2510.04371`, the URL
    `arxiv.org/abs/2406.11931`, and a bare ID in a parenthetical `(2312.00752)`.
    Accepting only the first two left Mamba and Megalodon invisible to the
    manifest while the offline scanner saw them.
    """
    cmd = [
        "rg",
        "-o",
        r"(arXiv:?\s*|arxiv\.org/abs/|\()\d{4}\.\d{4,5}",
        "--no-filename",
        *SCAN,
    ]
    out = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True).stdout
    return sorted({m.group(1) for m in ID_RE.finditer(out)})


def fetch(aid: str, attempts: int = 4) -> dict | None:
    """One ID's real metadata, with backoff — the arXiv rate-limits by IP window."""
    url = f"http://export.arxiv.org/api/query?id_list={aid}&max_results=1"
    req = urllib.request.Request(url, headers={"User-Agent": "sprachspiel-citation-audit/1.0"})
    for attempt in range(attempts):
        try:
            body = urllib.request.urlopen(req, timeout=60).read().decode()
            entries = ET.fromstring(body).findall("a:entry", NS)
            if not entries:
                print(f"  {aid}: no entry (does the ID exist?)", file=sys.stderr)
                return None
            entry = entries[0]
            title_el = entry.find("a:title", NS)
            published_el = entry.find("a:published", NS)
            if title_el is None or title_el.text is None or published_el is None or published_el.text is None:
                return None
            authors = [a.find("a:name", NS) for a in entry.findall("a:author", NS)]
            record = {
                "title": " ".join(title_el.text.split()),
                "authors": [a.text for a in authors if a is not None and a.text],
                "year": published_el.text[:4],
            }
            comment = entry.find("arxiv:comment", NS)
            comment_text = comment.text if comment is not None else None
            if comment_text and "withdraw" in comment_text.lower():
                record["withdrawn"] = " ".join(comment_text.split())
            return record
        except Exception as exc:  # noqa: BLE001 — any failure is retried the same way
            wait = 15 * (2**attempt)
            if attempt == attempts - 1:
                print(f"  {aid}: giving up after {attempts} attempts ({exc})", file=sys.stderr)
                return None
            print(f"  {aid}: {exc}; retrying in {wait}s", file=sys.stderr)
            time.sleep(wait)
    return None


def main() -> int:
    ids = cited_ids()
    print(f"{len(ids)} arXiv IDs cited across {', '.join(SCAN)}")

    existing = json.loads(OUT.read_text()) if OUT.exists() else {}
    manifest: dict[str, dict] = {}

    for i, aid in enumerate(ids, 1):
        if aid in existing and not os.environ.get("REFRESH_ALL"):
            manifest[aid] = existing[aid]
            print(f"  [{i:2}/{len(ids)}] {aid}: cached")
            continue
        record = fetch(aid)
        if record:
            manifest[aid] = record
            flag = " [WITHDRAWN]" if record.get("withdrawn") else ""
            print(f"  [{i:2}/{len(ids)}] {aid}: {record['title'][:56]}{flag}")
        time.sleep(3.0)

    missing = [a for a in ids if a not in manifest]
    if missing:
        # A manifest that silently omits IDs would make the "cited implies known"
        # test pass by absence. Fail loudly instead.
        print(f"\n{len(missing)} IDs could not be resolved: {missing}", file=sys.stderr)
        print("Re-run before committing; do not commit a partial manifest.", file=sys.stderr)
        return 1

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(manifest, indent=1, ensure_ascii=False, sort_keys=True) + "\n")
    print(f"\nwrote {OUT.relative_to(REPO)} ({len(manifest)} entries)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
