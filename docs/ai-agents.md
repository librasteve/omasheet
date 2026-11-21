# Working with AI agents

An `.omx` file is readable text with typed columns and one formula per column, which makes it a file an AI agent can read, write and check. Omasheet comes with an **agent skill**: a page of instructions that teaches an agent the format, the language and the habit of checking its work.

## Give the agent the skill

Prompt your AI with:

```text
Please load the Omasheet skill from https://raw.githubusercontent.com/librasteve/omasheet/main/skill/omasheet/SKILL.md and follow its instructions when creating, checking, or converting spreadsheets.
```

The Arch package also installs the skill to `/usr/share/omasheet/agent-skill/omasheet`. To give it to Claude Code:

```sh
ln -sfn /usr/share/omasheet/agent-skill/omasheet ~/.claude/skills/omasheet
```

The skill is `skill/omasheet/SKILL.md` in the repository. If the repository's `docs/format.md` and `docs/expressions.md` are there, they are newer than the skill, and the skill tells the agent to read them when something does not work.

## The loop

The skill asks the agent to work the same way a person should:

1. Write or edit the `.omx` text.
2. `omasheet lint` it. Fix every error; they are all reported at once.
3. Print it, `omasheet print file.omx`, and read the calculated sheets. A cell showing `#ERROR` failed, and the reason is printed after the sheets.
4. Spot-check a number or two with `omasheet eval`.

An agent should not report a file as working until step 3 has run clean. The language is young, so the tool is the authority and not any page about it.

Errors come as `file:line:col: error: message`, with the line and a marker, and often a `help:` line that says what to write instead, which is the part an agent should read:

```text
<expression>:1:1: error: cannot add kg and m: they measure Mass and Length
```

## What an agent is good at here

- **Writing a sheet from a description.** A table of data, the typed columns, and the formulas, in a file that lints.
- **Converting.** `omasheet import` turns an XLSX Workbook, a CSV, JSON or Parquet file into an `.omx`, and an agent can then tidy the types and add the calculated columns. See [Interchange](interop.md#import).
- **Checking.** Reading a file and saying what a column calculates, or whether a unit is used consistently, is a job where the dimension errors do the pointing.
- **Writing reports.** A Markdown document with a sheet inside, rendered and checked in the same loop. See [Markdown reports](reports.md).

## What to ask it not to do

- **Run the app to read a file.** `omasheet` with no command, or with only a file, opens the desktop app in a real window. Use `print` and `eval`, which are for reading.
- **Edit the `.expected` files by hand.** In the repository's `examples/`, each `.omx` has an `.expected` beside it that the tests compare. Regenerate it with `omasheet print examples/name.omx > examples/name.expected` and read the diff.

## If the command is missing

The skill tells the agent to say so, and without the program a file can be written but not checked. The installers cover macOS, Windows, Arch and Omarchy, and a build from source covers other Linux. See the [README](../README.md#install).
