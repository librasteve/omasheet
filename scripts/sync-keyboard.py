#!/usr/bin/env python3
"""Rewrite the key table of docs/keyboard.md from omasheet-ui/help.txt.

help.txt is the source: each line before the tips is `keys<two or more spaces>what it does`.
Run it after editing help.txt; `--check` exits 1 if the doc is out of date.
"""
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
help_txt = (root / "omasheet-ui" / "help.txt").read_text()
doc_path = root / "docs" / "keyboard.md"

rows = []
for line in help_txt.splitlines():
    if line.startswith("A cell starting"):
        break
    m = re.match(r"^(\S.*?)\s{2,}(\S.*)$", line)
    if m:
        rows.append((m.group(1), m.group(2)))

table = "| Keys | |\n|---|---|\n" + "\n".join(f"| {k} | {d} |" for k, d in rows)
doc = doc_path.read_text()
new = re.sub(
    r"(<!-- keys:begin[^>]*-->\n).*?(<!-- keys:end -->)",
    lambda m: m.group(1) + table + "\n" + m.group(2),
    doc,
    count=1,
    flags=re.S,
)
if "--check" in sys.argv:
    sys.exit(0 if new == doc else 1)
doc_path.write_text(new)
