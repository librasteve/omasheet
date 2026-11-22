## ADDED Requirements

### Requirement: Plot pane

The window SHALL have a pane beside the grid that draws the file's plots, one at a time, chosen from a list, in the theme's colours, and redrawn when the file changes. A footer button and F7 SHALL show and hide it, and whether it is shown SHALL be remembered.

#### Scenario: A file with plots
- **WHEN** a file with a plot is open and the pane is shown
- **THEN** the pane draws the first plot, and offers the others

#### Scenario: A file with none
- **WHEN** the file has no plot
- **THEN** the pane says so and shows how one is written
