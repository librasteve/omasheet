## ADDED Requirements

### Requirement: Making a plot

`+ Plot` SHALL ask for a name, a kind and the data, and add the plot to the end of the file, as one step to undo, and show it in the pane. A name used by a sheet, a constant or another plot, a kind that is not one, and a `data` that does not give a sheet SHALL be refused with why, and leave the file as it was.

#### Scenario: A new plot
- **WHEN** `+ Plot` is chosen with the name `Share`, the kind `pie` and the data `Sales`
- **THEN** the file ends with `plot Share`, `type: pie` and `data: Sales`, the pane shows it, and one undo takes it out

### Requirement: Changing a plot

The pane SHALL change a plot's `type`, `data`, `x`, `y` and `title`, rename it and delete it, each as one step to undo. A change that makes the file worse SHALL be refused with why. The columns offered for `x` and `y` are those of the sheet its `data` gives.

#### Scenario: Ticking a column
- **WHEN** the `y` column `Cost` is ticked on a plot with `y: Revenue`
- **THEN** the plot's line reads `y: Revenue, Cost`, and unticking both takes the line out

#### Scenario: Renaming a sheet
- **WHEN** a sheet is renamed
- **THEN** every plot whose `data` names it names the new one
