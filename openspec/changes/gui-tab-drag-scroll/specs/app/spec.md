# app

## ADDED Requirements

### Requirement: The tab bar scrolls while a sheet tab is dragged

While a sheet tab is dragged and the tab bar scrolls, holding the pointer near either end of the bar SHALL scroll the bar that way, faster the nearer the pointer is to the end, until the end of the tabs. The landing mark SHALL stay on the gap under the pointer as the tabs move. Nothing SHALL scroll while the bar does not.

#### Scenario: A place off screen

- **GIVEN** more sheets than fit the bar, and the last one's tab out of view
- **WHEN** the first sheet's tab is dragged and held near the right end of the bar
- **THEN** the bar scrolls right, and the landing mark follows the gap under the pointer
- **AND** dropping it there moves the sheet to that place

#### Scenario: Few sheets

- **GIVEN** all the tabs fit the bar
- **WHEN** a tab is dragged to either end
- **THEN** the bar does not move
