# The file format

An `.omx` file is plain text: constants, functions and sheets, in any order.
A line that starts with `#` is a comment.

```omx
# Optional: the time zone of the file.
zone Europe/London

# A named value, and a function.
const TaxRate = 20%
fn WithTax(x) = x * (1 + TaxRate)

sheet Sales

# Optional: the type of a column.
Revenue : Decimal

# A cell that starts with = is a formula.
| Month | Revenue | Cost         | Profit | Gross |
| ----- | ------- | ------------ | ------ | ----- |
| Jan   | 10000   | 6000         | *      | *     |
| Feb   | 12000   | = 7000 + 500 | *      | *     |

# A computed column: one formula, every row. The header row names it where
# it goes, and its cells are *.
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
| `Decimal` | `19.99`, `1/7`, `2 3/4` — exact |
| `Fraction` | `1/7`, `2 3/4`, `0.125` — exact, shown as a fraction |
| `Percent` | `20%`, `12.5%` — exact, shown as a percentage |
| `Float` | `1.5e3`, `Inf`, `-Inf`, `NaN` — floating point |
| `Scientific` | `10.5(3)`, `10 ± 0.3`, `10 +- 0.3` — a value with its measurement uncertainty, each a `Float`; a number with no uncertainty in a `Scientific` column has an uncertainty of zero. Declared `Scientific[±]`, a column is shown as `value ± uncertainty` |
| `Complex` | `3+4i`, `-1.5-2i`, `2i` — a real and an imaginary part, each a `Float`; a real number in a `Complex` column is that number with no imaginary part |
| `String` | `hi` or `"hi"`  (escape \|) |
| `Boolean` | `true`, `false` |
| `Date` | `2025-01-31` |
| `Time` | `09:30`, `09:30:15`, `09:30:15.5` |
| `Datetime` | `2025-01-31T09:30`, `2025-01-31T09:30:15.123456` |
| `Duration` | `PT30M`, `PT1H30M`, `P1DT6H`, `PT0.5S`, `P2W`, `-PT30M` — ISO 8601 |
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

A `Duration` is a length of time, written in ISO 8601: `P`, then weeks and
days, then `T` and hours, minutes and seconds, each number before its
letter. Only those units are taken; `P1Y` and `P1M` are refused, since a year
and a month are not a fixed length. Only the seconds take a fraction, to six
digits. It is held in microseconds and shown in days and the time after
them, `P1DT10H5M30S`, which reads back as the same duration; none is `PT0S`.
A name shaped as one, such as `P1D`, is read as a duration.

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

A `Decimal`, a `Fraction` and a `Percent` hold the same exact numbers, and a
column of one takes the others, showing what it holds its own way: `1/7` is
`0.14285714285714285714…` in a `Decimal` column, `1/7` in a `Fraction`
column, and `14.29%` in a `Percent` column.

The numeric types form a hierarchy of increasingly expressive number systems, **ℤ ⊂ ℚ ⊂ ℝ ⊂ ℂ**:

- **`Integer` (ℤ)**: whole numbers
- **`Decimal` (ℚ)**: exact numbers with a fractional part, held to 38 digits
- **`Float` (ℝ)**: real numbers, approximately represented
- **`Scientific`**: real numbers with a measurement uncertainty, between `Float` and `Complex`
- **`Complex` (ℂ)**: complex numbers

For any operation, and for the cells of a column, the result is the **more infectious** of the types it is made from, the least number system that holds them both: `Integer` + `Decimal` is a `Decimal`, `Decimal` * `Float` is a `Float`, and `Float` + `Complex` is a `Complex`. This is a model, not the machine's: an integer is an `i64`, a `Decimal` a 38-digit fixed point and a `Float` an `f64`, each of finite precision.

Each type may be shown more than one way:

| Type | Literal | Shown as |
|---|---|---|
| `Integer` | `1` | `Hex` `0xff`, `Octal` `0o17`, `Binary` `0b101` |
| `Decimal` | `1.0` | `Fraction` `1/7`, `2 3/4`; `Percent` `14.29%` |
| `Float` | `1e0` | none |
| `Scientific` | `10.5(3)` | `±` |
| `Complex` | `3+4i` | none |

- Two values shown the same way keep it: `Hex` + `Hex` is a `Hex`, and `Fraction` + `Fraction` a `Fraction`.
- Two different ways of showing one type give the plain type: `Hex` + `Binary` is an `Integer`, and `Fraction` + `Percent` a `Decimal`.
- Different types give the more infectious, shown plain: `Fraction` * `Integer` is a `Decimal`.
- A power with a whole exponent is exact; with any other, such as `2 ** 0.5`, it is a `Float`, as are the trigonometric, exponential and logarithmic functions.
- A number beside text gives `Any`.

The joins, written out:

| | `1` | `1.0` | `1e0` | `3+4i` | text |
|---|---|---|---|---|---|
| `1` | `Integer` | `Decimal` | `Float` | `Complex` | any |
| `1.0` | `Decimal` | `Decimal` | `Float` | `Complex` | any |
| `1e0` | `Float` | `Float` | `Float` | `Complex` | any |
| `3+4i` | `Complex` | `Complex` | `Complex` | `Complex` | any |
| text | any | any | any | any | `String` |

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
real number only when its imaginary part is zero, to within `1e-15`, as in
Raku, so `3+4i` in a `Float` column is an error.

A column of numbers and text is of any type, and each cell is as it was
written. What a file keeps of a cell is the text that was entered: a number
as it was written, or the text of a formula.

A `Percent` column takes any exact number and shows it as a percentage: `0.4`
there is `40%`, and so is `= Profit / Revenue` when it comes to two fifths.
A `20%` in a `Decimal` column is the number it stands for, `0.2`.

A `Fraction` column takes any exact number and shows it as a fraction of
whole numbers, with a whole part where there is one: `0.125` there is `1/8`
and `2.75` is `2 3/4`. The fraction is worked out from the number rather
than remembered from the cell, so one nobody wrote is found too — a seventh
added to a seventh shows as `2/7`. Only a fraction whose denominator is
10000 or less is looked for; past that the number is shown as a decimal.
Two fractions give a fraction, so a `:=` column of `Yield + Yield` over a
`Fraction` column shows fractions without being declared; a fraction with
any other number gives a `Decimal`, so `Yield * 2` shows `4.666…`.

A fraction of two whole numbers is an exact `Decimal`: `1/7` and `22/7` are
both read that way, and one may be written with a space, `2 3/4` for `11/4`.
Neither part need be the smaller: `11/3` and `5 7/3` are fine, and a
`Fraction` column shows them as `3 2/3` and `7 1/3`. A column with no type
whose cells are all written with a `/` is a `Fraction` column; with any other
number among them it is a `Decimal` one. A column declared `Decimal` takes the
same cells and shows them as decimals.

In a formula a fraction is written in angle brackets, `<1/10>`, `<-3/4>` or
`<2 3/4>`, since a bare `1/10` there is a quotient, a `Decimal`. A fraction
with a whole number is a `Decimal` too, so `<1/10> + 1` is `1.1`; to stay
a fraction add `<1/1>`, the fraction that is one, and `<1/10> + <1/1>` is
`1 1/10`. A space goes before a fraction that follows a `+`, since `+<` is a
shift.

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
rendered document all show it so: in a document, `{{ Sales.Price[0] }}`
and `{{ Sales.Price }}` are shown by the column's format, and
`{{ Sales.Price.sum() }}`, being worked out from it, is not. See
`examples/formats.omx`.

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
and `examples/dateformats.omx` for formats.

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
- `Logged : Datetime("Europe/London")` declares the zone of one column. Its
  cells are the wall clock of that zone, so what is typed in is taken there,
  and each is shown with its offset, which follows the zone's daylight
  saving: `2025-03-01T09:30+00:00`, `2025-07-01T09:30+01:00`. Comparison and
  subtraction are on the instant. Only a column of cells declares a zone, not
  one read from a file, and a zone the system does not know is an error.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
