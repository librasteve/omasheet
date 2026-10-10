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
- **THEN** rendering fails with a diagnostic pointing at the interpolation's location in the Markdown file, and nothing is written

#### Scenario: Several values
- **WHEN** an interpolation's value is the vector of `"Feb"` and `"Mar"`
- **THEN** it reads `Feb, Mar`

#### Scenario: A table
- **WHEN** an interpolation's value is a table
- **THEN** it is replaced by a rendered table, set apart from the text around it

#### Scenario: In code
- **WHEN** `{{ 1 + 1 }}` is written in a code span or a code block that is not `omx`
- **THEN** it is left as it is

### Requirement: Linking a sheet file
A Markdown document SHALL be able to name one or more `.omx` files whose tables
and constants its interpolations and `omx` blocks may reference. It names them
in YAML front matter, under `sheets`, as one path or a list of paths, each taken
from the directory the document is in. The files and the document's `omx`
blocks SHALL be one sheet: a name defined twice among them is an error, and a
`zone` line in any of them is the time zone of them all.

#### Scenario: Report over a separate sheet
- **GIVEN** `Sales.omx` defining `const TotalRevenue = Sales.Revenue.sum()` and `report.md` beginning with front matter `sheets: [Sales.omx]`
- **WHEN** `omasheet render report.md` is run
- **THEN** `{{ TotalRevenue }}` in the report is replaced by the computed total

#### Scenario: A sheet that cannot be read
- **WHEN** the front matter names a file that does not exist
- **THEN** rendering fails with a diagnostic pointing at the name in the front matter

### Requirement: Render output
`omasheet render` SHALL produce at least HTML and plain terminal output. The
terminal output SHALL be the document as Markdown, with each interpolation
replaced by its value and each `omx` block by its tables as Markdown tables.
The HTML output SHALL be a whole page.

#### Scenario: HTML output
- **WHEN** `omasheet render report.md --html` is run
- **THEN** an HTML document is produced with interpolations and `omx` blocks evaluated
