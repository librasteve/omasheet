# packaging

## ADDED Requirements

### Requirement: The package carries the agent skill
The package SHALL install the Omasheet skill to `/usr/share/omasheet/agent-skill/omasheet/SKILL.md`. On install and on upgrade, and not on removal, the package SHALL print the command that links that directory into `~/.claude/skills/omasheet`. The package SHALL NOT create the link itself.

#### Scenario: Installed
- **WHEN** the package is installed
- **THEN** `/usr/share/omasheet/agent-skill/omasheet/SKILL.md` exists
- **AND** the install output shows `ln -sfn /usr/share/omasheet/agent-skill/omasheet ~/.claude/skills/omasheet`

#### Scenario: Upgraded
- **WHEN** the package is upgraded
- **THEN** the same command is shown again

#### Scenario: Removed
- **WHEN** the package is removed
- **THEN** the command is not shown
- **AND** the caches are refreshed as before

#### Scenario: Linked
- **GIVEN** the package is installed
- **WHEN** the user runs the printed command
- **THEN** Claude Code lists the `omasheet` skill
