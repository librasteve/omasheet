# Proposal: A format for any quantity, and the `s` conversion

## Why

A derived constant such as ħ = h/2π is exact: it has no uncertainty to set its digits, so a `Quantity` shows it as a `Float` does, to three places, and `1.0545718e-34J.s` is shown as `0J.s·`. A column format could fix that, but a format needs a unit (`Mass[g; .9e]`), which a generic `Quantity` column of mixed dimensions does not have, and a precision such as `.9e` is forced on a value that has an uncertainty, where the `(nn)` should set the digits.

## What Changes

- **file-format** — ADDED: the unit part of a quantity format may be left out when a number format follows the `;`: `Quantity[; .10s]`, `Mass[; .10s]`. Each value is then shown in its own unit, or in the kind's own unit for a named kind
- **file-format** — ADDED: the conversion `s` for a `Scientific` or a `Quantity`, in which the uncertainty leads. A value with an uncertainty is shown by it, as with no precision, and a value with none is shown in `g` form to the precision as significant digits: `.10s` shows `1.0545718176461565e-34` as `1.054571818e-34` and `6.674 30(15)e-11` as itself

- **file-format** — ADDED: the flag `_`, first in a quantity's number format after any `±` or `~`, which sets the unit apart from its number by a space: `Quantity[; _.10s]` shows `6.022e+23 mol^-1`
- **app** — CHANGED: a number cut for display has its `·` after the unit, `6.022e+23mol^-1·`, and the footer says the full value of the cell the cursor is on as well as the one under the pointer

- **app** — ADDED: `Set format…` on the Value column of the Constants tab, and F4 there opens it alone. The format, such as `; .10s`, is written to the file as a `consts [; .10s]` line before the first sheet; it is shown to every constant that can take it, and the values are set right

## Impact

- `QuantityFormat.unit` is optional; a format that names no unit is not a conversion and does not set the unit a bare number in the column is read in
- `SciConv` has a fourth member, `Auto`, worked out as `g` before any digit is cut

## Out of Scope

- Showing a bare exact `Quantity`, with no format, in e notation: a `Quantity` is an `f64`, and the three places hide the noise of its arithmetic (`0.3m - 3 * 0.1m`), which a column format asks to see
