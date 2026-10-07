# markdown-integration

Markdown is presentation; Omasheet is computation.

## ADDED Requirements

### Requirement: Peer formats
Markdown (`.md`) and Omasheet (`.omx`) SHALL remain separate formats that
reference each other. An `.omx` file SHALL NOT need Markdown to be useful, and a
Markdown file with Omasheet content SHALL remain valid Markdown for renderers that
do not know Omasheet.

#### Scenario: Unaware renderer
- **GIVEN** a Markdown file containing `{{ Sales.Revenue.sum() }}` and a fenced `omx` block
- **WHEN** it is rendered by a Markdown tool with no Omasheet support
- **THEN** it renders without error, showing the interpolation text and the block as a code block

### Requirement: Fenced omx blocks
A fenced code block with the language tag `omx` in a Markdown file SHALL be
treated as `.omx` source. Tables and constants it declares SHALL be available
to the rest of that document, and the block SHALL render as its evaluated tables.

#### Scenario: Inline table
- **GIVEN** a Markdown file with
  ````markdown
  ```omx
  table Sales

  Month | Revenue
  Jan   | 100
  Feb   | 120
  ```
  ````
- **WHEN** the document is rendered
- **THEN** the block is replaced by a rendered `Sales` table

### Requirement: Expression interpolation
`{{ <OMX expression> }}` in Markdown prose SHALL be replaced on render by the
formatted value of the expression, which SHALL be able to refer to tables and
constants from the document's `omx` blocks and from linked `.omx` files.

#### Scenario: Computed value in prose
- **GIVEN** a document defining `Sales` with `Revenue` values `100`, `120`, `150`
- **WHEN** the text `Total revenue: {{ Sales.Revenue.sum() }}` is rendered
- **THEN** it reads `Total revenue: 370`

#### Scenario: Error in interpolation
- **WHEN** an interpolation fails to type-check
- **THEN** rendering fails with a diagnostic pointing at the interpolation's location in the Markdown file

### Requirement: Linking a sheet file
A Markdown document SHALL be able to name one or more `.omx` files whose tables
and constants its interpolations may reference.

> The linking syntax is not settled (`!sheet Sales.omx` and
> `{{ include("Sales.omx") }}` were both floated); the requirement is the
> capability.

#### Scenario: Report over a separate sheet
- **GIVEN** `Sales.omx` defining `const TotalRevenue = Sales.Revenue.sum()` and `report.md` linking it
- **WHEN** `omasheet render report.md` is run
- **THEN** `{{ TotalRevenue }}` in the report is replaced by the computed total

### Requirement: Render output
`omasheet render` SHALL produce at least HTML and plain terminal output.

#### Scenario: HTML output
- **WHEN** `omasheet render report.md --html` is run
- **THEN** an HTML document is produced with interpolations and `omx` blocks evaluated
