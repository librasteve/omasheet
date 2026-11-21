# Styling a report

`omasheet render --html` writes a complete page with a small default style: the system font, a readable width, and light and dark colours that follow the reader's setting. To make a report look like it belongs to your team, give it a theme.

## A theme is a stylesheet

`omasheet render` has no theme option, and needs none. Raw HTML in a Markdown document passes through to the page unchanged, so a report links its own stylesheet:

```markdown
---
files: [benchmark.omx]
---

<link rel="stylesheet" href="technical.css">

# Opening a big file
```

The `<link>` lands in the page's body, and the browser applies the stylesheet from there. Keep the `.css` file beside the HTML you write. A tool that does not know Omasheet may show the `<link>` line as text or ignore it, so a theme is for the rendered report and not for the source.

## What to style

The rendered page is plain HTML: `h1` and `h2`, paragraphs, `code`, `table` with `thead` and `tbody`, and links. A theme sets colours, fonts and spacing on these elements, and can number the headings and add a letterhead or footer with CSS alone:

```css
:root { color-scheme: dark; --bg: #0b0f14; --fg: #d6dee8; --accent: #38bdf8; }
body { background: var(--bg); color: var(--fg); font: 15px/1.65 system-ui, sans-serif; }
body::before { content: "HALDEN SYSTEMS  ·  ENGINEERING REPORT"; display: block; border-bottom: 2px solid var(--accent); }
h2 { counter-increment: sec; text-transform: uppercase; }
h2::before { content: counter(sec, decimal-leading-zero) "  "; color: var(--accent); }
table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
th { text-transform: uppercase; border-bottom: 2px solid var(--accent); }
```

`examples/showcase/technical.css` is a complete dark theme in this style, an invented engineering house style with a letterhead band, numbered section headings, an instrument-panel table and a footer. `benchmarks.md` links it. Replace the name and the document number in `body::before` with your own.

## Tips

- Use system fonts. A theme that loads a font or a stylesheet from the web makes the report depend on the network, and a report on an intranet may have none.
- Print is a theme too: `@media print` can drop the background and the letterhead.
- A number in a table is easier to read with `font-variant-numeric: tabular-nums`, so the digits line up.
- Keep a theme with the reports it belongs to, so a build can copy the two together.

## Where next

[Reports on every build](reports-in-builds.md) renders the report and its theme as part of a build, and sends the result on.
