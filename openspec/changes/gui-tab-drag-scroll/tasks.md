# Tasks: GUI: scroll the tab bar while a sheet tab is dragged

## 1. Scrolling (`app`)
- [x] 1.1 Record the pointer's place along the visible bar while a tab is dragged
- [x] 1.2 A timer scrolls the bar near either end, faster the nearer the end, and stops at the ends
- [x] 1.3 The landing gap is worked out again as the tabs move under a still pointer
- [x] 1.4 Nothing happens while the bar does not scroll

## 2. Docs and verify
- [x] 2.1 `docs/app.md`: nothing says how tabs are dragged, so nothing to change
- [x] 2.2 Run the app: open a file with enough sheets to scroll the bar, drag a tab and hold at the right end, then the left; drop it
