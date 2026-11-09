# Tasks: Keys for the filter box

## 1. Popup buttons and Enter (`app`)
- [x] 1.1 Replace OK and Cancel with one **Close**; `accept()` goes, and Esc and Close behave alike
- [x] 1.2 Enter applies when `changed`, and closes when nothing is left to apply
- [x] 1.3 Enter on an error does nothing, as OK did
- [x] 1.4 Multi-line field: Ctrl+Enter follows the same two steps

## 2. Arrow keys
- [x] 2.1 Up and Down move focus between the sort entries, the field, Clear filter and the buttons
- [x] 2.2 Left and Right move between the buttons
- [x] 2.3 In the field the arrows move the caret, and Up from the first line or Down from the last leaves it
- [x] 2.4 The focused control is drawn so that it can be seen

## 3. Shortcut
- [x] 3.1 Ctrl+Shift+F opens the popup for the selected column, with the field focused
- [x] 3.2 Check it against the keys already taken in the grid and the entry box
- [x] 3.3 Add it to the shortcut list (Ctrl+?) and `help.txt`

## 4. Docs and verify
- [x] 4.1 `docs/app.md`: the Sorting and filtering section and the Keys table
- [ ] 4.2 Run the app by hand: open with the shortcut, type, Enter, look, Enter, and the popup is closed with the filter in force (the QML loads clean offscreen; the keys are not yet exercised)
