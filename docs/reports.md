# Markdown reports

A report is a Markdown document with a sheet inside it. The numbers in the prose are worked out from the data, so the explanation and the calculation live in one file that can be reviewed and diffed.

## Render a document

```sh
omasheet render report.md                      # Markdown, with the values in place
omasheet render report.md --html -o report.html
```

Without `-o` the result is printed. With `--html` it is a complete HTML page.

## Quote a file, or write one in the document

`examples/showcase/benchmarks.md` is a report that reads `benchmark.omx`, the sheet from [Your first sheet](first-sheet.md):

```markdown
---
title: Opening a big file
files: [benchmark.omx]
---

# Opening a big file

How long does it take to put the first screen of rows up? Every figure below is worked out from the sheet, measured against a target of {{ Target }}.

## The runs

{{ Benchmark |> select(Run, FileSize, Rows, Warm, Cold, Change, Beats_50ms) }}

## What it shows

The CSV of {{ Benchmark.FileSize[0] }} takes {{ Benchmark.Cold[0] }} cold, over the {{ Target }} target.
```

It renders as:

```text
The CSV of 1,060 MiB takes 57.0 ms cold, over the 50ms target.
```

and the `select(…)` sheet becomes a Markdown table with the column formats of the file. The sheet need not be in a separate file: a fenced `omx` block is a file too, shown as its sheets, and `examples/showcase/short-report.md` has its sheet inside.

## What is in a document

- `files:` in the front matter names `.omx` files, from where the document is. `files: budget.omx` and a list of `- budget.omx` lines work too.
- A fenced `omx` block is a file, shown as its sheets. The files and all the blocks of a document are one file, so each can read the others.
- `{{ expression }}` is replaced by its value. Several values read as a list, `Feb, Mar`, and a sheet is shown as a Markdown table.
- `{{` in code is left alone, and so is `\{{`. To show the braces themselves, put them in backticks.
- A plot is drawn as an SVG; see [Plots](#plots).
- Text written bare in a cell may hold Markdown, `**bold**` or `[a link](there.md)`, and is read as that in the document. Text in quotes, or made by a formula, is shown as it is. HTML in a cell is shown as text.
- A mistake is reported at its line in the document, or in the file it is in, and nothing is written.

## Plots

A plot is declared in the file, as a sheet is (see [Plots](format.md#plots) for the declaration), and a report draws it as an SVG:

````markdown
---
files: [plots.omx]
---

The trend:

{{ Trend }}
````

- `{{ Name }}`, where `Name` is a plot, is replaced by its SVG, where it is written.
- A plot declared in a fenced `omx` block is drawn in the block, after the sheets before it and before those after it, in the order written.
- The SVG is in the page itself, in both the Markdown and the HTML, with no other files. Its ink is the page's text colour and its background is clear, so it follows a light or dark theme.
- A mistake in a plot is reported at its line, in the document or in the file it is in.

`examples/plot-report.md` has one of each kind.

A tool that does not know Omasheet shows the block as code and the braces as they are, so a report stays readable on GitHub.

## Repeatable

`today()` and `now()` read the clock, and `rand`, `roll` and `pick` are random, so a report that uses them changes between runs. `--now 2026-11-20T09:00` and `--seed 7` fix them, which is what you want when a report is rebuilt and compared. A converted currency depends on the rates in use, so say what date they were as of; see [Exact money](exact-money.md).

## Where next

[Styling a report](report-styling.md) gives the HTML a theme, and [Reports on every build](reports-in-builds.md) runs `render` as part of a build and sends the result on. The reference for everything above is [Markdown](interop.md#markdown) in the interchange chapter.
