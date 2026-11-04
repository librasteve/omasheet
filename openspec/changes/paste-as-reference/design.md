# Design: a read-only view is guarded, and a copy can be pasted as a reference

## Context

Source: the conversation of 2026-11-04. Builds on `add-dataframe-interop` D49 (a view is read-only) and on the clipboard of `add-interactive-app`. D numbers continue from `restore-complex` (D206).

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D207 | Decided | An edit of a read-only sheet is refused when it is started, in the bottom bar, at the prominence of the app's other refusals | Owner, 2026-11-04. Today the editor opens and the refusal comes on commit (`edit_sheet` in `omasheet-ui/src/file.rs`), after the user has typed. `Document::read_only` already has the reason |
| D208 | Decided | Clear, delete and paste onto a read-only sheet say the same; a cut from one is a copy, and says so | A cut would clear what it cuts, which nothing can do here |
| D209 | Decided | The system clipboard keeps its tab-separated text. A copy also remembers its source in the app, and a reference is made from that | A custom clipboard type would need Qt clipboard code in Rust, and QML only gives text. The memory beside the text is how Paste Values already works. A reference is therefore only for a paste into this app |
| D210 | Decided | Ctrl+V stays values and cells; **Paste as Reference** is a command of its own, beside Paste Values and Paste Transposed | Owner, 2026-11-04, choosing it over references by default. Predictable: what Ctrl+V gives does not depend on where the cells came from |
| D211 | Decided | Paste as Reference into data cells writes `= Sheet[Col; row]` for each cell | |
| D212 | Decided | Paste as Reference into a formula column sets its formula, aligned by row: a whole column is `Sheet[Col; *]`; rows copied from `s` and pasted from `t` are `Sheet[Col; *+(s-t)]`, written `*-n` for a negative offset and plain `*` for none; one cell is `Sheet[Col; row]`, the same in every row | Owner, 2026-11-04. A formula column has one formula for all its rows, so the offset applies to the whole column and a row that falls outside the source is empty. A partial-height paste is allowed, since the offset is useful (a lag column). A block over several formula columns gives one formula each, in order |
| D213 | Decided | From the same sheet the sheet's name is left out | `Fare`, not `Passengers.Fare`, in a decorator |
| D214 | Decided | A copied formula pasted onto another sheet has its names qualified with the sheet it came from | `[Revenue; *-1]` becomes `Sales[Revenue; *-1]`, so it reads what it read. Only for a paste to a different sheet |
| D215 | Decided | A reference to a row of a sorted view is to its place in the source, not in the grid | Owner, 2026-11-04. A grid sort changes what the grid calls row 3, not the source's row 3. The copy resolves the displayed row to the source's before it makes the reference. Docs say so |
| D216 | Decided | The status bar says what a Paste as Reference wrote | "pasted `Passengers[Fare; *+5]`" |
| D217 | Decided | Paste as Reference is Ctrl+Shift+R | Owner, 2026-11-04. Paste Values stays Ctrl+Shift+V |
| D218 | Decided | Paste Transposed moves from Ctrl+Alt+V to Ctrl+Shift+T | Owner, 2026-11-04, so the three pastes share the Shift family: V values, T transposed, R reference. `Main.qml` and `docs/app.md` have it as Ctrl+Alt+V today. The old key is dropped, not kept beside it |

## Holding

- A reference made in the app is not on the clipboard for other programs. Copied in this app and pasted into a text editor it is the tab-separated values, as now.
- A reference to a row by its place breaks when rows are inserted or deleted before it in the source, as any index would. A `view` has no inserts to make, so this matters for a sheet of the file's own, where a reference to a row by place is what a formula cell `[Col; 3]` already is.

## Risks

- `Clip` matches the clipboard text to its memory by the text alone (`copied`), so if two copies have the same text the later one wins, which is right. If another program changes the clipboard to the same text, the paste is taken as ours. This is as Paste Values has it and no worse.
