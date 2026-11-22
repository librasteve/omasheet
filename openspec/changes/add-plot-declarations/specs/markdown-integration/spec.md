## ADDED Requirements

### Requirement: Plots in a document

A plot declared in an `omx` block or a file the front matter names SHALL be drawn, in place of its block, as an inline SVG, and `{{ Name }}` where `Name` is a plot SHALL be replaced by its SVG. The SVG SHALL hold no blank line, so that an HTML block is not ended inside it.

#### Scenario: A plot in a block
- **WHEN** an `omx` block holds a sheet and a `plot` over it
- **THEN** the output shows the sheet's table and the plot's `<svg>`, in Markdown and in HTML

#### Scenario: A plot by name
- **WHEN** the text says `{{ Revenue }}` and `Revenue` is a plot
- **THEN** it is replaced by that plot's `<svg>`

## REMOVED Requirements

### Requirement: Plot blocks

The fenced `plot` block is removed; it was never released.
