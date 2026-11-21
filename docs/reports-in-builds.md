# Reports on every build

A report is a file that turns data into a document, so it can be rebuilt whenever the data changes. This chapter is the recipe: render every report, apply the theme, keep the result in the repository, and send it where the team will read it.

## One command

```sh
omasheet render benchmarks.md --html --now 2026-11-20T09:00 -o public/benchmarks.html
```

- `--html` writes a page, and `-o` says where.
- `--now` fixes what `today()` and `now()` give, and `--seed` fixes `rand`, `roll` and `pick`, so that two runs over the same data give the same file and a change in the output is a change in the data.
- `--zone Europe/London` stands in for the machine's time zone, for a build that runs somewhere else.

A report that has a mistake is reported at its line, and nothing is written, so a build that renders reports fails where it should.

## A script

```sh
#!/bin/sh
set -e
mkdir -p public
cp technical.css public/
for report in reports/*.md; do
  name=$(basename "$report" .md)
  omasheet render "$report" --html --now "$(date +%Y-%m-%dT%H:%M)" -o "public/$name.html"
done
```

Every `.md` in `reports/` becomes a page in `public/`, with the theme beside it. A report links `technical.css` as described in [Styling a report](report-styling.md), so the stylesheet is copied into the same folder.

The `omasheet` command has to be installed where the build runs. The installers cover macOS, Windows, Arch and Omarchy, and the [README](../README.md) says how to build from source for another Linux.

## Keep the result

Whether the rendered pages belong in the repository is a choice. Committing them gives the team a page to open and a diff that shows what changed in the numbers. Leaving them out keeps the repository to the sources. Either way the `.md` and the `.omx` it reads are the sources, and the HTML is the output.

## Send it on

Once the pages are built, they are ordinary files:

- **An intranet or a static site:** copy the `public/` folder to wherever the site is served from.
- **Email:** the HTML is a single page that refers only to its stylesheet, so attach the page and the theme, or render to Markdown, which needs no stylesheet, and paste it.
- **A pull request:** commit the rendered page with the change that caused it, so a reviewer sees the numbers move.

## Using an AI to run it

The steps above are the whole of it, which makes them a good job to hand to an AI agent: it can write the report, run `render`, read any error, fix the document and render again. See [Working with AI agents](ai-agents.md).

## A note on rates

If a report converts currency, the figures depend on the rates in use when it was built. `omasheet rates` says what date they are as of, and `omasheet rates update` fetches the latest. A build should either fetch deliberately before rendering or state the date in the report. See [Exact money](exact-money.md).
