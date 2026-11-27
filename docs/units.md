# Units and dimensions

A number followed at once by an SI unit is a quantity: `5kg`, `300cm`, `9.81m.s^-2`, `20°C`. Quantities add, multiply and convert, and a calculation that mixes dimensions is an error that says why. This chapter is a tour; [Expressions](expressions.md#units) is the reference.

## Writing a quantity

The unit follows the number with no space. A unit of more than one term is joined with `.`, each term with an optional `^` and a whole exponent, and a negative exponent stands where a `/` would: `kg.m^2.s^-2`, `m.s^-1`. All the SI prefixes from `q` to `Q` work on every unit that takes one, and the metric non-SI units (`l`, `t`, `ha`, `bar`, `eV`, `cal`, `Wh`, `min`, `h`, `d` and more) are there too.

Results are shown in the SI style, with a raised dot and superscripts: `m⋅s⁻¹`, `kg⋅m²`. Either form is read.

## Sums convert, products combine

```text
> 5kg + 300g
5.3kg
> 300g + 5kg
5300g
> 2m * 3m
6m²
> 10m / 4s
2.5m⋅s⁻¹
> 6m / 3cm
200
```

A sum converts its right side to the unit of the left. A product or quotient combines the units, and where they cancel the answer is a plain number.

## Converting

`.to` gives a value in another unit of the same dimension:

```text
> 5kg.to('g')
5000g
> 1KiB.to('B')
1024B
```

## Mixing dimensions is an error

```text
> 5kg + 3m
error: cannot add kg and m: they measure Mass and Length
```

The message names the dimensions, which is usually what the mistake was.

## Powers and roots

A quantity to a whole power has its unit to that power, `2m ** 3` is `8m³`. A root is the power `1/2`, `1/3` or `1/4`, and it needs every exponent of the unit to divide by it: `9m^2 ** (1/2)` is `3m`, and `5m ** (1/2)` is an error.

## Kinds

Some dimensions have a name, a *kind*: `Length`, `Mass`, `Area`, `Force`, `Energy`, `Speed`, `Duration` and more. A column can declare one, `Load : Mass`, and then admits only a unit of that dimension, converting whatever is written to the column's unit. The product or quotient of two quantities takes the kind that has the dimension of the result: `Force * Length` is an `Energy`, and `Length / Speed` a `Duration`.

## Data

Data has two units, the bit `b` and the byte `B`, and takes the SI prefixes as well as the binary ones: `kB` is 1000 B and `KiB` is 1024 B.

```text
> 4GiB + 512MiB
4.5GiB
> 10MB.s^-1 * 3s
30MB
```

## Computing

| Unit | Counts | Rate | Kinds |
| --- | --- | --- | --- |
| `tok` | tokens | `tok.s^-1` | `Tokens`, `TokenRate` |
| `flop` | floating point operations | `FLOPS` | `Flops`, `FlopRate` |
| `op` | other operations | `OPS`, or `IOPS` for storage | `Operations`, `OperationRate` |
| `px` | pixels | | `Pixels` |
| `b` | bits | `bps` | `Data`, `DataRate` |

All take the SI prefixes: `Mtok`, `PFLOPS`, `TOPS`, `kIOPS`, `Mpx`, `Gbps`. Each of tokens, floating point operations, other operations and pixels is a dimension of its own, so adding one to another is an error. They combine with any unit, for a price or an efficiency:

```text
> 1000tok / 20s
50tok⋅s⁻¹
> 2Mtok * 3USD.Mtok^-1
6USD
```

A named rate stays as it is written in a product or quotient, so ask for the unit wanted: `(1MiB / 1Mbps).to('s')` is `8.389s·`.

## Angles, temperature and time

Angles have the radian `rad`, the steradian `sr` and the degree, minute and second of arc, written together as `45°30′30″`; `sin`, `cos` and `tan` take an angle in any unit. `20°C` is Celsius, and an interval of two temperatures keeps the unit: `20°C - 10°C` is `10°C`. Light has the candela `cd`, the lumen `lm`, the lux `lx` and, for the brightness of a screen, the `nit`, a candela per square metre (`1000nit`, kind `Luminance`). Time has `s`, `min`, `h`, `d`, `wk` and `yr`, and a `Duration` column holds a length of time with its own display format, as in the benchmark sheet's `Duration[ms; v_.1f]`.

## Money

Money is a kind of quantity, `Currency`, and the currencies are units by their codes. See [Exact money](exact-money.md).

## Uncertainty

A quantity carries an uncertainty like any `Scientific`: `5kg ± 100g` is `5.00(10)kg`, and it goes through the arithmetic. See [Constants and uncertainty](constants.md).

## Where next

[Expressions](expressions.md#units) lists every unit and the rules for each kind, and the app has a unit picker with a sector for `Metric`, `Non-Metric`, `Currency` and `All`.
