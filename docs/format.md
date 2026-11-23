# The file format

An `.omx` file is plain text: constants, functions, sheets and plots, in any order.
A line that starts with `//` is a comment. It is plain text to Markdown, where a `#` line would be a heading.

```omx
// Optional: the time zone of the file.
zone Europe/London

// A named value, and a function.
const TaxRate = 20%
fn WithTax(x) = x * (1 + TaxRate)

sheet Sales

// Optional: the type of a column.
Revenue : Decimal

// A cell that starts with = is a formula.
| Month | Revenue | Cost         | Profit | Gross |
| ----- | ------- | ------------ | ------ | ----- |
| Jan   | 10000   | 6000         | *      | *     |
| Feb   | 12000   | = 7000 + 500 | *      | *     |

// A computed column: one formula, every row. The header row names it where
// it goes, and its cells are *.
Profit := Revenue - Cost
Gross  := WithTax(Revenue)
```

## Sheets

- `sheet Name`, then a header row and data rows, written as a Markdown
  table: each line between outer pipes, `| a | b |`, cells separated by `|`,
  and a `| --- | --- |` line under the header row. A sheet is then a table
  wherever the file is shown as Markdown, a sheet of one column too.
- The outer pipes and the `---` line may be left out, as in older files:
  `a | b`. A header row that starts with `|` says the sheet has outer pipes.
  Without them, a `|` at the start or end of a row is an empty cell.
- The app writes every sheet it changes as a Markdown table, lined up.
- `Name := expression` after the rows is the formula of a computed column.
  The header row names the column, anywhere among the others, and each of
  its cells is `*`.
