## MODIFIED Requirements

### Requirement: The plot pane follows the sheet

#### Scenario: Changing sheet
- **WHEN** another sheet is shown
- **THEN** the pane lists the plots whose `data` reads it, and draws the first

#### Scenario: All plots
- **WHEN** `all` is pressed
- **THEN** every plot of the file is listed, whatever it reads
