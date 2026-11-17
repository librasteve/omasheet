# Omasheet 0.1.12: A plain-text spreadsheet with real units, exact numbers and native desktop installers

I've been working on Omasheet, a spreadsheet that takes a slightly different approach: **spreadsheets as plain-text, typed, version-controlled files.**

The native file format is `.omx`. It supports typed sheets and formulas, lives happily in Git, and has a Qt desktop application for working with your data.

I've just pushed five releases (0.1.8–0.1.12), and there's quite a lot to share.

### What's new?

**1. Units that actually understand dimensions**

You can now write expressions such as:

- `5kg + 300g` → `5.3kg`
- `9.81m.s^-2`
- `20°C`
- `45°30′30″`

SI prefixes and binary prefixes (including KiB) are supported. Adding incompatible dimensions produces an error rather than a meaningless result.

**2. Typed columns and uncertainty**

Columns can declare their types, units and display formats:

`Load : Mass[g; .1f]`

Values are converted into the column's unit automatically.

Uncertainty also propagates through calculations, so `5kg ± 100g` is more than just text in a cell.

**3. Exact numbers**

- `Fraction` provides arbitrary-precision rational arithmetic.
- `Decimal(n)` provides a defined decimal scale per column.

Useful when floating-point approximations aren't what you want.

**4. Polars integration**

Almost everything except `Fraction` can be lowered to Polars.

That includes window operations:

- `cum_sum`
- `diff`
- `rank`
- `rolling_*`

You can also partition calculations, for example:

`Amount.cum_sum(by Account)`

**5. More data types**

Booleans now have display formats such as ✓ and ✗, along with logic functions.

Complex numbers support `re`, `im`, `conj`, `arg` and `i`. For example:

`3 + 4 * i` → `3+4i`

**6. Scientific constants built in**

Every file has access to constants such as `pi`, `c`, `kB`, `NA` and `G`.

Exact constants follow the 2019 SI definitions, with other values based on CODATA 2022.

### The desktop app has improved too

- Type, unit and format pickers
- Live checking
- Whole-column copy and paste that preserves type, unit and format

### Now available on more platforms

Native installers/builds are available for:

- macOS (Apple silicon and Intel)
- Windows 64-bit
- Omarchy/Arch (x86_64 and aarch64)

On macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/librasteve/omasheet/main/install.sh | bash
```

On Windows, use `install.ps1` in PowerShell.

There's also a Claude Code skill for reading and writing `.omx` files.

### A couple of known issues

- Double-clicking an `.omx` file in Finder may not open it yet. Running `omasheet file.omx` works.
- macOS and Windows builds are currently not code-signed.

### Links

- GitHub: https://github.com/librasteve/omasheet
- SI units Rust crate: https://github.com/librasteve/siquant

I'm particularly interested in feedback from people working with scientific data, engineering calculations, financial models or anyone who likes the idea of spreadsheets that can be version-controlled and reviewed in Git.

**Question:** What would you want from a spreadsheet that treats units, types and numerical precision as first-class features rather than afterthoughts?

---

Omasheet 0.1.12: units, exact numbers, and macOS/Windows installers

Omasheet is a plain-text spreadsheet format (.omx) with typed sheets and formulas. It lives in Git and has a Qt grid app. Five
releases landed this week (0.1.8 to 0.1.12):

- Units in cells: 5kg + 300g gives 5.3kg, and 9.81m.s^-2, 20°C and 45°30′30″ all work. All SI prefixes and binary prefixes (KiB) are
supported. A mismatch of dimensions is an error.
- Typed columns: Load : Mass[g; .1f] converts cells to the column's unit. 5kg ± 100g carries uncertainty through arithmetic.
- Exact numbers: Fraction is an arbitrary-precision rational, and Decimal(n) has a per-column scale.
- Polars: Everything except Fraction lowers to Polars. That includes windowed columns such as cum_sum, diff, rank and rolling_*, with
by to partition, e.g. Amount.cum_sum(by Account).
- Booleans and complex numbers: Boolean has display formats (✓✗) and logic functions. Complex has re, im, conj, arg and i, so 3 + 4 *
i is 3+4i.
- Built-in constants in every file: pi, c, kB, NA and G, among others. The exact ones follow the 2019 SI and the rest CODATA 2022.
- App: Type, unit and format pickers with live checking. Whole-column copy and paste carries type, unit and format.
- Install: macOS (Apple silicon and Intel), Windows 64-bit, and Omarchy/Arch (x86_64 and aarch64). On macOS or Linux, run curl -fsSL
https://raw.githubusercontent.com/librasteve/omasheet/main/install.sh | bash. On Windows, use install.ps1 in PowerShell.
- Agent skill: A Claude Code skill is included for reading and writing .omx files.
- Known issues: Double-clicking an .omx in Finder may not open it, though omasheet file.omx does. The macOS and Windows builds are not
code-signed.

The units come from siquant, a Rust crate of SI units and prefixes, at github.com/librasteve/siquant. Repo:
github.com/librasteve/omasheet.

That's 14 lines. The file is docs/releases/0.1.12.md, with no v prefix.
