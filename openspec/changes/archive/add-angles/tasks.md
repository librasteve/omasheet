# Tasks: Angles

## 1. siquant
- [x] 1.1 `rad`, `sr`, `°` `′` `″`, `lm`, `lx`; `Angle`, `SolidAngle`, `AngularSpeed`, `LuminousFlux`, `Illuminance`; the `Angle` and `Light` groups
- [x] 1.2 `Quantity::from_dms` and `to_dms`; a quantity in its own unit is not rounded
- [x] 1.3 Tests: conversions, alternates, `°` against `°C`, the dimension, DMS read and written, the round trip

## 2. omasheet
- [x] 2.1 The lexer reads `45°30′30″` after a number; `′` and `″` are unit characters
- [x] 2.2 `sin`, `cos` and `tan` of an angle; `dms`
- [x] 2.3 Engine and lexer tests, `examples/angles.omx` and its expected output, docs
- [x] 2.4 A new siquant revision in `Cargo.toml` (a path dependency until siquant is pushed)
- Demo: `omasheet eval 'sin(30°30′)'`; `omasheet print examples/angles.omx`