- A computed column may be written as a
  [sequence](expressions.md#sequences-with-), `Odd := 1, 3 ... Inf`, which
  fills it from the terms given.
- An empty cell is `Null`, and shown as nothing. Arithmetic with it gives
  `Null`, comparison with it is neither true nor false, and an aggregate
  passes over it rather than counting it as zero.

```omx
sheet Sales

| Month | Revenue | Profit | Cost | Tax |
| ----- | ------- | ------ | ---- | --- |
| Jan   | 10000   | *      | 6000 | *   |
| Feb   | 12000   | *      | 7000 | *   |

Profit := Revenue - Cost
Tax    := Profit * 20%
```

`Profit` is the third column and `Tax` the fifth. A formula with no column
of its name in the header row is an error, and so is anything but `*` in a
cell of a computed column.

### Styling

A sheet is styled in Markdown, written once for a whole column or row.

- The colons of the `---` line align a column: `:---` left, `:---:` centre,
  `---:` right. With none, numbers are set right and anything else left.
- A header cell written as Markdown styles its column: `**Qty**` is bold and
  `_Qty_` italic for the whole column, and `## Qty` is a heading of that level
  (`#` to `######`), which sets the header cell itself larger and, see below,
  declares a block of columns. They combine, `## **_Qty_**`.
- The leftmost cell of a row written the same way styles the whole row:
  `**Total**` makes every cell of the row bold, and `## Total` sets the
  leftmost cell larger and declares a block of rows.
- The markers are not part of the name or the value. `**Total**` is the
  label `Total`, so `Orders[; Item == 'Total']` still finds it, and the
  column is `Qty` to a formula. Bold and italic from a row and its
  column add up. A heading level is for the header cell and the leftmost cell
  alone, so the two never meet; the alignment is always the column's.
- Only a header cell and the leftmost cell of a row carry a style. Bare text
  anywhere else is Markdown for display, as before, and its value keeps its
  markers. To keep markers in a header or leftmost cell as they are, quote it:
  `"**Total**"`.
- The app shows the styles in the grid, and sets them with Ctrl+B, Ctrl+I,
  Ctrl+Alt+1 to 6 and Ctrl+Alt+L, E and R on whole columns or rows (alignment
  on any cells' columns), writing the markers for you. A rendered report
  sets each column's alignment, and bold and italic in its cells; a heading
  level is bold there, as a table has no headings.

```omx
sheet Orders

| **Item**     | _Qty_ | ## Price | Net |
| :----------- | :---: | -------: | --- |
| Tea          | 3     | 4.50     | *   |
| ## **Total** | 11    | 20.75    | *   |

Net := Qty * Price
```

### Blocks

A heading level also declares a block, the same for rows and columns. A row whose leftmost cell has a level starts a block that runs down to just before the next row with the same level or a higher one (a smaller number), and a header cell with a level starts a block of columns the same way to the right. Lines under it with a deeper level are blocks inside it. A heading with nothing under it is only a heading.

```omx
sheet Sales

| Region     | ## Y2025 | ### Q1 | ### Q2 | ## Y2026 |
| :--------- | -------: | -----: | -----: | -------: |
| ## EMEA    | 30       | 10     | 20     | 40       |
| ### UK     | 10       | 4      | 6      | 15       |
| ### France | 20       | 6      | 14     | 25       |
| ## APAC    | 5        | 2      | 3      | 8        |
```

- In the app a block shows a caret on its head, which folds the lines under it away. A row's caret stands in the row numbers and points down when the block is open and right when it is closed; a column's stands in its header and points right when open and down when closed. Both are stepped in by how many blocks they are inside, and Alt-click folds a block and the blocks inside it. The corner has a line of numbered buttons for the columns and another for the rows, each showing that dimension down to a depth on its own; 0 closes every block, 1 opens the top level, and the last number opens every block.
- The style bar reads, left to right, ↶ ↷ (undo and redo, style changes included), then B, I, H (heading level), Σ (summary), alignment, width and wrap, which are stored in the file, then ❄, ⊘ and ✕, which are not. ✕ clears the selection's styles and column width after asking. B, I, H and Σ need whole rows or columns selected; on a block of cells they say so in the message bar.
- ❄ freezes the rows above and the columns to the left of the selected cell, so that they stay in view as the rest scrolls; pressed again it thaws them. Like hiding, it belongs to the view and is not kept in the file.
- The style bar's ⊘ menu hides the selected columns, or rows when whole rows are selected, and a double line marks where something is hidden. Each hide is a step: Show last undoes the latest, Show all brings everything back, and Hide again puts back what Show all took off. Hide empty hides in one step every row with nothing in it but its leftmost cell, and every column with nothing in it, which takes away the empty head column of a block; a closed block whose head is hidden shows its own summary in its place. Like folding, hiding is the view's alone and is not kept in the file. The ⇔ menu sets the selected columns' width, 4 to 60 characters, or auto to fit the cells, and that is kept in the file as the length of the column's `---` line: a rule wider or narrower than the cells need is a width, and one as wide as they need (as a tidy table has) is auto. A rule shorter than four characters is never a width. The ⇢ menu next to it sets what a long text does at its column's edge, ⇢ spill (the default), ⇥ clip or ↵ wrap, for the selected text columns, and writes it as the column's `String[clip]` or `String[wrap]`; a row with wrapped text is as tall as its longest one needs. A sheet read from a file, or of more than 3000 rows, does not wrap: its long text stays on one line. In an `.xlsx` export a wrapped column is a workbook's wrapped text. A column inside a block of columns with nothing in it is hidden by itself; a head and a summary column stay.
- Folding only hides. Hidden cells still calculate, and a formula over them includes them. Which blocks are folded is not kept in the file.
- A block of rows or columns is drawn in the app as a box: the outermost in a full line and the block inside it in a fainter, stepped-in one, and never more than two levels. A rendered page marks a block's edge with a slightly fatter gridline.
- A sheet with blocks is not sorted or filtered, and the controls for them are muted.
- A heading level sets only the header cell, or the leftmost cell, larger; the head row's other cells are ordinary data.

#### Summary rows and columns

A total goes at the foot of its block, in a summary row: a row whose leftmost cell starts with `^ `. A header cell that starts with `^ ` is a summary column, at the right end of its block. A summary line belongs to the block it is inside, so one block may be closed by several: a sum, a mean, a maximum.

```omx
sheet Costs

| Place            | Net                   |
| ---------------- | --------------------- |
| ## EMEA          |                       |
| ### UK           |                       |
| London           | 10                    |
| Leeds            | 5                     |
| ^ ### UK sum     | = [Net; block].sum()  |
| ^ ### UK mean    | = [Net; block].mean() |
| ### France       |                       |
| Paris            | 7                     |
| ^ ### France sum | = [Net; block].sum()  |
| ^ ## EMEA sum    | = [Net; block].sum()  |
```

- A summary line is never a head and never ends a block, whatever level it has. Without a level it belongs to the innermost block around it. With one, `^ ## EMEA sum`, it belongs to the nearest block headed by that level, which is how `EMEA sum` closes EMEA and not France, where both end together. The level also sets the line larger, as a heading does. The marker comes first and combines with the others: `^ ## **Total**`.
- Folding a block hides everything under its head, its summary lines too, and the head shows the values of its first summary line, in italics, so a closed sheet reads as its totals. A summary line of a block around it stays in view. The app's style bar has a Σ button that marks the selected rows or columns as summary lines.
- `[Net; block]` in a summary row is the lines the block sums: those with no heading level and not themselves summary lines, so nothing is counted twice.

## Views: a sheet whose rows are in Parquet or CSV

A sheet may read its rows from a Parquet or CSV file instead of holding them:

```omx
view Sales := dataframe['data/sales.parquet']

Month   : String
Revenue : Integer
Cost    : Integer

Profit := Revenue - Cost
```

`view` takes the place of `sheet` and closes the sheet before it. The
string names one Parquet or CSV file, read from beside the `.omx` file. No
row is written in the file, and the row count comes from the Parquet.

A CSV is named the same way, `dataframe['data/sales.csv']`, and is read
through a Parquet copy kept in the cache, so everything below holds for it
too. Its column types are not written in the file, so they are worked out
from its first 10,000 rows: a schema line must say what was found. A column
of plain numbers with a point, `1.5`, `2.25` and `4`, is `Decimal`, read
exactly, with as many places as the most any value has (up to 18); a column
of whole numbers is `Integer`. A column with an exponent (`1.5e3`), a `NaN`
or `Inf`, or more than 18 places is `Float`. Parquet carries its types, and
is read as they are. A Parquet column stored as `Float` (a price, say) can
be read as exact decimals all the same: give its schema line `Decimal`, as
in `Price : Decimal`, and it is converted as it is read.

A column whose name in the file is not a name a formula could write — one
with a space or a slash in it, which Parquet allows and omasheet does not —
is given a name here with a `from` clause:

```omx
SiblingsSpousesAboard : Integer from 'Siblings/Spouses Aboard'
```

The quoted name is the file's, used to find the column and to check it; the
name on the left is what formulas use. Form it from the file's own, dropping
whatever a name has no room for, rather than shortening it: `Siblings` would
say something this column does not. A column the file holds and no schema
line names is reported with such a name in the help. Nothing in the file is
renamed, and a sheet that holds its own rows has no use for a `from`, so one
there is reported.

The schema lines are the columns, in the order they are written, and they
say what the file is expected to hold: a column the file holds under another
type, or holds and the schema does not mention, is reported rather than
quietly taken from either side. A view with no schema lines at all takes
its columns from the file, which is how a source is read before its schema
has been written down.

Otherwise the sheet is a sheet: its columns are selected and indexed as any
other, a `:=` column over them calculates for every row, and an aggregate
reads the whole column. What it cannot do is hold an edited cell — there is
no text behind a cell whose row lives in Parquet — so the app shows such a
sheet without editing it, and says so if you try: the way to edit the data
is `omasheet import sales.parquet`, which makes an `.omx` holding the rows
itself.

`examples/analysis.omx` is a working one, over `examples/titanic.parquet`.

## Sheets calculated from an expression

`sheet Name := expression` makes a sheet of another sheet and the stages
over it ([Sheets from sheets](expressions.md#sheets-from-sheets)):

```omx
sheet Dearest  := Passengers |> sort(Fare, desc) |> select(Name, Fare) |> head(5)
sheet ByClass  := Passengers |> group_by(Pclass) |> agg(Fare.mean(), Aboard := Name.len())
sheet Against  := Sales |> join(Goals, Month, left)
sheet Takings  := Sales |> group_by(Region) |> apply(Takings)
```

It has no rows in the text and no header row: its columns and its rows are
what the expression gives, worked out whenever the file is. A filtered,
sorted or selected sheet is a view of its source, each column the source's
own, with its name and its type. A grouped sheet has a row for each group,
its key columns first and an aggregate after them, and a joined sheet a row
for each pair of rows matched, the left sheet's columns and then the other's.

All of them are read-only, as a view is: nothing is ever written back to
the sheet they come from, and a cell of one cannot be edited. Its formula
is its text, so that is what is changed — in the app, by the box at the top.
A sort in the app's grid of a sheet made by a filter, a sort or a
selection is written into its formula, as a `|> sort(…)` at the end; a
grouped or joined sheet is sorted in the grid alone, as a view is, its
formula ending in a stage nothing can come after.

Exporting one writes its rows as values; importing them back gives a sheet
of those values, since nothing in a worksheet says how they were made.

## Sheets of formulas alone

A sheet written as `:=` lines with no table of its own has no cells to
write:

```omx
sheet PerHead

Class       := Passengers[Pclass; *]
Fare        := Passengers[Fare; *].Decimal.round(2)
GroupSize   := Passengers[Family; *] + 1
FarePerHead := (Fare / GroupSize).round(2)
```

Its columns are those lines, in the order they are written, and there is no
header row and no `*` to write for a cell. Its length is the length of the
sheets its formulas pick a row of with `*`: `PerHead` has a row for every
row of `Passengers`, whether those rows are in the text or in a Parquet
file. Where several are read, the longest says how many rows there are, and
a row past the end of a shorter one is empty, as any row outside a sheet is.

An aggregate reads a whole column rather than a row, so a sheet of
aggregates is one row long:

```omx
sheet Summary

Aboard   := Passengers.Survived.count()
Lived    := Passengers.Survived.sum()
MeanFare := Passengers.Fare.mean().round(2)
```

Otherwise the sheet is a sheet: its columns are selected and indexed as any
other, and an aggregate reads the whole column. What it cannot hold is a
written cell: there is no text behind one, and no row of its own to add or
delete, so the app refuses those, and a data column with them. Its formulas
are its text, so those are edited as any sheet's — typing into one of its
cells changes that column's formula, a column may be renamed, and another
`:=` column added after it.

Exporting one writes its rows like any other sheet's; importing them back
gives the header row and the `*` cells, since nothing in a worksheet says a
sheet's rows were derived.

## Types

`Integer`, `Hex`, `Octal`, `Binary`, `Decimal`, `Fraction`, `Percent`,
`Float`, `String`, `Boolean`, `Date`, `Time`, `Datetime`, `Duration`, `Url`,
`Email`, `Uuid`.
A column with no `Name : Type` line takes its type from its cells.
The cells never set a column's type: only a `Name : Type` line, or setting the type in the app, does. An undeclared column's type is the join of the cells it holds, worked out as they are entered: the first gives it, and each later one widens it if it needs to. Each cell is shown in the format its own contents give it, so `1`, `1.1`, `1/2`, `23%`, `1e5` and text may stand together and none is changed by what is entered beside it. Setting a type is different: it applies one common format, and a compatible value is shown in it, so `15` entered in a `Binary` column is `0b1111`, a way to convert a base on the fly. The file keeps each cell as it was entered.
`Integer` was called `Int` before, and a column declared that way is still
read as one; `Integer` is what is written.

| Type | Written |
|---|---|
| `Integer` | `42`, `1_000_000` |
| `Hex` | `0xff`, `0xFF_FF` — an `Integer` shown in base 16 |
| `Octal` | `0o17` — an `Integer` shown in base 8 |
| `Binary` | `0b101` — an `Integer` shown in base 2 |
| `Decimal` | `19.99`, `0.125` — exact to the places its column keeps: `Decimal[2]` keeps two, a bare `Decimal` ten (`Decimal(38, 10)`); a column of fewer than ten places shows them as `.2f` does, unless a format says otherwise, and the format follows the places after a `;`, `Decimal[2; .4f]`. A column that declares nothing keeps the least that holds its cells, and more than eighteen places make a `Fraction` |
| `Fraction` | `1/7`, `2 3/4`, `0.125` — the exact rational, of integers of any size, shown as a fraction. It is never rounded and never lowered |
| `Percent` | `20%`, `12.5%` — exact, shown as a percentage, to two places unless it has a format: a `Decimal`'s, as `Percent[.1f]`, which shows `41.7%` for `41.667%` |
| `Float` | `1.5e3`, `Inf`, `-Inf`, `NaN` — floating point |
| `Scientific` | `10.5(3)`, `10 ± 0.3` — a value with its measurement uncertainty, each a `Float`; a number with no uncertainty in a `Scientific` column has an uncertainty of zero. Its format, in brackets, is a number format as for a `Float`: with nothing before it the value is shown as `10.5(3)`, with `±` before it as `value ± uncertainty`, and with `~` before it as the value alone, at the places the uncertainty gives: `Length : Scientific[.4f]`, `Scientific[±.3e-3]`, `Scientific[~.2f]`. The conversion `s` lets the uncertainty lead: a value with one is shown by it, whatever the precision, and a value with none is shown to the precision as significant digits, in `g` form: `Scientific[.9s]` shows `1.0545718176e-34` as `1.05457182e-34` and `6.674 30(15)e-11` as it is |
| `Quantity` and its kinds | `5kg`, `300cm`, `9.81m.s^-2`, `10.5(3)e-3kg`, `20°C` — a number and an SI unit, written with no space; a `Scientific` magnitude, so it carries an uncertainty. A column is declared `Quantity` for any dimension, or one of its kinds (`Length`, `Mass`, `Force`, `Energy`, `Pressure`, ...), which admits only a unit of that dimension: `Load : Mass`. `Angle` holds `rad`, `°` or `45°30′`; `SolidAngle`, `AngularSpeed`, `LuminousFlux` and `Illuminance` hold `sr`, `rad.s^-1`, `lm` and `lx`. `Data`, `DataRate` and `DataDensity` hold `B`, `GiB`, `Mb`, `MiB.s^-1` and `Gb.m^-2`. A cell in another unit of it is converted to the column's unit, which is the preferred unit of the kind (`kg` for `Mass`) or the one in brackets, `Load : Mass[g]`; a bare number is read in that unit. The column's head says its unit once, `Mass[kg]`, in full whatever the width it takes, and its cells are numbers: `5`, `0.3`, `7.50(20)e-3`. After a `;` a number format as a `Scientific` takes may follow: `Mass[mg; .1f]`. A `v` before the conversion, `Mass[g; v.1f]`, puts the unit down in each row too, `5000.0g`; the head still says it, `Mass[g]`. A `*` before a metric unit, `Length[*m]`, shows each value with the prefix that gives it the shortest form, a power of a thousand, in each row: `1.5km`, `2mm`; the head holds no unit. It takes the unit without a prefix (`*g`, not `*kg`), and a bare `*` of a kind takes its preferred unit. The bit and the byte take the binary prefixes, powers of 1024, `Data[*B]`: `1536B` is `1.5KiB`. The unit may be left out before the `;`, `Quantity[; .10s]`, and each value is then shown in its own unit, as it has no one unit for a head, and `v` is an error there; with the conversion `s`, a derived value with no uncertainty is shown in ten significant digits, as `1.054571818e-34J.s`, rather than as `0J.s·`. A `_` before the conversion, `Quantity[; _.10s]`, sets the unit apart from its number by a space, `6.022e+23 mol^-1`; with a unit in the head it is written `v_`, `Mass[g; v_.1f]`; a number shown cut has its `·` after the unit. A unit is shown as the SI recommends, `m⋅s⁻¹` and `kg⋅m²`, in the head and in each row, and is read in that form as well as `m.s^-1`; a `^` after the `v` or `_`, `Speed[m.s^-1; ^]`, shows it as it is written in a file instead. A table printed or put in Markdown names the unit after the column, `Load [kg]`, and a value in a sentence keeps its unit. See *Units* in the expressions guide |
| `Complex` | `3+4i`, `-1.5-2i`, `2i` — a real and an imaginary part, each a `Float`; a real number in a `Complex` column is that number with no imaginary part |
| `String` | `hi` or `"hi"`  (escape \|). Its format, in brackets, says what a cell wider than its column does: nothing said, the text spills over the empty cells to its right; `String[clip]` cuts it off at the column's edge; `String[wrap]` carries it on to more lines inside the column. `Url` and `Email` take them too |
| `Boolean` | `true`, `false`; a `Boolean` column also reads `1` and `0`, `x` and `o`, `X` and `O`. Its format, in brackets, is any two different characters, the symbol for true and then for false: `Boolean[10]`, `Boolean[xo]`, `Boolean[XO]`, `Boolean[✓✗]` (not a space, `]` or `%`), and the column reads its own two as well; with none it shows `true` and `false` |
| `Date` | `2025-01-31` |
| `Time` | `09:30`, `09:30:15`, `09:30:15.5` |
| `Datetime` | `2025-01-31T09:30`, `2025-01-31T09:30:15.123456` |
| `Duration` | `PT1H30M`, `P1DT6H`, `-PT30M` (ISO 8601), `139s` (a quantity of time) or `+11:30`, `-0:00:30.5`, `+128:04:15` (a sign, then hours, minutes and perhaps seconds) — a kind of `Quantity` for time. Its head shows the unit, `Duration[s]`, and the cells are numbers, `5400`; `Duration[h; .2f]` shows hours, `Duration[iso]` shows `PT1H30M` and `Duration[%H:%M:%S]` shows `+01:30:00`, with hours past 24 |
| `Url` | `https://example.com`, `mailto:name@example.com` |
| `Email` | `name@example.com` |
| `Uuid` | `7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f` |

A division by zero is `Inf`, `-Inf` or `NaN`, which are `Float`s. A column
left to its cells is as wide as they are, so one that holds such a value is a
`Float` column, and a Parquet export writes it as one, with every value kept.
A column declared `Decimal`, `Fraction` or `Percent` cannot hold one: its cell
is an error that says to declare the column `Float`, leave out its type, or
guard the division.

A `_` may stand anywhere among a number's digits, before or after the
point, to group them: `1_000_000`, `10_00_000`, `0.000_1`, `0xFF_FF`. It
may not stand in an exponent.

`Hex`, `Octal` and `Binary` hold what an `Integer` holds and show it in
their own base, after a lower-case `0x`, `0o` or `0b`; the prefix and the
hex digits are read in either case. A leading `0` alone is decimal: `017` is
17. A negative one is shown as its 64-bit two's complement, so `-255` in a
`Hex` column is shown `0xffffffffffffff01`, and is written in the file as
`-0xff`, which reads back as the same number. Each goes where an `Integer`
does and takes an `Integer` where it is declared. Arithmetic on two of one
base gives that base, but for `/`, and on any other mix gives an `Integer`:
`0xF0 + 0x0F` is `0xff` and `0xF0 + 1` is `241`. A format on the column
shows the number by its conversion instead: `Hex[08X]` shows 255 as
`000000FF`. `Hex(x)`, `Octal(x)` and `Binary(x)` convert as `Integer(x)`
does.

A `Time` and a `Datetime` may carry a fraction of a second after the
seconds: `09:30:15.5`, `2025-01-31T09:30:15.123456`. The fraction always
goes with the seconds, since `09:30.5` would say which of minutes or seconds
it belonged to only by convention. A `Time` is held in nanoseconds and takes
up to nine digits; a `Datetime` is held in microseconds and takes six, and a
seventh is reported rather than quietly dropped. What is shown is the digits
held and no more, so `09:30:15.5` comes back as it was written.
`millisecond`, `microsecond` and `nanosecond` give what is left after the
second.

A `Duration` is a length of time and a kind of `Quantity`, of the dimension of time, with the unit `s`. It is written three ways: in ISO 8601, `P`, then weeks and days, then `T` and hours, minutes and seconds, each number before its letter; as a quantity of time, `139s`, `90min`, `2h`, `1.5d`; or with a sign and a colon, `+hh:mm:ss.x`, where the sign is what makes it a duration and not a time of day, `+11:30` is 11 hours 30 minutes and `-0:00:30.5` is minus half a minute and a half; the hours may pass 24, `+128:04:15`, but minutes and seconds are below 60. `min`, `h`, `d` and `wk` are units of 60, 3600, 86400 and 604800 seconds. In ISO only those units are taken; `P1Y` and `P1M` are refused, since a year and a month are not a fixed length. Only the seconds take a fraction, to six digits. It is held in microseconds, as a Polars `Duration`, whatever unit the column shows it in. A value may have an uncertainty, `10.5(3)s`, which a whole number of microseconds cannot hold, so it is held as a quantity of seconds and the column is calculated by the engine rather than Polars; it cannot move a date or a time, which are exact. A Parquet file holds a `Duration` as a Parquet duration, and one with an uncertainty as the numbers of its unit with a `sigma`, as a quantity is; a bare number in a `Duration` column is read in its unit. A bare `Duration` column is `Duration[s]`: its head says `Duration[s]` and its cells are numbers of seconds. `Duration[h; .2f]` shows hours to two places, `Duration[iso]` shows `P1DT10H5M30S` (days and the time after them; none is `PT0S`) and `Duration[%H:%M:%S]` shows `+34:05:30`; each reads back as the same duration. A name shaped as an ISO duration, such as `P1D`, is read as one.

A cell's written form carries its type where that form is distinctive:
`true`, `2025-01-31`, `09:30`, `20%` and `1.5e3` read as a `Boolean`, a
`Date`, a `Time`, a `Percent` and a `Float` wherever they stand. A
`Name : Type` line is what makes the type the whole column's, and is needed
where the written values would not carry it: a `Float` column of `22` and
`7.25`, which read as an `Integer` and a `Decimal` on their own, or a `Fraction`
or `Percent` column over numbers written plainly. Nothing is lost by leaving
a line out where the cells already say it — the app shows a type
[muted](app.md#the-app) where it comes from the cells and plain where it is
declared.

Text written bare may hold Markdown: `a **slow** start`,
`see the [lease](lease.pdf)`. The app shows it as that, and so does a
[rendered document](interop.md#markdown). Text in quotes is as it is:
`"2 * 3 * 4"`. So is the text a formula makes, as it is written in single
quotes there (a cell's text is in double quotes and code's in single); a value read from a cell is Markdown or not as that cell is.
`omasheet print` shows every cell as it is written.

A `Url`, an `Email` and a `Uuid` are text read as something narrower. A
column of one keeps the text that was written and takes only text that reads
as one, so a cell that is not an address says so where it is. No cell carries
one of these by its written form, so a column is one only where the line says
it: text that came in from another file stays text until it is declared, or
read as one by name — `Contact.Email`, which marks the cells that do not read
as one and leaves the rest of the file calculating.

What reads as one is kept plain. A `Url` needs a scheme and then the rest of
the address, so `https://example.com` and `mailto:name@example.com` are ones
and bare `example.com` is not. An `Email` needs one `@`, a name before it,
and a domain carrying a dot after it; nothing else about the name is read
into, so `a.b+tag@example.co.uk` is one. A `Uuid` is the thirty-six character
form, `7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f`, in either case. None of them
may hold a space. Only `.omx` keeps which one a column is: through CSV, an
XLSX Workbook, JSON or Parquet the text goes out and comes back as text.

A `Decimal` and a `Percent` hold the same exact numbers, and a column of one
takes the other, showing what it holds its own way. A `Fraction` column
takes any exact number, and a `Decimal` column given a `Fraction` rounds it
to its places: `<1/7>` is `1/7` in a `Fraction` column, `0.1428571429` in a
`Decimal` one.

The numeric types form a hierarchy, **ℤ ⊂ ℚ ⊂ ℝ ⊂ ℂ**. The more _infectious_ a type, the less exact it is: `Integer` < `Fraction` < `Decimal` < `Float` < `Scientific` < `Complex`. A `Fraction` is exact without limit, whereas a `Decimal` is held to a fixed number of places:

- **`Integer` (ℤ)**: whole numbers (i64)
- **`Fraction` (ℚ)**: exact rational (num/den)
- **`Decimal` (ℚ)**: exact to s places (`Decimal(38, s[=10])`)
- **`Float` (ℝ)**: real numbers (f64)
- **`Scientific` (ℝ)**: real numbers with uncertainty (2x f64)
- **`Complex` (ℂ)**: complex numbers (2x f64)

For any operation, and for the cells of a column, the result is the **more infectious** of the types it is made from, the least number system that holds them both: `Integer` + `Decimal` is a `Decimal`, `Decimal` * `Float` is a `Float`, and `Float` + `Complex` is a `Complex`. This is a model, not the machine's: an integer is an `i64`, a `Decimal` a 38-digit fixed point with the places its column keeps, a `Fraction` exact, and a `Float` an `f64`.

Each type may be shown more than one way:

| Type | Literal | Representations |
|---|---|---|
| `Integer` | `1` | `0xff`, `0o17`, `0b101` |
| `Fraction` | `<1/7>` | `1/7`, `2 3/4` |
| `Decimal` | `1.0` | `14.29%` |
| `Float` | `1e0` | - |
| `Scientific` | `1.01(01)e0` | `1 ± 0.01` |
| `Complex` | `3+4i` | - |

- Two values shown the same way keep it: `Hex` + `Hex` is a `Hex`, and `Fraction` + `Fraction` a `Fraction`.
- Two different ways of showing one type give the plain type: `Hex` + `Binary` is an `Integer`.
- Different types give the more infectious, shown plain: `Decimal[2]` * `Decimal[4]` is a `Decimal[4]`, and `Fraction` * `Integer` is a `Fraction`, but `Fraction` + `Percent` or `Fraction` * `Decimal` is a `Decimal`.
- A power with a whole exponent is exact; with any other, such as `2 ** 0.5`, it is a `Float`, as are the trigonometric, exponential and logarithmic functions.
- A number beside text gives `Any`.

The joins, written out:

| | `1` | `1/1` | `1.0` | `1e0` | `10.5(3)` | `3+4i` | text |
|---|---|---|---|---|---|---|---|
| `1` | `Integer` | `Fraction` | `Decimal` | `Float` | `Scientific` | `Complex` | any |
| `1/1` | `Fraction` | `Fraction` | `Decimal` | `Float` | `Scientific` | `Complex` | any |
| `1.0` | `Decimal` | `Decimal` | `Decimal` | `Float` | `Scientific` | `Complex` | any |
| `1e0` | `Float` | `Float` | `Float` | `Float` | `Scientific` | `Complex` | any |
| `10.5(3)` | `Scientific` | `Scientific` | `Scientific` | `Scientific` | `Scientific` | `Complex` | any |
| `3+4i` | `Complex` | `Complex` | `Complex` | `Complex` | `Complex` | `Complex` | any |
| text | any | any | any | any | any | any | `String` |

A `Scientific` beside a `Complex` is a `Complex`, but the uncertainty is dropped: there are no uncertain complex numbers, and the app says `uncertain complex not supported`.

A way of showing a type joins with a `Complex` as the type would: `0x10 + 1i`
is `16+1i`. A `Complex` has no order, so it cannot be compared with `<` or
sorted, but `=` and `!=` compare both parts exactly.

| | `0b1` | `0o1` | `0x1` | `1` |
|---|---|---|---|---|
| `0b1` | `Binary` | `Integer` | `Integer` | `Integer` |
| `0o1` | `Integer` | `Octal` | `Integer` | `Integer` |
| `0x1` | `Integer` | `Integer` | `Hex` | `Integer` |
| `1` | `Integer` | `Integer` | `Integer` | `Integer` |

| | `100%` | `1/1` | `1.0` |
|---|---|---|---|
| `100%` | `Percent` | `Decimal` | `Decimal` |
| `1/1` | `Decimal` | `Fraction` | `Decimal` |
| `1.0` | `Decimal` | `Decimal` | `Decimal` |

A cell in a column with a declared type is accepted when it converts to that
type, and is then shown in the type's own format: `0b1` in an `Integer` column
is shown `1`, and `3+0i` in a `Float` column is `3`. A `Complex` converts to a
real number only when its imaginary part is zero, so `3+4i` in a `Float` column is an error.

A column of numbers and text is of any type, and each cell is as it was
written. What a file keeps of a cell is the text that was entered: a number
as it was written, or the text of a formula.

A `Percent` column takes any exact number and shows it as a percentage: `0.4`
there is `40%`, and so is `= Profit / Revenue` when it comes to two fifths.
A `20%` in a `Decimal` column is the number it stands for, `0.2`.

A `Fraction` column takes any exact number and shows it as a fraction of
whole numbers, with a whole part where there is one: `0.125` there is `1/8`
and `2.75` is `2 3/4`. It holds the exact rational: a cell `1/7` is a
seventh, to every digit, and a seventh added to a seventh is `2/7`, exactly.
Only a fraction whose denominator is 10000 or less is shown as one; past that
the number is shown as a decimal, cut and marked. A whole number beside a
fraction gives a fraction, so a `:=` column of `Yield * 2` over a `Fraction`
column shows fractions.

In a cell, `1/7` and `22/7` are fractions, and so is `2 3/4` for `11/4`.
Neither part need be the smaller: `11/3` and `5 7/3` are fine, and a
`Fraction` column shows them as `3 2/3` and `7 1/3`. A column with no type
whose cells are all written with a `/` (or beside whole numbers) is a
`Fraction` column; with a decimal among them it is a `Decimal` one, a fraction
beside a decimal being a decimal. A column declared `Decimal` takes the same
cells and rounds them to its places.

In a formula a fraction is written in angle brackets, `<1/10>`, `<-3/4>` or
`<2 3/4>`, though a bare `1/10` there is a quotient of whole numbers, which is
the same `Fraction`. A quotient with a decimal in it, `1.0/10`, is a `Decimal`
rounded to ten places. A fraction beside a whole number is a fraction, so
`<1/10> + 1` is `11/10`; beside a decimal it is a decimal, so `<1/10> + 0.5` is `0.6`. A space goes before a fraction that follows a `+`,
since `+<` is a shift.

Without a format, a number is shown in a practical way whatever its type, and what is stored is not changed:

- A very large or very small number is shown in e notation, not as a long run of zeros. From `1e15` up, or below `1e-15` (other than zero), an `Integer`, a `Decimal`, a `Fraction` that is a whole number and a `Float` are shown so: `10**30` is `1e+30`, `0.0000000000000001` is `1e-16`, and `123456789012345678` is `1.23456789012345678e+17`. In between they are plain. A `Fraction` that is not whole stays a fraction, `1/3`, or a mixed number once it is large.
- A `Decimal` is shown to three places after the point, `12345.679·`, with a `·` after it where it stores more digits. A `Decimal` below 1 is shown to three significant digits, `0.00314·`. A number with no more digits than that is shown whole, and a number too long for 23 characters is cut and marked the same way. Pointing at a cut number in the app shows it in full, and `--exact` on the command line does too.
- A `Percent` is rounded to two places, `41.67%`.

A column format, below, replaces these rules for its column.

An `Integer`, `Decimal` or `Float` column may say how its numbers are shown,
after its type in brackets, as a C `printf` conversion without its `%`:

```omx
Price : Decimal['.2f]
Id    : Integer[#x]
```

A column with a format shows each number by it rather than by the usual
rules: `'.2f` shows `12345.678` as `12,345.68`, with as many digits as it
takes and none cut. The value is unchanged, and the file and the cell
editor still hold it in full. The point is your locale's, and `'` groups
the whole part by your locale's separator. The app, `omasheet print` and a
rendered document all show it so (a scale word after a `;`, as `Decimal[.1f; ppm]`, shows 0.000005 as `5.0ppm`; see `docs/expressions.md`): in a document, `{{ Sales.Price[0] }}`
and `{{ Sales.Price }}` are shown by the column's format, and
`{{ Sales.Price.sum() }}`, being worked out from it, is not. See
`examples/formats.omx`.

A formula may show a value in a format of its own with `.fmt()`, which gives text: `Price.fmt('.2f')` is `12345.68`, and over a column it gives a column of text, one for each value. The format is written as a column's is, in single quotes, and must suit the value's type: `'x'` of a `Decimal` is an error, where it is written if the type is known there. An empty cell gives an empty cell.

| Part | Means |
|---|---|
| `-` | on the left of the width |
| `+` | a sign on every number |
| space | a space where there is no sign |
| `0` | the width made up with zeros |
| `'` | the whole part grouped in threes |
| `#` | the point kept, and `0x`, `0` or `0b` before hex, octal or binary |
| a number | the width, at least |
| `.` and a number | places after the point; for `d`, `x`, `o`, `b` the fewest digits |

An `Integer` takes `d` or `i` (decimal), `x` or `X` (hex), `o` (octal) and
`b` or `B` (binary); a negative one in hex, octal or binary is shown as its
64-bit two's complement, as C shows it. A `Decimal` or `Float` takes `f`
(fixed places), `e` (e notation) and `g` (whichever is shorter), or the
same in capitals. A format that does not read, or that the column's type
does not take, is an error where it is written.

A `Boolean` column takes two characters (see `examples/booleans.omx`). It reads `1`, `0`, `x`, `o`, `X`, `O` and the format's own two as well as `true` and `false`, in a declared column only: a bare `1` elsewhere is an `Integer`. The value is unchanged, and the file keeps the cell as it was entered.

A `Date`, `Time` or `Datetime` column may say how it is shown too, after
its type in brackets, as a C `strftime` format, `%` and all:

```omx
Due   : Date[%a %-d %b %Y]
Start : Time[%-I:%M %p]
At    : Datetime[%d/%m/%Y %H:%M %Z]
```

`%a %-d %b %Y` shows `2025-01-31` as `Fri 31 Jan 2025`. As with numbers,
the value is unchanged: the file holds it in ISO form and the cell editor
in your locale's. Names of days and months, `%p`, and `%c`, `%x` and `%X`
are your locale's.

| Part | Means |
|---|---|
| `%Y` `%y` `%C` | the year, its last two digits, its century |
| `%m` `%b` `%B` | the month: `01`, `Jan`, `January` |
| `%d` `%e` `%j` | the day of the month, space-padded, of the year |
| `%a` `%A` `%u` `%w` | the day of the week: `Fri`, `Friday`, `5` from Monday, `5` from Sunday |
| `%G` `%g` `%V` `%U` `%W` | the ISO year and week, the week from Sunday, from Monday |
| `%H` `%I` `%k` `%l` `%p` `%P` | the hour of 24 and of 12, space-padded, `AM`, `am` |
| `%M` `%S` | the minute and second |
| `%f` `%3f` `%6f` `%9f` | the fraction of a second: every digit held, or that many |
| `%F` `%T` `%D` `%R` `%r` | `%Y-%m-%d`, `%H:%M:%S`, `%m/%d/%y`, `%H:%M`, `%I:%M:%S %p` |
| `%c` `%x` `%X` | the date and time, the date, the time, as your locale writes them |
| `%z` `%Z` `%s` | the offset, `+0100`, the zone, `BST`, and seconds since 1970 UTC |
| `%n` `%t` `%%` `%]` | a newline, a tab, `%` and `]` |

Between the `%` and its letter, `-` leaves a number unpadded (`%-d` is
`5`), `_` pads it with spaces, `0` with zeros, `^` puts a name in capitals,
and a number is the width, at least. A `Date` takes no time of day and a
`Time` no day; `%c`, `%z`, `%Z` and `%s` are a `Datetime`'s alone, in the
file's zone. A format that does not read, or that the column's type does
not take, is an error where it is written.

See `examples/types.omx` for every type at work, and `examples/formats.omx`
and `examples/datetimes.omx` for formats.

Dates and times are always written this way in the file. The app shows and
accepts them in your locale's form.

## Constants, functions, zone

- `const Keys = [Region, Month]` names a list of columns, to use again where a
  function takes columns: `Sales |> group_by(Keys)`, and the same in `select`,
  `sort` and `join`. The names are looked up in the sheet it is used on, so
  one list serves every sheet that has those columns, and it can sit beside
  plain names, as `group_by(Keys, Quarter)`. It is only for taking columns:
  it is not a value, and a list of constants or numbers is still an ordinary
  list.
- `const Name = expression`
- `fn Name(a, b) = expression` — sees its parameters, constants and sheets,
  not the row it is called from. The comment above it is its description.
- `zone Area/City` before the first sheet says which time zone the file's
  date-times are in. Without it, they are in the zone of the machine.
- `consts [; .10s]` before the first sheet is the number format the
  constants' values are shown by, as `Set format…` on the Constants tab sets
  it. Each constant that can take the format uses it. Without the line the
  format is `; _.10s`, ten significant digits and a space before the unit;
  `consts []` shows them with no format.
- `Logged : Datetime("Europe/London")` declares the zone of one column. Its
  cells are the wall clock of that zone, so what is typed in is taken there,
  and each is shown with its offset, which follows the zone's daylight
  saving: `2025-03-01T09:30+00:00`, `2025-07-01T09:30+01:00`. Comparison and
  subtraction are on the instant. Only a column of cells declares a zone, not
  one read from a file, and a zone the system does not know is an error.

## Plots

`plot Name` declares a plot of a sheet, with its controls as `key: value` lines up to the next blank line. A line that starts `//` is a comment.

```omx
plot Trend
type: line
data: Sales |> select(Month, Revenue, Cost)
title: Revenue and cost
```

| Control | Meaning |
|---|---|
| `type` | `line`, `bar`, `pie`, `candle`, `scatter`, `area`, `stacked`, `step`, `histogram`, `box`, `heatmap`, `waterfall`, `bubble` or `donut`. Required |
| `data` | An expression that gives a sheet: `Sales`, or `Sales \|> filter(Region == 'North') \|> select(Month, Revenue)`. On one line. Required |
| `x` | The column of labels. The first column of `data` if left out |
| `y` | The columns of values, `Revenue, Cost`. All the others if left out |
| `title` | A title above the plot |
| `width`, `height` | In pixels, 100 to 4000; 640 and 360 if left out |
| `bins` | For `histogram`: how many bins, 1 to 200. From the number of values if left out |
| `k` | For `candle`: how many standard deviations the body covers (1). The wick is twice as far |

- A plot has a name of its own: not that of a sheet, a constant or another plot. It may come before or after the sheet it reads.
- `data` gives a sheet: a sheet by name, or one piped through stages. A grouping or a join is the whole of a `sheet Name :=` formula, so plot that sheet by name.
- A **line** has a line for each `y` column and a key if there is more than one; an empty cell breaks the line. A **bar** plot groups the columns' bars. A **pie** takes one `y` column of values that are not negative, and shows each slice's share.
- A **candle** is for uncertainty. One `Scientific` column, `10.5(3)`, is drawn as a body of the value ± `k`σ and a wick of ± 2`k`σ, with a white tick at the value. Four columns are read as open, high, low and close.
- Line, scatter and bar plots draw an error bar for a value that has an uncertainty.
- Where every `x` is a number, a **line**, **step** or **area** is drawn at them, with a numeric axis, and a **scatter** or **bubble** plot needs them.
- A **scatter** is a point for each value of each `y` column. A **bubble** is a point for each row: its height is the first `y` column and its size the second.
- An **area** fills under its lines, a **step** holds each value until the next row, and **stacked** puts the columns' bars one on another.
- A **histogram** counts the values of each `y` column in bins that are the same for all of them, `bins` of them.
- A **box** is a box for each `y` column: the quartiles, a line at the median, whiskers to the furthest values within 1.5 times the range between the quartiles, and a circle for each beyond them.
- A **heatmap** is a cell for each value, a row for each row of `data` (named by `x`) and a column for each `y` column, as dark as the value is large, with a scale.
- A **waterfall** takes one `y` column of changes, draws each from where the last ended, and ends with a bar for the total. A **donut** is a pie with a hole.
- Numbers, percentages, quantities (in the column's unit) and durations (in seconds) are plotted; a column of text or dates in `y` is an error. Dates and text in `x` are labels, in the order of the rows.
- `omasheet lint` reports a control that is not one, a `type` that is not a kind, a `data` that is not a sheet, and an `x` or `y` that is not a column of it. `omasheet print` does not list plots.
- A report draws a plot ([Reports](reports.md#plots)) and the app shows the file's plots in a pane beside the sheet.
- A version of omasheet from before plots stops at a `plot` line, as at any word it does not know.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
