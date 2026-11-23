## ADDED Requirements

### Requirement: The plot settings pane

#### Scenario: Choosing a kind
- **WHEN** a tile of the settings is pressed
- **THEN** the plot's `type` is that kind, and the plot is drawn again

#### Scenario: Ticking columns
- **WHEN** a chip under Values is pressed
- **THEN** that column is in `y` or not; with every column ticked `y` is left out

### Requirement: Clicking a mark

#### Scenario: A line or a bar
- **WHEN** a mark of a line, bar or other plot is clicked
- **THEN** the row of the sheet it stands for is selected
