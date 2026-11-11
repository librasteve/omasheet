# Proposal: Data units (Phase 4, stage 2)

## Why

Sizes and rates of data are written in two families of prefix, the SI ones (`kB` is 1000 B) and the binary ones (`KiB` is 1024 B), and a sheet that mixes them needs the difference kept, not guessed. Stage 1 made a unit part of a value; this stage gives data a dimension of its own so `4GiB + 512MiB` is `4.5GiB`, a rate times a time is an amount, and `5B + 3m` is an error.

## What Changes

- **units** — ADDED: the bit `b` and the byte `B` (eight bits), each taking the SI prefixes and the binary prefixes `Ki Mi Gi Ti Pi Ei Zi Yi Ri Qi`; rates by combination (`MiB.s^-1`, `kb.s^-1`) with no unit of their own; the scale literal with a binary prefix (`1Ki.` is 1024); the quantity types `Information`, `DataRate` and `InformationDensity`

## Impact

- No omasheet code changes beyond a new `siquant` revision: units, scales and quantity types come from siquant's resource files, so the lexer, types and display already follow
- The pickers gain a Data group (`siquant::GROUPS`)
- `5b` is five bits and `0B` is no bytes; `0b101` is still a binary number

## Out of Scope

- Non-SI data units (`bps`, `kbps` as names), a shortest-name display (`4096MiB` is not shown as `4GiB`), and currencies
