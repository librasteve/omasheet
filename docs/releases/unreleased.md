# Next release: what has changed since 0.1.12

Notes for the release after 0.1.12, kept here until it is cut. The items marked **Breaking** need a line in the release notes of their own.

## Breaking

- **`Duration` is a kind of `Quantity`, for time.** A bare `Duration` column now shows seconds, `Duration[s]`, in its head with numbers in its cells (`5400`), where it showed `PT1H30M`. Ask for the old look with `Duration[iso]`. `Duration[h; .2f]`, `Duration[min]` and `Duration[%H:%M:%S]` are new.
- **`.Quantity` and `.Duration` are gone.** A `Duration` is already a quantity of time, so there is nothing to convert: `PT90M.Quantity` is `PT90M`, and `5400s.Duration` is `5400s` in a `Duration` column. There is no compatibility form.
- **An uncertainty on a `Duration`** (`10.5(3)s`) is held as seconds and the column is calculated by the engine; it cannot move a date or a time, which are exact.

## Durations

- **Literals:** ISO 8601 (`PT1H30M`, no calendar years or months), a quantity of time (`139s`, `90min`, `1.5d`) and a signed colon form, `+11:30` or `-0:00:30.5` (the sign is what makes it a duration; an unsigned `11:30` is still a time of day). After an operand a sign is an operator, so `09:00 + +01:30` is `10:30`.
- **Arithmetic:** two dates or times subtract to a `Duration`; `Duration / Duration` is a number; a `Duration` compares with a quantity of time (`Took > 1h`).
- **Display:** `Duration[iso]`, `Duration[%H:%M:%S]` (`+34:05:30`, hours past 24), or any unit with a number format, and `v` puts the unit in each row. A value in a Markdown sentence uses its column's format (`139s`).
- **Interchange:** a `Duration` is a native Parquet duration, and one with an uncertainty is a `value` and `sigma` struct; a bare number in a `Duration` column is read in its unit.
- **Units:** `min`, `h`, `d`, `wk` and `yr` (the Julian year).

## Results have a kind

The product or quotient of two named quantities is the kind of its dimension: `Force * Length` is an `Energy`, `Length / Speed` a `Duration`. Where two kinds share a dimension the result is `Energy` (not `Torque`), `Frequency` (not `Radioactivity`), `Momentum` (not `Impulse`) or `SpecificEnergy` (not `Dose`); a dimension with no kind stays a `Quantity`. A column may declare the other kind or any unit of the dimension and takes the result all the same. Error messages name one kind (`Energy`), not every kind of the dimension. A missing root now fails when the file is checked: `Length ** (1/2)` is an error before any value is known.

## Powers and roots of quantities

`3m ** 2` and `2m ** 4` keep working, and a root is the power `1/2`, `1/3` or `1/4`: `9m^2 ** (1/2)` is `3m` and `8m^3 ** (1/3)` is `2m`, with the uncertainty carried through. A root the unit does not divide (`5m ** (1/2)`) and any root past the fourth are errors. Where the exponent is written out the result has the kind of the new dimension.

## New metric units

Each is an exact multiple of an SI unit (or a conventional value), and each takes part in prefixes where it says so. None is imperial or cgs.

- **Length:** `au` (149 597 870 700 m), `ly` (light year, 9 460 730 472 580 800 m), `ld` (light day), `pc` (parsec) and `Å` (angstrom); `ly`, `pc` and `yr` take prefixes, so `kpc`, `Mly` and `Gyr` work. `nmi` is the nautical mile (1852 m) and `kn` the knot.
- **Mass:** `ct` (carat, 200 mg), and `kt` and `Mt` (kilotonne and megatonne, as the tonne `t` takes no prefix).
- **Area:** `are` (100 m²) and `barn` (1e-28 m², spelled out because `b` is the bit; `mbarn` works).
- **Angle:** `gon` (1/400 of a turn) and `rev` (a turn, 2π rad).
- **Pressure and power:** `atm` (101 325 Pa), `torr` (101325/760 Pa) and `hp`, the **metric** horsepower, 735.49875 W. The German `PS` cannot be a symbol because it is a petasiemens (`P` and `S`); `hp` is not the mechanical horsepower of 745.7 W.
- **Radiation:** `rem` (0.01 Sv), `Ci` (curie, 3.7e10 Bq) and `R` (roentgen, 2.58e-4 C/kg).
- **Other:** `Jy` (jansky), `tog` (0.1 K·m²/W) and `sverdrup` (1e6 m³/s; `Sv` is the sievert).
- **Left out on purpose:** imperial and US units, cgs units (`dyn`, `erg`, `G`, `Mx`, `Oe`, `Gal`, `P`, `St`), the rad of dose, `g0` (use the constant `gn`), `kgf`, solid-angle `deg²` and `spat`.
- `lm` (lumen) and `lx` (lux), with the `LuminousFlux` and `Illuminance` kinds, are now in the Radiation group of the unit picker, and the empty Light group is gone. The units come from siquant 0.1.6.
