#!/usr/bin/env python3
"""Turn release notes into a 1200x630 social card (HTML, then PNG via Chrome).

usage: render.py NOTES.md OUT.html [--tag v0.1.12] [--name "Omasheet 0.1.12"]

The card shows the release name, the notes' opening paragraph, and the first
three feature sections: heading, the first sentence of its first bullet, and
up to four code spans from it as chips. Sections named Install, Known issues,
By release and the like are skipped. A hidden <!-- card ... --> block in the notes overrides all of this (see
parse_block). Nothing is fetched; fonts are the system's.
"""
import argparse, html, pathlib, re

SKIP = re.compile(r"install|update|known|by release|fixed|installers|packaging|in the app", re.I)
HERE = pathlib.Path(__file__).parent


def inline(text):
    """Strip Markdown links and emphasis down to plain text."""
    text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", text)
    return re.sub(r"\*\*|__", "", text)


def parse_block(md):
    """An optional hidden block in the notes that sets the card's content:

    <!-- card
    title: Omasheet 0.1.12: units that understand dimensions
    summary: One or two sentences.
    Units | Dimensions are checked, not guessed. | `5kg + 300g` `9.81m.s^-2`
    -->
    """
    m = re.search(r"<!--\s*card\s*\n(.*?)-->", md, re.S)
    if not m:
        return None
    title = summary = ""
    picks = []
    for l in m.group(1).splitlines():
        l = l.strip()
        if l.startswith("title:"):
            title = l[6:].strip()
        elif l.startswith("summary:"):
            summary = l[8:].strip()
        elif l.count("|") >= 1:
            parts = [x.strip() for x in l.split("|", 2)] + ["", ""]
            picks.append({"head": parts[0], "text": parts[1],
                          "chips": re.findall(r"`([^`]+)`", parts[2])[:4]})
    return title, summary, picks[:3]


def parse(md):
    block = parse_block(md)
    lines = md.splitlines()
    title = next((l[2:].strip() for l in lines if l.startswith("# ")), "")
    summary, sections, cur = "", [], None
    for l in lines:
        if l.startswith("## "):
            cur = {"head": l[3:].strip(), "body": []}
            sections.append(cur)
        elif cur is not None:
            cur["body"].append(l)
        elif not summary and l.strip() and not l.startswith("#") and not l.startswith("!["):
            summary = inline(l.strip()).replace("`", "")
    picks = []
    for s in sections:
        if SKIP.search(s["head"]):
            continue
        bullet = next((b for b in s["body"] if b.lstrip().startswith(("- ", "* "))), "")
        bullet = bullet.lstrip()[2:] or next((b for b in s["body"] if b.strip()), "")
        chips = re.findall(r"`([^`]{1,28})`", bullet)
        chips = [c for c in dict.fromkeys(chips)][:4]
        text = inline(re.sub(r"`", "", bullet))
        first = re.split(r"(?<=[.!?])\s", text, maxsplit=1)[0]
        picks.append({"head": s["head"], "text": first[:140], "chips": chips})
        if len(picks) == 3:
            break
    if block:
        return block[0] or title, block[1] or summary, block[2] or picks
    return title, summary, picks


def render(title, summary, picks, tag):
    tpl = (HERE / "template.html").read_text()
    e = html.escape
    items = ""
    for i, p in enumerate(picks, 1):
        chips = "".join(f"<code>{e(c)}</code>" for c in p["chips"])
        items += (
            f'<li><span class="n">{i}</span><div><h2>{e(p["head"])}</h2>'
            f'<p>{e(p["text"])}</p><div class="chips">{chips}</div></div></li>\n'
        )
    summary = summary if len(summary) <= 200 else summary[:197].rsplit(" ", 1)[0] + "…"
    return (tpl.replace("{{TITLE}}", e(title)).replace("{{TAG}}", e(tag))
               .replace("{{SUMMARY}}", e(summary)).replace("{{ITEMS}}", items))


def page(title, summary, tag, repo, site):
    """The share page: Open Graph tags for crawlers, a redirect for people."""
    e = html.escape
    share = f"{site}/r/{tag}/"
    release = f"https://github.com/{repo}/releases/tag/{tag}"
    return ((HERE / "page.html").read_text()
            .replace("{{TITLE}}", e(title, quote=True)).replace("{{SUMMARY}}", e(summary, quote=True))
            .replace("{{SHARE}}", share).replace("{{RELEASE}}", release))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("notes"); ap.add_argument("out")
    ap.add_argument("--tag", default=""); ap.add_argument("--name", default="")
    ap.add_argument("--page", help="also write the share page here")
    ap.add_argument("--repo", default="librasteve/omasheet")
    ap.add_argument("--site", default="https://omasheet.org")
    a = ap.parse_args()
    title, summary, picks = parse(pathlib.Path(a.notes).read_text())
    if a.page:
        pathlib.Path(a.page).write_text(page(a.name or title, summary, a.tag, a.repo, a.site))
    pathlib.Path(a.out).write_text(render(a.name or title, summary, picks, a.tag))
