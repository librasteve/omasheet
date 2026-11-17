# Proposal: Ship the Omasheet skill with the package

## Why

The skill that teaches an AI agent to write, check and convert `.omx` files lives at `.claude/skills/omasheet/SKILL.md`, and the README tells users to point their AI at its raw GitHub URL. That works for someone who has read the README. It does nothing for someone who has installed the package: the skill is not on their machine, and nothing tells them it exists.

Installing the package should leave the skill on disk, a symlink away from being loaded by Claude Code.

## What Changes

- **packaging** — ADDED: the package installs the skill to `/usr/share/omasheet/agent-skill/omasheet/`, and prints the one-line `ln -sfn` command that links it into `~/.claude/skills/` on install and upgrade
- The skill moves from `.claude/skills/omasheet/` to `skill/omasheet/`, so it is a first-class part of the repository and the package builds from it. `.claude/skills/omasheet` stays as a symlink to it, so the skill still loads inside the repository
- `README.md`: the raw URL points at `skill/omasheet/SKILL.md`, and the install note shows the link command
- `SKILL.md`: where `omasheet` is not on the PATH, it offers the package and `cargo install --locked`, rather than `cargo install` alone
- Earlier `openspec/changes/**` notes now name the new skill path

## Impact

- `skill/omasheet/SKILL.md`, `.claude/skills/omasheet` (symlink)
- `pkgbuild/PKGBUILD` and `pkgbuild/omarchy-pkgs/omasheet/PKGBUILD`: one `install -Dm644` line in `package()`
- `pkgbuild/omasheet.install` and its copy under `omarchy-pkgs`: `_refresh_caches` and `_skill_hint` helpers; the hint is printed by `post_install` and `post_upgrade`, not `post_remove`
- The Omarchy `PKGBUILD` builds the tagged tarball, so it carries the skill only from the next tag
- The README's raw URL does not resolve until the branch is merged to `main`

## Out of Scope

- Linking the skill automatically. A package install script runs as root and does not know whose `~/.claude` to use, so the link is a command the user runs
- The OmaStore route. It unpacks into `$HOME` and installs only what its manifest names, so those users get the skill from the README URL or the repository
- Skills for agents other than Claude Code
- Changing the skill's content beyond its install advice
