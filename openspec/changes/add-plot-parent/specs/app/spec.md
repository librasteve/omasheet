## MODIFIED Requirements

### Requirement: The plot pane follows the sheet

#### Scenario: Changing sheet
- **WHEN** another sheet is shown
- **THEN** the pane lists the plots whose `data` reads it, and draws the first

#### Scenario: Constants
- **WHEN** the constants are shown
- **THEN** the pane lists no plots, and `+ Plot` is not offered
