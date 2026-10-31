# Proposal: Bitwise Operations

> **Archived**: built, verified and shipped; every task is done.

## Why

`Hex`, `Octal` and `Binary` (`add-radix-integers`) let a column of register
values, flags or permissions be written as it is meant, but nothing can be
done with the bits: a mask can't be applied, a flag can't be set or tested,
and a field can't be shifted out of a word.

## What Changes

- **omx-expressions**: ADDED. The numeric bitwise operators, spelled as Raku
  spells them: `+&`, `+|` and `+^` for and, or and xor, `+^` before a number
  for its complement, and `+<` and `+>` for the shifts. `and`, `or` and
  `not` stay the Boolean words they are.
- **numerics**: ADDED. What a bitwise operation gives: the base of its
  operands where they share one, as arithmetic does (D128), and for a shift
  the base of the number shifted.
- **file-format**: MODIFIED. A `+|` in a cell's formula does not divide the
  cell, as `|>` does not.

## Impact

- `omasheet-omx`: the lexer reads the six operators; the parser places them;
  the checker takes whole numbers only; the cell splitter keeps `+|`
- `omasheet-engine`: evaluation on the row path, and lowering to Polars,
  whose `and`, `or`, `xor` and `not` are bitwise on integers
- `omasheet-interop`: an XLSX Workbook export writes the values of a
  bitwise formula, with a note, since Excel's `BITAND` and the rest take
  only 0 to 2^48 - 1
- A file that uses none of the six reads and calculates as before

## Out of Scope

- Counts of bits (`bitwise_count_ones` and the like) and the column
  aggregates `bitwise_and`, `bitwise_or` and `bitwise_xor`
- Bitwise operations on anything but whole numbers
- Raku's string and Boolean bitwise operators (`~&`, `?|` and the rest)
