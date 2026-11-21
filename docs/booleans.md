# Booleans and logic

A `Boolean` is `true` or `false`. A column of them can be shown as words, as `1` and `0`, as ticks and crosses, or as any two characters you like, and the logic that combines them includes the gates of digital electronics.

## Reading and showing

A `Boolean` column reads `true` and `false`, and also `1` and `0`, `x` and `o`, and `X` and `O`. The format, in brackets after the type, says how it is shown: any two different characters, the symbol for true and then for false. The value is unchanged, only how it is shown. An empty cell is neither true nor false.

```omx
A    : Boolean[10]
Done : Boolean[✓✗]
Sq   : Boolean[XO]
```

## Operators

`and`, `or` and `not` are written as words. The gates that `and` and `or` have no word for, `xor`, `nand` and `nor`, are written between two values too. `or` and `nor` bind loosest, then `xor`, then `and` and `nand`. An empty value is treated as absent: `xor` and `nand` are empty if either side is.

## A truth table

`examples/booleans.omx` builds the truth table of two inputs, with each gate as a column:

```omx
And  := A and B
Or   := A or B
Xor  := A xor B
Nand := A nand B
Nor  := A nor B
Sum  := A.Integer + B.Integer
Mul  := A.Integer * B.Integer
```

```text
A | B | And | Or | Xor | Nand | Nor | Sum | Mul
--|---|-----|----|-----|------|-----|-----|----
0 | 0 | 0   | 0  | 0   | 1    | 1   |   0 |   0
0 | 1 | 0   | 1  | 1   | 1    | 0   |   1 |   0
1 | 0 | 0   | 1  | 1   | 1    | 0   |   1 |   0
1 | 1 | 1   | 1  | 0   | 0    | 0   |   2 |   1
```

`Sum` and `Mul` are the arithmetic on 0 and 1 that the gates come from: `and` is the product being 1, and `xor` is the sum being 1.

## Across a column

`x.all()`, `x.any()` and `x.none()` say whether every, some or no value of a column is true. As `sum` and `count` do, they leave the empty ones out, so a column of three squares with one empty is `all` true if the other two are. Where that matters, test `x.count() == 3` too, as the noughts and crosses example does:

```omx
Column A | = Board.A.count() == 3 and Board.A.all()
```

## Choosing

`if … then … else …` picks one of two values: `if Lines.X.Integer.sum() > 0 then 'X' else 'no one yet'`.

## An example

The same file plays noughts and crosses: a `Board` of three rows of `Boolean[XO]` squares, a `Lines` sheet that asks of each row, column and diagonal whether X or O holds it, and a `Result` sheet that names the winner. `omasheet print examples/booleans.omx` shows it worked out.

## Where next

[The file format](format.md#types) lists the other symbols a `Boolean` format takes, and [Expressions](expressions.md#functions) the functions that test for empty and `NaN` values.
