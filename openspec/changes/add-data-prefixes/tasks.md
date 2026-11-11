# Tasks: Data units

## 1. siquant
- [x] 1.1 Binary prefixes in `prefixes.yaml`; the bit and the byte (`prefixable: both`); `Information` and `DataRate`; a `Data` menu group
- [x] 1.2 Tests: kB/KiB/Mb/MiB, rates, the validator stays clean, the binary scale

## 2. omasheet
- [x] 2.1 A new siquant revision; `1Ki.` and `4GiB + 512MiB` work with no code change
- [x] 2.2 Engine tests, `examples/data.omx` and its expected output, docs
- Demo: `omasheet eval '4GiB + 512MiB'` is `4.5GiB`; `omasheet print examples/data.omx` (done)
