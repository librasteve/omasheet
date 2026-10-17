// Copyright (c) 2026 Stephen Roe

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs as Dialogs
import QtQuick.Layouts
import io.omacom.omasheet

ApplicationWindow {
    id: win
    width: 1180
    height: 760
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    title: (file.modified ? "* " : "") + file.fileName + " - Omasheet"

    // ---- theme -------------------------------------------------------------

    readonly property color pageColor: file.themeBackground
    readonly property color textColor: file.themeForeground
    readonly property color accentColor: file.themeAccent
    readonly property color selectionFill: file.themeSelection
    readonly property color mutedColor: file.themeMuted
    readonly property color errorColor: file.themeError
    readonly property color lineColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.14)
    readonly property color panelColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.05)
    readonly property color computedTint: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.07)

    // Ctrl+plus and Ctrl+minus change the size; everything scales from it.
    property int fontSize: 14
    // Dialogs are set a little smaller than the file.
    readonly property int dialogSize: fontSize - 2
    readonly property int rowHeight: fontSize * 2
    readonly property int headerHeight: Math.round(fontSize * 3.3)
    readonly property int gutterWidth: Math.round(fontSize * 3.4)
    onFontSizeChanged: Qt.callLater(function() { win.measure(); win.reveal(); })

    function zoom(step) {
        fontSize = step === 0 ? 14 : Math.max(8, Math.min(40, fontSize + step));
    }

    Material.theme: file.darkMode ? Material.Dark : Material.Light
    Material.accent: accentColor
    Material.background: pageColor
    Material.foreground: textColor
    color: pageColor
    font.family: "monospace"
    font.pixelSize: fontSize
    // No ligatures: `|>`, `==` and `:=` are shown as they are typed.
    font.features: { "calt": 0, "liga": 0 }

    FontMetrics {
        id: metrics
        font.family: "monospace"
        font.pixelSize: win.fontSize
        font.features: win.font.features
    }

    // ---- document state ----------------------------------------------------

    // The file, shared with every other window open on it.
    required property File file
    // Whether this is the only window on the file.
    property bool alone: true
    // Asks for another window on the same file, showing the sheet `tab`.
    signal newWindow(int tab)
    // The window has closed for good.
    signal dismissed()

    // The parsed snapshot, and the tabs made from it: one per sheet, plus
    // the constants shown as a sheet of their own.
    property var snap: ({ sheets: [], consts: [], problems: [] })
    property var tabs: []
    property int tab: 0
    readonly property var sheet: tab < tabs.length ? tabs[tab] : emptySheet
    readonly property var emptySheet: ({ name: "", columns: [], rows: [], isConsts: false })
    property var colWidths: []
    property real totalWidth: 0

    // A frame drawn alone can be shown late, when the next one comes: moving
    // up or down a row draws just the one. So each last frame is drawn twice.
    property bool drawnAgain: false
    onFrameSwapped: {
        drawnAgain = !drawnAgain;
        if (drawnAgain)
            update();
    }

    property int curRow: 0
    property int curCol: 0
    property int anchorRow: 0
    property int anchorCol: 0
    readonly property int selTop: Math.min(curRow, anchorRow)
    readonly property int selBottom: Math.max(curRow, anchorRow)
    readonly property int selLeft: Math.min(curCol, anchorCol)
    readonly property int selRight: Math.max(curCol, anchorCol)

    // Where the arrow keys are pointing a formula reference, while one is
    // being typed: -1 when no such pick is under way.
    property int pickRow: -1
    property int pickCol: -1
    // The reference last placed by it, to take back out on the next move.
    property int pickAt: -1
    property int pickLen: 0

    property bool editing: false
    // The source takes the place of the grid while it is shown.
    property bool showSource: false
    onShowSourceChanged: Qt.callLater(function() {
        if (showSource) sourceText.forceActiveFocus();
        else grid.forceActiveFocus();
    })
    // Take the source as it has been edited: one step to undo.
    function applySource() {
        if (showSource && sourceText.text !== file.sourceText())
            file.setSourceText(sourceText.text);
    }
    function viewSource(show) {
        applySource();
        showSource = show;
    }
    property string editSeed: ""
    property bool closeConfirmed: false
    property string pendingAction: ""
    property url pendingUrl

    function cellAt(row, col) {
        var r = sheet.rows[row];
        return r && r[col] ? r[col] : null;
    }

    function reload() {
        // Any edit may renumber the columns that were cut.
        grabbed = null;
        // What was copied stays marked through an edit that says where it
        // went, and no other.
        if (!keepMark)
            marked = null;
        // Stay on the same sheet when another window adds or removes one.
        var name = sheet.name, consts = sheet.isConsts;
        var parsed = JSON.parse(file.snapshotJson());
        var list = [];
        for (var i = 0; i < parsed.sheets.length; i++) {
            parsed.sheets[i].isConsts = false;
            list.push(parsed.sheets[i]);
        }
        if (parsed.consts.length > 0) {
            var rows = [];
            for (var k = 0; k < parsed.consts.length; k++) {
                var c = parsed.consts[k];
                rows.push([
                    { d: c.name, s: c.name, e: "", x: "", n: false, f: false },
                    { d: c.source, s: c.source, e: c.error, x: "", n: false, f: true },
                    { d: c.error ? "#ERROR" : c.display, s: c.source, e: c.error, x: c.exact, n: false, f: false }
                ]);
            }
            list.push({
                name: "Constants",
                isConsts: true,
                columns: [
                    { name: "Name", type: "", formula: "", computed: false, declared: false },
                    { name: "Expression", type: "", formula: "", computed: false, declared: false },
                    { name: "Value", type: "", formula: "", computed: true, declared: false }
                ],
                rows: rows
            });
        }
        snap = parsed;
        tabs = list;
        for (var t = 0; t < list.length; t++) {
            if (list[t].name === name && list[t].isConsts === consts) {
                tab = t;
                break;
            }
        }
        if (tab >= tabs.length)
            tab = Math.max(0, tabs.length - 1);
        measure();
        clampCursor();
    }

    // Open a link from a cell: one with no scheme is a file beside the file.
    function openLink(link) {
        if (!/^[a-z][a-z0-9+.-]*:/i.test(link) && file.filePath.length > 0)
            link = "file://" + file.filePath.replace(/[^\/]*$/, "") + link;
        Qt.openUrlExternally(link);
    }

    function measure() {
        var widths = [];
        var total = 0;
        var cols = sheet.columns;
        for (var c = 0; c < cols.length; c++) {
            // A formula column shows `:= ` before its type.
            var chars = Math.max(cols[c].name.length, cols[c].type.length + (cols[c].computed ? 5 : 2));
            for (var r = 0; r < sheet.rows.length; r++)
                chars = Math.max(chars, sheet.rows[r][c].d.length);
            var w = Math.round(Math.min(46, Math.max(7, chars + 2)) * metrics.averageCharacterWidth) + 14;
            widths.push(w);
            total += w;
        }
        colWidths = widths;
        totalWidth = total;
    }

    function clampCursor() {
        var maxRow = Math.max(0, sheet.rows.length - 1);
        var maxCol = Math.max(0, sheet.columns.length - 1);
        curRow = Math.min(curRow, maxRow);
        anchorRow = Math.min(anchorRow, maxRow);
        curCol = Math.min(curCol, maxCol);
        anchorCol = Math.min(anchorCol, maxCol);
    }

    function selectCell(row, col, extend) {
        if (sheet.rows.length === 0 || sheet.columns.length === 0)
            return;
        if (!extend) {
            byRow = false;
            byCol = false;
        }
        curRow = Math.max(0, Math.min(sheet.rows.length - 1, row));
        curCol = Math.max(0, Math.min(sheet.columns.length - 1, col));
        if (!extend) {
            anchorRow = curRow;
            anchorCol = curCol;
        }
        reveal();
    }

    // Select whole rows up to row `index`, or whole columns up to column
    // `index`, from where the selection was if it is to `extend`. The cursor
    // waits in the first cell.
    function selectLine(rows, index, extend) {
        var maxRow = Math.max(0, sheet.rows.length - 1);
        var maxCol = Math.max(0, sheet.columns.length - 1);
        byRow = rows;
        byCol = !rows;
        if (rows) {
            curRow = Math.max(0, Math.min(maxRow, index));
            curCol = 0;
            anchorCol = maxCol;
            if (!extend)
                anchorRow = curRow;
        } else {
            curCol = Math.max(0, Math.min(maxCol, index));
            curRow = 0;
            anchorRow = maxRow;
            if (!extend)
                anchorCol = curCol;
        }
        reveal();
    }

    // Go one way with an arrow key: by `dRow` rows or `dCol` columns, or to
    // the end with `ctrl`. Left of the first column are the row numbers, and
    // above the first row the column headers: there the whole row or column
    // is selected, and goes up and down or left and right as a whole; past
    // those is the corner, which is the whole sheet. While
    // some are picked up the arrows go no other way: what they choose is
    // where to put them down, not a cell.
    function arrow(dRow, dCol, ctrl, extend) {
        var vertical = dRow !== 0;
        var d = vertical ? dRow : dCol;
        var max = Math.max(0, (vertical ? sheet.rows.length : sheet.columns.length) - 1);
        var cur = vertical ? curRow : curCol;
        var to = ctrl ? (d < 0 ? 0 : max) : cur + d;
        if (vertical ? selKind === "row" : selKind === "column") {
            // Past the first of the numbers or the headers is the corner.
            if (d < 0 && cur === 0 && !ctrl && !extend && !grabbedHere)
                selectAll();
            else
                selectLine(vertical, to, extend);
        } else if (selKind === "all") {
            // Off the corner, to the numbers below it or the headers beside.
            if (d > 0)
                selectLine(vertical, 0, false);
        } else if (selKind === "row" || selKind === "column") {
            // Back in from the numbers or the headers.
            if (d > 0 && !grabbedHere)
                selectCell(vertical ? (ctrl ? max : 0) : curRow, vertical ? curCol : (ctrl ? max : 0), false);
        } else if (selKind === "cells" || !grabbedHere) {
            if (d < 0 && cur === 0 && !ctrl && !extend && selKind === "cells" && !sheet.isConsts)
                selectLine(!vertical, vertical ? curCol : curRow, false);
            else
                selectCell(vertical ? to : curRow, vertical ? curCol : to, extend);
        }
    }

    // Select the whole sheet: the cursor waits in the first cell.
    function selectAll() {
        byRow = false;
        byCol = false;
        anchorRow = Math.max(0, sheet.rows.length - 1);
        anchorCol = Math.max(0, sheet.columns.length - 1);
        curRow = 0;
        curCol = 0;
    }

    // Let go of the selection, and of anything cut or copied: one cell is
    // left, where the cursor is.
    function deselect() {
        grabbed = null;
        marked = null;
        file.forgetCut();
        notice = "";
        byRow = false;
        byCol = false;
        anchorRow = curRow;
        anchorCol = curCol;
    }

    function reveal() {
        revealAt(curRow, curCol);
    }

    function revealAt(row, col) {
        rowsView.positionViewAtIndex(row, ListView.Contain);
        var x = gutterWidth;
        for (var c = 0; c < col; c++)
            x += colWidths[c];
        var w = colWidths[col] || 0;
        if (x - gutterWidth < hflick.contentX)
            hflick.contentX = x - gutterWidth;
        else if (x + w > hflick.contentX + hflick.width)
            hflick.contentX = x + w - hflick.width;
    }

    function switchTab(index) {
        if (index < 0 || index >= tabs.length)
            return;
        stopEditing();
        tab = index;
        curRow = 0; curCol = 0; anchorRow = 0; anchorCol = 0;
        measure();
        hflick.contentX = 0;
        rowsView.positionViewAtBeginning();
        grid.forceActiveFocus();
    }

    // ---- editing -----------------------------------------------------------

    function startEditing(seed) {
        if (!cellAt(curRow, curCol))
            return;
        if (sheet.isConsts && curCol !== 1)
            return;
        // A formula column has one formula, written in the box at the top.
        var column = sheet.columns[curCol];
        if (!sheet.isConsts && column && column.computed) {
            errorDialog.say("`" + column.name + "` is a formula column: click F2 to edit");
            return;
        }
        anchorRow = curRow;
        anchorCol = curCol;
        byRow = false;
        byCol = false;
        editSeed = seed;
        editing = true;
        stopPicking();
    }

    function stopEditing() {
        editing = false;
        stopPicking();
        grid.forceActiveFocus();
    }

    // The text field a formula is being typed into, if any: the cell's own
    // editor or the entry box.
    property var editor: null

    // Whether a click on a cell should add a reference to the formula
    // being typed, rather than move the selection.
    function canPick() {
        return !!editor && !sheet.isConsts && editor.text.trim().charAt(0) === "=";
    }

    // The reference a cell is written by, from the formula being typed:
    // relative to the edited cell, or by row number when `absolute`.
    function refFor(row, col, absolute) {
        var name = sheet.columns[col].name;
        var offset = row - curRow;
        var step = function(n) { return n === 0 ? "*" : "*" + (n > 0 ? "+" : "-") + Math.abs(n); };
        if (absolute) {
            return "[" + name + "; " + row + "]";
        } else if (sheet.columns[curCol] && sheet.columns[curCol].computed) {
            // A column formula names columns: it is the same for every row.
            return offset === 0 ? name : "[" + name + "; " + step(offset) + "]";
        } else {
            // A formula in one cell counts rows and columns from itself.
            return "[" + step(col - curCol) + "; " + step(offset) + "]";
        }
    }

    // Write a reference to a cell at the cursor of the formula being typed.
    function pickCell(row, col, absolute) {
        if (!canPick() || !sheet.columns[col])
            return;
        var ref = refFor(row, col, absolute);
        var at = editor.selectionStart;
        editor.remove(at, editor.selectionEnd);
        editor.insert(at, ref);
        editor.cursorPosition = at + ref.length;
        editor.forceActiveFocus();
        // A click picks outright: a pick under way from the keyboard, if
        // any, does not carry on from here.
        stopPicking();
    }

    // Move the keyboard's formula-reference pick by `dRow`/`dCol` cells,
    // writing (or rewriting, if one is already under way) the reference it
    // lands on, exactly as a click on that cell would. Returns whether
    // there was a formula to pick a cell for.
    function pickArrow(dRow, dCol, absolute) {
        if (!canPick() || sheet.rows.length === 0 || sheet.columns.length === 0)
            return false;
        if (pickRow < 0) {
            pickRow = curRow;
            pickCol = curCol;
            pickAt = editor.selectionStart;
            editor.remove(pickAt, editor.selectionEnd);
            pickLen = 0;
        } else {
            editor.remove(pickAt, pickAt + pickLen);
        }
        pickRow = Math.max(0, Math.min(sheet.rows.length - 1, pickRow + dRow));
        pickCol = Math.max(0, Math.min(sheet.columns.length - 1, pickCol + dCol));
        var ref = refFor(pickRow, pickCol, absolute);
        editor.insert(pickAt, ref);
        pickLen = ref.length;
        editor.cursorPosition = pickAt + ref.length;
        revealAt(pickRow, pickCol);
        return true;
    }

    // End a keyboard pick under way, so that the next arrow pressed starts
    // a fresh one from the edited cell rather than carrying on from here.
    function stopPicking() {
        pickRow = -1;
        pickCol = -1;
        pickAt = -1;
        pickLen = 0;
    }

    // Go to the box at the top, to write what the cell holds there: the
    // one formula of a formula column, from any cell of it or the whole.
    function editInBox() {
        if (tabs.length === 0)
            return;
        stopEditing();
        if (selKind === "column" && selLeft === selRight)
            selectCell(0, selLeft, false);
        if (!hasCursor || !cellAt(curRow, curCol))
            return;
        formulaField.forceActiveFocus();
        formulaField.selectAll();
    }

    function commit(text, dRow, dCol) {
        var row = curRow, col = curCol;
        editing = false;
        stopPicking();
        keeping(row + 1, 0, function() {
            if (sheet.isConsts) {
                if (col === 1)
                    file.setConst(row, text);
            } else {
                file.setCell(tab, row, col, text);
            }
            // Enter on the last row of a sheet opens a new row to carry on typing.
            if (dRow > 0 && row === sheet.rows.length - 1 && !sheet.isConsts)
                file.insertRows(tab, row + 1, 1);
        });
        selectCell(row + dRow, col + dCol, false);
        grid.forceActiveFocus();
    }

    // Whole rows or columns cut with Ctrl+X, waiting to be put down with
    // Ctrl+V: `{tab, rows, first, count}`, or null.
    property var grabbed: null
    // Why the last move was refused, shown in the footer.
    property string notice: ""
    // Whether the selection was last made from a row number or a column
    // header, to tell a row from a column where one selection is both.
    property bool byRow: false
    property bool byCol: false
    readonly property bool wholeColumns: !sheet.isConsts && sheet.columns.length > 0
        && selTop === 0 && selBottom >= sheet.rows.length - 1
    readonly property bool wholeRows: !sheet.isConsts && sheet.rows.length > 0
        && selLeft === 0 && selRight >= sheet.columns.length - 1
    // What is selected: whole "row"s, whole "column"s, "all" of the sheet,
    // or a block of "cells". The menu and the keys act on rows only for rows
    // and cells, and on columns only for columns and cells.
    readonly property string selKind: {
        if (sheet.isConsts || sheet.rows.length === 0)
            return "cells";
        // One cell of a sheet with one row is a cell, unless its header or
        // row number was what was clicked.
        var cols = wholeColumns && (byCol || sheet.rows.length > 1);
        var rows = wholeRows && (byRow || sheet.columns.length > 1);
        if (cols && rows)
            return byRow ? "row" : byCol ? "column" : "all";
        return rows ? "row" : cols ? "column" : "cells";
    }
    // Whole rows or columns are selected as a whole, with no cell of their
    // own to show at the top or to outline.
    readonly property bool hasCursor: selKind === "cells"
    // Several cells that are not whole rows, columns or the sheet.
    readonly property bool isBlock: selKind === "cells" && !sheet.isConsts
        && (selTop !== selBottom || selLeft !== selRight)
    // What a cut would pick up: "row", "column", or "" for anything else.
    readonly property string grabKind: selKind === "row" || selKind === "column" ? selKind : ""
    readonly property bool grabbedHere: grabbed !== null && grabbed.tab === tab && !sheet.isConsts

    // The cells last copied or cut, shown as they were selected for as long
    // as they can be pasted, until Esc or the next copy:
    // `{tab, top, left, bottom, right, text}`, or null.
    property var marked: null
    property bool keepMark: false

    function isMarked(row, col) {
        var m = marked;
        return m !== null && m.tab === tab
            && row >= m.top && row <= m.bottom && col >= m.left && col <= m.right;
    }

    function mark(text) {
        marked = { tab: tab, top: selTop, left: selLeft, bottom: selBottom, right: selRight, text: text, cut: false };
    }

    // Where the rows or columns `first` to `last` are once `by` of them are
    // added before `at`, or taken away from `at` on: `[first, last]`, or null
    // if none are left.
    function shifted(first, last, at, by) {
        if (by >= 0)
            return at <= first ? [first + by, last + by] : at <= last ? [first, last + by] : [first, last];
        var end = at - by;
        var f = first < at ? first : first >= end ? first + by : at;
        var l = last < at ? last : last >= end ? last + by : at - 1;
        return l < f ? null : [f, l];
    }

    // Make an edit that adds or takes away rows at `row` or columns at
    // `col`, or neither, and keep the mark on the cells it was on.
    function keeping(row, col, edit) {
        var m = marked;
        var rows = sheet.rows.length, cols = sheet.columns.length;
        keepMark = true;
        edit();
        keepMark = false;
        if (m === null || m.tab !== tab)
            return;
        var v = shifted(m.top, m.bottom, row, sheet.rows.length - rows);
        var h = shifted(m.left, m.right, col, sheet.columns.length - cols);
        marked = v === null || h === null ? null
            : { tab: m.tab, top: v[0], left: h[0], bottom: v[1], right: h[1], text: m.text, cut: m.cut };
    }

    // Paste `text` with `put`, keeping the mark if it is what was copied.
    // A paste adds rows only at the end, unless it makes room at `row` or
    // `col`.
    function pasteWith(text, put, row, col) {
        if (marked !== null && marked.text !== text)
            marked = null;
        var cut = marked !== null && marked.cut;
        var problem = "";
        keeping(row === undefined ? sheet.rows.length : row, col === undefined ? sheet.columns.length : col,
                function() { problem = put() || ""; });
        // A paste that was refused says why, and what was cut is still cut.
        if (problem.length > 0) {
            errorDialog.say(problem);
            return;
        }
        // Cells that were cut have gone to where they were pasted.
        if (cut)
            marked = null;
    }

    function isGrabbed(row, col) {
        if (!grabbedHere)
            return false;
        var at = grabbed.rows ? row : col;
        return at >= grabbed.first && at < grabbed.first + grabbed.count;
    }

    // Cut: whole rows or columns are picked up to be moved; cells are copied
    // and stay as they are until the next paste, which moves them.
    function cutSelection() {
        notice = "";
        var kind = grabKind;
        if (kind === "" && !sheet.isConsts) {
            grabbed = null;
            var text = file.cutCells(tab, selTop, selLeft, selBottom, selRight);
            mark(text);
            marked.cut = true;
            clipboard.put(text);
            return;
        }
        copySelection();
        // Rows and columns picked up are shown dimmed instead.
        if (kind !== "")
            marked = null;
        if (kind === "row")
            grabbed = { tab: tab, rows: true, first: selTop, count: selBottom - selTop + 1 };
        else if (kind === "column")
            grabbed = { tab: tab, rows: false, first: selLeft, count: selRight - selLeft + 1 };
        else
            clearSelection();
    }

    // Put what was cut down where the selection is. It lands on the selected
    // row or column, and those in between close up.
    function dropGrabbed() {
        var g = grabbed;
        grabbed = null;
        var target = g.rows ? selTop : selLeft;
        if (target >= g.first && target < g.first + g.count)
            return;
        var to = target < g.first ? target : target - g.count + 1;
        var problem = g.rows ? file.moveRows(tab, g.first, g.count, to)
                             : file.moveColumns(tab, g.first, g.count, to);
        if (problem.length > 0) {
            notice = problem;
            return;
        }
        if (g.rows) {
            anchorCol = Math.max(0, sheet.columns.length - 1);
            anchorRow = to + g.count - 1;
            curCol = 0;
            curRow = to;
            byRow = true;
            byCol = false;
        } else {
            anchorRow = Math.max(0, sheet.rows.length - 1);
            anchorCol = to + g.count - 1;
            curRow = 0;
            curCol = to;
            byRow = false;
            byCol = true;
        }
    }

    function copySelection() {
        grabbed = null;
        var text;
        if (sheet.isConsts) {
            var lines = [];
            for (var r = selTop; r <= selBottom; r++) {
                var parts = [];
                for (var c = selLeft; c <= selRight; c++)
                    parts.push(sheet.rows[r][c].d);
                lines.push(parts.join("\t"));
            }
            text = lines.join("\n");
        } else {
            text = file.copyBlock(tab, selTop, selLeft, selBottom, selRight, grabKind === "column");
        }
        mark(text);
        clipboard.put(text);
    }

    function clearSelection() {
        if (!sheet.isConsts)
            keeping(0, 0, function() { file.clearCells(tab, selTop, selLeft, selBottom, selRight); });
    }

    function pasteSelection() {
        notice = "";
        if (grabbedHere) {
            dropGrabbed();
            return;
        }
        grabbed = null;
        var text = clipboard.take();
        if (text.length === 0)
            return;
        pasteWith(text, function() {
            if (!sheet.isConsts)
                return file.pasteCells(tab, selTop, selLeft, selBottom, selRight, text);
            if (curCol === 1)
                file.setConst(curRow, text.split("\n")[0]);
        });
    }

    // Paste what the copied cells showed, not their formulas.
    function pasteValues() {
        notice = "";
        grabbed = null;
        var text = clipboard.take();
        if (text.length > 0 && !sheet.isConsts)
            pasteWith(text, function() { return file.pasteValues(tab, selTop, selLeft, selBottom, selRight, text); });
    }

    // Paste what was copied turned about: its rows as columns.
    function pasteTransposed() {
        notice = "";
        grabbed = null;
        var text = clipboard.take();
        if (text.length > 0 && !sheet.isConsts)
            pasteWith(text, function() { return file.pasteTransposed(tab, selTop, selLeft, selBottom, selRight, text); });
    }

    // Make room for what was copied: copied columns become new columns to
    // the left of the selection, and anything else new rows above it.
    function insertCopied() {
        notice = "";
        grabbed = null;
        var text = clipboard.take();
        if (text.length === 0 || sheet.isConsts)
            return;
        // After the selection: rows under it, columns to its right.
        var row = selBottom + 1, col = selKind === "column" ? selRight + 1 : selLeft;
        pasteWith(text, function() { notice = file.insertCopied(tab, row, col, text); }, row, col);
    }

    // A new sheet beside the one shown: asked for by name.
    function newSheet(after) {
        var here = sheet.isConsts || tabs.length === 0 ? snap.sheets.length : tab;
        promptDialog.ask("sheet", after ? here + 1 : here);
    }

    function deleteSheet() {
        if (sheet.isConsts || tabs.length === 0)
            return;
        notice = file.deleteSheet(tab);
        if (notice.length > 0)
            return;
        // On the sheet that took its place, the whole of it.
        switchTab(Math.min(tab, tabs.length - 1));
        if (!sheet.isConsts)
            selectAll();
    }

    function deleteColumns() {
        if (sheet.isConsts)
            return;
        var col = selLeft, count = selRight - selLeft + 1;
        var whole = selKind === "column";
        keeping(0, col, function() { notice = file.deleteColumns(tab, col, count); });
        if (notice.length > 0)
            return;
        // On the column that took their place: the whole of it, if whole
        // columns were deleted.
        if (whole)
            selectLine(false, col, false);
        else
            selectCell(curRow, col, false);
    }

    // The type the selected columns all have, if there is one: with
    // `declared`, only if each is declared as it.
    function sharedType(declared) {
        var found = "";
        for (var c = selLeft; c <= selRight; c++) {
            var column = sheet.columns[c];
            if (!column || (declared && !column.declared) || (c > selLeft && column.type !== found))
                return "";
            found = column.type;
        }
        return found;
    }

    function anyDeclared() {
        for (var c = selLeft; c <= selRight; c++) {
            if (sheet.columns[c] && sheet.columns[c].declared)
                return true;
        }
        return false;
    }

    // Declare the selected columns as `type`, or with none leave them to
    // take their type from their cells.
    function setColumnTypes(type) {
        if (sheet.isConsts)
            return;
        file.setColumnTypes(tab, selLeft, selRight - selLeft + 1, type);
    }

    // The menu a right-click opens, from the keyboard: for the selection as
    // it is, below the cell the cursor is in.
    function keyMenu() {
        if (tabs.length === 0)
            return;
        var x = gutterWidth - hflick.contentX;
        for (var c = 0; c < curCol; c++)
            x += colWidths[c];
        var y = headerHeight + (curRow + 1) * rowHeight - rowsView.contentY;
        cellMenu.popup(grid, Math.max(0, Math.min(grid.width - cellMenu.width, x)),
                       Math.max(headerHeight, Math.min(grid.height - rowHeight, y)));
    }

    function insertRow(below) {
        insertRows(below, 1);
    }

    function insertRows(below, count) {
        if (sheet.isConsts || count < 1)
            return;
        var at = sheet.rows.length === 0 ? 0 : (below ? selBottom + 1 : selTop);
        // The left of the selection: a whole-row selection ends at the last
        // column, and following it would pan the view to the right.
        var col = selLeft;
        var whole = selKind === "row";
        keeping(at, 0, function() { file.insertRows(tab, at, count); });
        // Leave the new rows selected: as whole rows, if whole rows were.
        if (whole) {
            selectLine(true, at, false);
            selectLine(true, at + count - 1, true);
        } else {
            selectCell(at, col, false);
            selectCell(at + count - 1, col, true);
        }
    }

    function deleteRows() {
        if (sheet.isConsts || sheet.rows.length === 0)
            return;
        var top = selTop, gone = selBottom - selTop + 1;
        var whole = selKind === "row";
        keeping(top, 0, function() { file.deleteRows(tab, top, gone); });
        // On the row that took their place: the whole of it, if whole rows
        // were deleted. Back to the first column, with the view panned
        // fully left.
        if (whole)
            selectLine(true, top, false);
        else
            selectCell(top, 0, false);
        hflick.contentX = 0;
    }

    // Right-click: keep a selection that contains the cell, otherwise move
    // to it. A negative row or column means the whole column or row.
    function openMenu(row, col) {
        stopEditing();
        grid.forceActiveFocus();
        var maxRow = Math.max(0, sheet.rows.length - 1);
        var maxCol = Math.max(0, sheet.columns.length - 1);
        var inRows = row < 0 || (row >= selTop && row <= selBottom);
        var inCols = col < 0 || (col >= selLeft && col <= selRight);
        if (!(inRows && inCols)) {
            anchorRow = row < 0 ? maxRow : row;
            curRow = row < 0 ? 0 : row;
            anchorCol = col < 0 ? maxCol : col;
            curCol = col < 0 ? 0 : col;
            byRow = col < 0 && row >= 0;
            byCol = row < 0 && col >= 0;
        }
        cellMenu.popup();
    }

    // ---- files -------------------------------------------------------------

    function guard(action) {
        applySource();
        if (!file.modified) {
            run(action);
            return;
        }
        pendingAction = action;
        unsavedDialog.open();
    }

    function run(action) {
        if (action === "close") {
            closeConfirmed = true;
            close();
        } else if (action === "new") {
            file.newDocument();
            switchTab(0);
        } else if (action === "open") {
            openDialog.open();
        } else if (action === "openUrl") {
            if (file.openUrl(pendingUrl)) {
                switchTab(0);
                // What an import changed or left behind.
                if (file.notes.length > 0)
                    notesDialog.open();
            }
        }
    }

    function save() {
        applySource();
        if (file.filePath.length === 0) {
            saveDialog.open();
            return false;
        }
        return file.save();
    }

    onClosing: function(close) {
        applySource();
        // Nothing is lost while another window still shows the file.
        if (closeConfirmed || !file.modified || !alone) {
            dismissed();
            return;
        }
        close.accepted = false;
        guard("close");
    }

    Connections {
        target: file
        function onRevisionChanged() { win.reload(); }
    }

    Component.onCompleted: {
        reload();
        grid.forceActiveFocus();
    }

    // Follow a theme change made while the window was in the background.
    onActiveChanged: if (active) file.reloadTheme()

    // Development aid: OMASHEET_UI_SNAPSHOT=<file.png> saves a picture and quits.
    Timer {
        interval: 600
        running: file.snapshotPath.length > 0
        onTriggered: root.grabToImage(function(result) {
            result.saveToFile(file.snapshotPath);
            win.closeConfirmed = true;
            Qt.quit();
        })
    }

    // The clipboard, through a text control: QML has no clipboard object.
    TextEdit {
        id: clipboard
        visible: false
        textFormat: TextEdit.PlainText
        function put(text) {
            clipboard.text = text;
            clipboard.selectAll();
            clipboard.copy();
        }
        function take() {
            clipboard.text = "";
            clipboard.paste();
            return clipboard.text;
        }
    }

    // ---- shortcuts ---------------------------------------------------------

    readonly property bool typing: editing || formulaField.activeFocus || sourceText.activeFocus || promptDialog.opened

    Shortcut { sequence: "Ctrl+S"; onActivated: win.save() }
    Shortcut { sequence: "Ctrl+Shift+S"; onActivated: saveDialog.open() }
    Shortcut { sequence: "Ctrl+O"; onActivated: win.guard("open") }
    Shortcut { sequence: "Ctrl+E"; onActivated: exportChoice.open() }
    Shortcut { sequence: "Ctrl+N"; enabled: !win.typing; onActivated: win.newSheet(true) }
    Shortcut { sequence: "Ctrl+K"; enabled: !win.typing; onActivated: promptDialog.ask("const") }
    Shortcut { sequence: "Ctrl+Shift+N"; onActivated: win.newWindow(win.tab) }
    Shortcut { sequence: "Ctrl+Z"; enabled: !win.typing; onActivated: file.undo() }
    Shortcut { sequences: ["Ctrl+Shift+Z", "Ctrl+Y"]; enabled: !win.typing; onActivated: file.redo() }
    Shortcut { sequences: ["Ctrl++", "Ctrl+="]; onActivated: win.zoom(1) }
    Shortcut { sequence: "Ctrl+-"; onActivated: win.zoom(-1) }
    Shortcut { sequence: "Ctrl+0"; onActivated: win.zoom(0) }
    // With the arrows, or the brackets: not every keyboard has Page Up and
    // Page Down. Shift makes a brace of a bracket, and which of them a
    // keyboard reports, with Shift or without, is its own affair.
    Shortcut { sequences: ["Ctrl+Alt+Right", "Ctrl+Shift+]", "Ctrl+Shift+}", "Ctrl+}"]; onActivated: win.switchTab(win.tab + 1) }
    Shortcut { sequences: ["Ctrl+Alt+Left", "Ctrl+Shift+[", "Ctrl+Shift+{", "Ctrl+{"]; onActivated: win.switchTab(win.tab - 1) }
    Shortcut { sequence: "Ctrl+?"; onActivated: helpDialog.open() }
    Shortcut { sequence: "F1"; onActivated: functionsDialog.open() }
    Shortcut { sequence: "F2"; enabled: !win.showSource; onActivated: win.editInBox() }
    Shortcut { sequence: "F3"; onActivated: win.viewSource(!win.showSource) }
    Shortcut { sequences: ["Meta+F", "F11"]
        onActivated: win.visibility = win.visibility === Window.FullScreen ? Window.Windowed : Window.FullScreen }

    // ---- layout ------------------------------------------------------------

    Rectangle {
        id: root
        anchors.fill: parent
        color: win.pageColor
    }

    ColumnLayout {
        parent: root
        anchors.fill: parent
        spacing: 0

        // The entry box: the source of the current cell.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: win.fontSize * 3 + 2
            color: win.panelColor

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                spacing: 10

                // The file, always: a tiling desktop shows no title bar.
                Text {
                    text: (file.modified ? "* " : "") + file.fileName
                    color: win.textColor
                    font: win.font
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: 260
                    rightPadding: 6
                    MouseArea {
                        id: fileArea
                        anchors.fill: parent
                        hoverEnabled: true
                        ToolTip.visible: containsMouse && file.filePath.length > 0
                        ToolTip.delay: 400
                        ToolTip.text: file.filePath
                    }
                }

                Text {
                    readonly property var column: win.hasCursor ? win.sheet.columns[win.curCol] : undefined
                    // The selection, as a formula would write it. Several
                    // columns go by position: a range is not written with
                    // names.
                    readonly property string across: win.selRight > win.selLeft
                        ? win.selLeft + ".." + win.selRight
                        : win.sheet.columns.length > 0 ? win.sheet.columns[win.selLeft].name : ""
                    readonly property string down: win.selBottom > win.selTop
                        ? win.selTop + ".." + win.selBottom : "" + win.selTop
                    text: win.sheet.isConsts
                        ? (column && win.curRow < win.sheet.rows.length ? win.sheet.rows[win.curRow][0].d : "")
                        : across.length === 0 ? ""
                        : win.sheet.name + (win.selKind === "all" ? ""
                            : win.selKind === "row" ? "[; " + down + "]"
                            : win.selKind === "column" ? "[" + across + "]"
                            : "[" + across + "; " + down + "]")
                    color: win.mutedColor
                    font: win.font
                    Layout.minimumWidth: 150
                    elide: Text.ElideRight
                }

                TextField {
                    id: formulaField
                    Layout.fillWidth: true
                    font: win.font
                    color: win.textColor
                    selectByMouse: true
                    readonly property var cell: win.hasCursor ? win.cellAt(win.curRow, win.curCol) : null
                    readonly property string source: cell ? cell.s : ""
                    readonly property bool locked: win.sheet.isConsts && win.curCol !== 1
                    readOnly: locked || !cell
                    placeholderText: cell ? ""
                        : win.selKind === "row" ? (win.selTop === win.selBottom ? "Whole row selected" : "Whole rows selected")
                        : win.selKind === "column" ? (win.selLeft === win.selRight ? "Whole column selected" : "Whole columns selected")
                        : win.selKind === "all" ? "Whole sheet selected"
                        : "No cell selected"
                    background: Item {}
                    onSourceChanged: if (!activeFocus) text = source
                    onActiveFocusChanged: {
                        if (activeFocus) {
                            win.editor = formulaField;
                        } else {
                            if (win.editor === formulaField)
                                win.editor = null;
                            text = source;
                        }
                    }
                    onAccepted: {
                        if (!readOnly)
                            win.commit(text, 0, 0);
                        text = Qt.binding(function() { return source; });
                        text = source;
                    }
                    Keys.onEscapePressed: {
                        text = source;
                        grid.forceActiveFocus();
                    }
                }

                Text {
                    // For a cell of the column, or the whole of it.
                    readonly property var column: win.hasCursor || (win.selKind === "column" && win.selLeft === win.selRight)
                        ? win.sheet.columns[win.curCol] : undefined
                    visible: !!column && column.computed && !win.sheet.isConsts
                    text: "formula column"
                    color: win.accentColor
                    font: win.font
                }
                Text {
                    readonly property var column: win.hasCursor || (win.selKind === "column" && win.selLeft === win.selRight)
                        ? win.sheet.columns[win.curCol] : undefined
                    visible: !!column && column.declared && !win.sheet.isConsts
                    text: "typed column"
                    color: win.textColor
                    font: win.font
                }

                // What the column accepts, in the locale's way of writing it.
                Text {
                    readonly property var column: win.hasCursor ? win.sheet.columns[win.curCol] : undefined
                    readonly property string hint: column && column.type && !win.sheet.isConsts
                        ? file.entryHint(column.type) : ""
                    visible: hint.length > 0
                    text: hint
                    color: win.mutedColor
                    font.family: win.font.family
                    font.features: win.font.features
                    font.pixelSize: win.fontSize - 1
                    Layout.maximumWidth: win.width * 0.45
                    elide: Text.ElideRight
                }
            }

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: win.lineColor
            }
        }

        // The grid.
        FocusScope {
            id: grid
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !win.showSource
            focus: true

            Keys.onPressed: function(event) {
                if (win.editing)
                    return;
                var ctrl = event.modifiers & Qt.ControlModifier;
                var shift = event.modifiers & Qt.ShiftModifier;
                var page = Math.max(1, Math.floor(rowsView.height / win.rowHeight) - 1);
                event.accepted = true;
                switch (event.key) {
                case Qt.Key_Up: win.arrow(-1, 0, ctrl, shift); break;
                case Qt.Key_Down: win.arrow(1, 0, ctrl, shift); break;
                case Qt.Key_Left: win.arrow(0, -1, ctrl, shift); break;
                case Qt.Key_Right: win.arrow(0, 1, ctrl, shift); break;
                case Qt.Key_PageUp: win.selectCell(win.curRow - page, win.curCol, shift); break;
                case Qt.Key_PageDown: win.selectCell(win.curRow + page, win.curCol, shift); break;
                case Qt.Key_Home: win.selectCell(ctrl ? 0 : win.curRow, 0, shift); break;
                case Qt.Key_End: win.selectCell(ctrl ? win.sheet.rows.length - 1 : win.curRow, win.sheet.columns.length - 1, shift); break;
                case Qt.Key_Tab: win.selectCell(win.curRow, win.curCol + 1, false); break;
                case Qt.Key_Backtab: win.selectCell(win.curRow, win.curCol - 1, false); break;
                case Qt.Key_Return:
                case Qt.Key_Enter:
                    // A row goes in beside whole rows, and nothing else.
                    if (ctrl) { if (win.selKind === "row") win.insertRows(!shift, win.selBottom - win.selTop + 1); }
                    else win.startEditing(null);
                    break;
                case Qt.Key_Delete:
                    if (ctrl) {
                        // The whole of what is selected goes: a sheet, its
                        // columns or its rows. Cells are only cleared.
                        if (win.selKind === "all") win.deleteSheet();
                        else if (win.selKind === "column") win.deleteColumns();
                        else if (win.selKind === "row") win.deleteRows();
                    } else {
                        win.clearSelection();
                    }
                    break;
                case Qt.Key_Backspace: win.clearSelection(); break;
                case Qt.Key_Escape: win.deselect(); break;
                case Qt.Key_Menu: win.keyMenu(); break;
                default:
                    if (ctrl && (event.key === Qt.Key_BraceRight || (shift && event.key === Qt.Key_BracketRight))) {
                        // The brackets again, should the shortcut not see them.
                        win.switchTab(win.tab + 1);
                    } else if (ctrl && (event.key === Qt.Key_BraceLeft || (shift && event.key === Qt.Key_BracketLeft))) {
                        win.switchTab(win.tab - 1);
                    } else if (ctrl && event.key === Qt.Key_Period) {
                        win.keyMenu();
                    } else if (ctrl && event.key === Qt.Key_C) {
                        win.copySelection();
                    } else if (ctrl && event.key === Qt.Key_X) {
                        win.cutSelection();
                    } else if (ctrl && event.key === Qt.Key_V) {
                        if (event.modifiers & Qt.AltModifier) {
                            if (win.isBlock) win.pasteTransposed();
                        }
                        else if (shift)
                            win.pasteValues();
                        else
                            win.pasteSelection();
                    } else if (ctrl && event.key === Qt.Key_A) {
                        win.selectAll();
                    } else if (!ctrl && !(event.modifiers & (Qt.AltModifier | Qt.MetaModifier))
                               && event.text.length === 1 && event.text.charCodeAt(0) >= 32
                               && event.text.charCodeAt(0) !== 127) {
                        // Typing replaces the cell.
                        win.startEditing(event.text);
                    } else {
                        event.accepted = false;
                    }
                }
            }

            Text {
                anchors.centerIn: parent
                visible: win.tabs.length === 0
                text: "This file has no sheets yet.\nUse + Sheet below to add one."
                horizontalAlignment: Text.AlignHCenter
                color: win.mutedColor
                font: win.font
            }

            Flickable {
                id: hflick
                anchors.fill: parent
                anchors.rightMargin: vbar.width
                anchors.bottomMargin: hbar.height
                contentWidth: win.gutterWidth + win.totalWidth
                contentHeight: height
                flickableDirection: Flickable.HorizontalFlick
                boundsBehavior: Flickable.StopAtBounds
                clip: true
                visible: win.tabs.length > 0
                ScrollBar.horizontal: hbar

                Column {
                    // Column headers: name over type.
                    Row {
                        height: win.headerHeight
                        // The corner: the whole sheet.
                        Rectangle {
                            width: win.gutterWidth
                            height: win.headerHeight
                            color: win.selKind === "all"
                                ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.16)
                                : win.panelColor
                            MouseArea {
                                anchors.fill: parent
                                enabled: !win.sheet.isConsts
                                onClicked: {
                                    win.stopEditing();
                                    win.selectAll();
                                }
                            }
                        }
                        Repeater {
                            model: win.sheet.columns.length
                            Rectangle {
                                id: headCell
                                required property int index
                                readonly property var column: win.sheet.columns[index]
                                // The name is being typed over, in place.
                                property bool renaming: false
                                width: win.colWidths[index] || 0
                                height: win.headerHeight
                                // Cut and waiting to be put down.
                                opacity: win.grabbedHere && !win.grabbed.rows && win.isGrabbed(-1, index) ? 0.4 : 1
                                color: index >= win.selLeft && index <= win.selRight
                                    ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.16)
                                    : win.panelColor
                                Column {
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: 7
                                    width: parent.width - 14
                                    Text {
                                        width: parent.width
                                        text: headCell.column ? headCell.column.name : ""
                                        color: win.textColor
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.fontSize
                                        font.bold: true
                                        elide: Text.ElideRight
                                    }
                                    // The type: marked `:=` in a formula column, and
                                    // plain rather than muted where it is declared.
                                    Text {
                                        readonly property bool computed: !!headCell.column && headCell.column.computed && !win.sheet.isConsts
                                        readonly property bool declared: !!headCell.column && headCell.column.declared
                                        readonly property color typeColor: declared ? win.textColor : win.mutedColor
                                        width: parent.width
                                        textFormat: Text.StyledText
                                        text: !headCell.column ? ""
                                            : (computed ? "<font color=\"" + win.accentColor + "\">:=</font> " : "")
                                              + "<font color=\"" + typeColor + "\">" + headCell.column.type + "</font>"
                                        color: win.mutedColor
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.fontSize - 3
                                        elide: Text.ElideRight
                                    }
                                }
                                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.lineColor }
                                MouseArea {
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    // The formula of a formula column, in full.
                                    readonly property bool hasFormula: !!headCell.column && headCell.column.computed && !win.sheet.isConsts
                                    ToolTip.visible: containsMouse && !cellMenu.visible && hasFormula
                                    ToolTip.delay: 400
                                    ToolTip.text: hasFormula ? headCell.column.name + " := " + headCell.column.formula : ""
                                    onClicked: function(mouse) {
                                        if (mouse.button === Qt.RightButton) {
                                            win.openMenu(-1, headCell.index);
                                            return;
                                        }
                                        // Select the whole column.
                                        win.stopEditing();
                                        win.byRow = false;
                                        win.byCol = true;
                                        // The cursor waits in the first cell.
                                        win.anchorRow = Math.max(0, win.sheet.rows.length - 1);
                                        win.anchorCol = (mouse.modifiers & Qt.ShiftModifier) ? win.anchorCol : headCell.index;
                                        win.curRow = 0;
                                        win.curCol = headCell.index;
                                    }
                                    // Rename the column where its name is.
                                    onDoubleClicked: function(mouse) {
                                        if (mouse.button !== Qt.LeftButton || win.sheet.isConsts || !headCell.column)
                                            return;
                                        win.notice = "";
                                        headCell.renaming = true;
                                    }
                                }
                                Loader {
                                    anchors.fill: parent
                                    active: headCell.renaming
                                    sourceComponent: TextField {
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.fontSize
                                        font.bold: true
                                        color: win.textColor
                                        leftPadding: 7
                                        rightPadding: 7
                                        topPadding: 0
                                        bottomPadding: 0
                                        verticalAlignment: TextInput.AlignVCenter
                                        selectByMouse: true
                                        background: Rectangle {
                                            color: win.pageColor
                                            border.width: 2
                                            border.color: win.accentColor
                                        }
                                        property bool done: false
                                        Component.onCompleted: {
                                            text = headCell.column.name;
                                            forceActiveFocus();
                                            selectAll();
                                        }
                                        // A name that is refused is said why, and the old one stays.
                                        function finish(keep) {
                                            if (done)
                                                return;
                                            done = true;
                                            var name = text.trim();
                                            if (keep && name.length > 0 && name !== headCell.column.name)
                                                win.keeping(0, 0, function() { win.notice = file.renameColumn(win.tab, headCell.index, name); });
                                            headCell.renaming = false;
                                            grid.forceActiveFocus();
                                        }
                                        onAccepted: finish(true)
                                        Keys.onEscapePressed: finish(false)
                                        onActiveFocusChanged: if (!activeFocus) finish(true)
                                    }
                                }
                            }
                        }
                    }

                    ListView {
                        id: rowsView
                        width: hflick.contentWidth
                        height: hflick.height - win.headerHeight
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        model: win.sheet.rows.length
                        reuseItems: true
                        // The grid moves the selection. After an edit the
                        // focus is left inside a row, and the list would
                        // take Up and Down for itself.
                        keyNavigationEnabled: false
                        ScrollBar.vertical: vbar

                        delegate: Row {
                            id: rowItem
                            required property int index
                            height: win.rowHeight

                            // The row number.
                            Rectangle {
                                width: win.gutterWidth
                                height: win.rowHeight
                                color: rowItem.index >= win.selTop && rowItem.index <= win.selBottom
                                    ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.16)
                                    : win.panelColor
                                Text {
                                    anchors.centerIn: parent
                                    text: rowItem.index
                                    color: win.mutedColor
                                    font.family: win.font.family
                                    font.features: win.font.features
                                    font.pixelSize: win.fontSize - 2
                                }
                                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                MouseArea {
                                    anchors.fill: parent
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    onClicked: function(mouse) {
                                        if (mouse.button === Qt.RightButton) {
                                            win.openMenu(rowItem.index, -1);
                                            return;
                                        }
                                        // Select the whole row.
                                        win.stopEditing();
                                        win.byRow = true;
                                        win.byCol = false;
                                        // The cursor waits in the first cell.
                                        win.anchorCol = Math.max(0, win.sheet.columns.length - 1);
                                        win.anchorRow = (mouse.modifiers & Qt.ShiftModifier) ? win.anchorRow : rowItem.index;
                                        win.curCol = 0;
                                        win.curRow = rowItem.index;
                                    }
                                }
                            }

                            Repeater {
                                model: win.sheet.columns.length
                                Rectangle {
                                    id: cellItem
                                    required property int index
                                    readonly property int row: rowItem.index
                                    readonly property var cell: win.cellAt(row, index)
                                    readonly property var column: win.sheet.columns[index]
                                    readonly property bool current: win.hasCursor && row === win.curRow && index === win.curCol
                                    readonly property bool selected: row >= win.selTop && row <= win.selBottom
                                        && index >= win.selLeft && index <= win.selRight
                                    // The cursor is outlined only when it is the whole
                                    // selection: a block is shown as a block.
                                    readonly property bool alone: current
                                        && win.selTop === win.selBottom && win.selLeft === win.selRight
                                    // Copied or cut, and not yet done with.
                                    readonly property bool marked: win.isMarked(row, index)
                                    // Where the arrows are pointing a formula reference.
                                    readonly property bool picked: row === win.pickRow && index === win.pickCol
                                    width: win.colWidths[index] || 0
                                    height: win.rowHeight
                                    opacity: win.grabbedHere && win.isGrabbed(row, index) ? 0.4 : 1
                                    color: (selected && !alone) || marked
                                        ? Qt.rgba(win.selectionFill.r, win.selectionFill.g, win.selectionFill.b, 0.55)
                                        : column && column.computed ? win.computedTint : "transparent"

                                    Text {
                                        id: cellText
                                        anchors.fill: parent
                                        anchors.leftMargin: 7
                                        anchors.rightMargin: 7
                                        visible: !(win.editing && cellItem.current)
                                        // Text written bare is Markdown; in quotes it is as it is.
                                        readonly property bool marked: !!cellItem.cell && cellItem.cell.h !== undefined
                                        text: marked ? cellItem.cell.h : cellItem.cell ? cellItem.cell.d : ""
                                        textFormat: marked ? Text.StyledText : Text.PlainText
                                        linkColor: win.accentColor
                                        clip: marked
                                        color: cellItem.cell && cellItem.cell.e ? win.errorColor : win.textColor
                                        opacity: cellItem.cell && cellItem.cell.f && cellItem.column && !cellItem.column.computed ? 0.92 : 1
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.fontSize
                                        font.italic: !!cellItem.cell && cellItem.cell.f && !!cellItem.column && !cellItem.column.computed
                                        verticalAlignment: Text.AlignVCenter
                                        horizontalAlignment: cellItem.cell && cellItem.cell.n ? Text.AlignRight : Text.AlignLeft
                                        elide: Text.ElideRight
                                    }

                                    Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.lineColor }
                                    // The outline of what was copied or cut.
                                    Rectangle { visible: cellItem.marked && cellItem.row === win.marked.top; width: parent.width; height: 1; color: win.accentColor }
                                    Rectangle { visible: cellItem.marked && cellItem.row === win.marked.bottom; anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.accentColor }
                                    Rectangle { visible: cellItem.marked && cellItem.index === win.marked.left; width: 1; height: parent.height; color: win.accentColor }
                                    Rectangle { visible: cellItem.marked && cellItem.index === win.marked.right; anchors.right: parent.right; width: 1; height: parent.height; color: win.accentColor }
                                    Rectangle {
                                        anchors.fill: parent
                                        visible: cellItem.alone
                                        color: "transparent"
                                        border.width: 2
                                        border.color: win.accentColor
                                    }
                                    Rectangle {
                                        anchors.fill: parent
                                        visible: cellItem.picked
                                        color: "transparent"
                                        border.width: 2
                                        border.color: win.accentColor
                                        opacity: 0.6
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        property bool picking: false
                                        // Why the cell failed, or a number shown
                                        // with fewer digits than it has, in full,
                                        // which grouping its digits does not make
                                        // it; or what is cut off at the cell's edge.
                                        readonly property string full: cellItem.cell && cellItem.cell.x
                                            && cellItem.cell.x !== cellItem.cell.d.replace(/,/g, "") ? cellItem.cell.x : ""
                                        readonly property string tip: cellItem.cell
                                            ? cellItem.cell.e || full || (cellText.truncated ? cellItem.cell.d : "")
                                            : ""
                                        ToolTip.visible: containsMouse && !cellMenu.visible && tip.length > 0
                                        ToolTip.delay: 400
                                        ToolTip.text: tip
                                        // A link in the cell under the pointer, opened with Ctrl and a click.
                                        readonly property string link: containsMouse
                                            ? cellText.linkAt(mouseX - cellText.x, mouseY - cellText.y) : ""
                                        cursorShape: link.length > 0 ? Qt.PointingHandCursor : Qt.ArrowCursor
                                        onPressed: function(mouse) {
                                            if (mouse.button === Qt.RightButton) {
                                                win.openMenu(cellItem.row, cellItem.index);
                                                return;
                                            }
                                            if (link.length > 0 && (mouse.modifiers & Qt.ControlModifier)) {
                                                win.openLink(link);
                                                return;
                                            }
                                            // While a formula is typed, a click picks this cell.
                                            picking = win.canPick();
                                            if (picking)
                                                return;
                                            if (win.editing)
                                                win.stopEditing();
                                            grid.forceActiveFocus();
                                            win.selectCell(cellItem.row, cellItem.index, mouse.modifiers & Qt.ShiftModifier);
                                        }
                                        onClicked: function(mouse) {
                                            if (picking && mouse.button === Qt.LeftButton)
                                                win.pickCell(cellItem.row, cellItem.index, mouse.modifiers & Qt.ShiftModifier);
                                        }
                                        onPositionChanged: function(mouse) {
                                            if (picking || !(pressedButtons & Qt.LeftButton))
                                                return;
                                            // Drag to extend the selection.
                                            var p = mapToItem(rowsView.contentItem, mouse.x, mouse.y);
                                            var r = Math.floor(p.y / win.rowHeight);
                                            var x = p.x - win.gutterWidth;
                                            var c = 0;
                                            while (c < win.colWidths.length - 1 && x > win.colWidths[c]) {
                                                x -= win.colWidths[c];
                                                c++;
                                            }
                                            win.selectCell(r, c, true);
                                        }
                                        onDoubleClicked: function(mouse) {
                                            if (mouse.button === Qt.LeftButton && !picking)
                                                win.startEditing(null);
                                        }
                                    }

                                    Loader {
                                        anchors.fill: parent
                                        active: win.editing && cellItem.current
                                        sourceComponent: TextField {
                                            font.family: win.font.family
                                            font.features: win.font.features
                                            font.pixelSize: win.fontSize
                                            color: win.textColor
                                            leftPadding: 7
                                            rightPadding: 7
                                            topPadding: 0
                                            bottomPadding: 0
                                            verticalAlignment: TextInput.AlignVCenter
                                            selectByMouse: true
                                            background: Rectangle {
                                                color: win.pageColor
                                                border.width: 2
                                                border.color: win.accentColor
                                            }
                                            property bool done: false
                                            Component.onDestruction: if (win.editor === this) win.editor = null
                                            Component.onCompleted: {
                                                win.editor = this;
                                                // Typing started the edit: replace. Otherwise edit the source.
                                                text = win.editSeed !== null && win.editSeed !== undefined && win.editSeed.length > 0
                                                    ? win.editSeed : (cellItem.cell ? cellItem.cell.s : "");
                                                forceActiveFocus();
                                                if (!(win.editSeed && win.editSeed.length > 0))
                                                    selectAll();
                                            }
                                            function finish(dRow, dCol) {
                                                if (done)
                                                    return;
                                                done = true;
                                                win.commit(text, dRow, dCol);
                                            }
                                            onAccepted: finish(1, 0)
                                            Keys.onTabPressed: finish(0, 1)
                                            Keys.onBacktabPressed: finish(0, -1)
                                            Keys.onEscapePressed: {
                                                done = true;
                                                win.stopEditing();
                                            }
                                            onActiveFocusChanged: if (!activeFocus) finish(0, 0)
                                            // While what is typed is a formula, the arrows pick
                                            // a cell's reference instead of moving in the text.
                                            Keys.priority: Keys.BeforeItem
                                            Keys.onPressed: function(event) {
                                                var absolute = (event.modifiers & Qt.ShiftModifier) !== 0;
                                                switch (event.key) {
                                                case Qt.Key_Up: event.accepted = win.pickArrow(-1, 0, absolute); break;
                                                case Qt.Key_Down: event.accepted = win.pickArrow(1, 0, absolute); break;
                                                case Qt.Key_Left: event.accepted = win.pickArrow(0, -1, absolute); break;
                                                case Qt.Key_Right: event.accepted = win.pickArrow(0, 1, absolute); break;
                                                default:
                                                    win.stopPicking();
                                                    event.accepted = false;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // A click beside the columns or below the rows deselects.
            MouseArea {
                id: beside
                visible: hflick.visible
                x: Math.min(hflick.width, win.gutterWidth + win.totalWidth - hflick.contentX)
                width: hflick.width - x
                height: hflick.height
                onPressed: {
                    win.stopEditing();
                    win.deselect();
                }
            }
            MouseArea {
                visible: hflick.visible
                y: Math.min(hflick.height, win.headerHeight + win.sheet.rows.length * win.rowHeight - rowsView.contentY)
                width: beside.x
                height: hflick.height - y
                onPressed: {
                    win.stopEditing();
                    win.deselect();
                }
            }

            ScrollBar {
                id: vbar
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.topMargin: win.headerHeight
                anchors.bottom: parent.bottom
                anchors.bottomMargin: hbar.height
                policy: ScrollBar.AsNeeded
            }

            ScrollBar {
                id: hbar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.rightMargin: vbar.width
                anchors.bottom: parent.bottom
                orientation: Qt.Horizontal
                policy: ScrollBar.AsNeeded
            }
        }

        // The source: the file as it is written, to read and to edit. An
        // edit is taken when the source is left, saved or closed.
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: win.showSource
            spacing: 0

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: win.fontSize * 2 + 10
                color: win.panelColor

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 6
                    spacing: 10

                    Text {
                        text: "Source: " + file.fileName
                        color: win.mutedColor
                        font: win.font
                        elide: Text.ElideMiddle
                        Layout.fillWidth: true
                    }
                    FooterButton { label: "\u00d7"; onClicked: win.viewSource(false) }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: win.lineColor
                }
            }

            Flickable {
                id: sourceFlick
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }

                TextArea.flickable: TextArea {
                    id: sourceText
                    // `win.snap` is read so that an edit rewrites the text.
                    text: win.showSource && win.snap ? file.sourceText() : ""
                    selectByMouse: true
                    wrapMode: TextEdit.NoWrap
                    textFormat: TextEdit.PlainText
                    color: win.textColor
                    font: win.font
                    leftPadding: 12
                    rightPadding: 12
                    background: null
                    Keys.onEscapePressed: win.viewSource(false)
                    // Another window, or a dialog, sees the file as edited.
                    onActiveFocusChanged: if (!activeFocus) win.applySource()
                }
            }
        }

        // Footer: sheet tabs on the left, actions and status on the right.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: win.fontSize * 2 + 10
            color: win.panelColor

            Rectangle { width: parent.width; height: 1; color: win.lineColor }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 12
                spacing: 2

                Repeater {
                    model: win.tabs.length
                    FooterButton {
                        required property int index
                        label: win.tabs[index].name
                        active: index === win.tab && !win.showSource
                        onClicked: {
                            win.viewSource(false);
                            win.switchTab(index);
                        }
                        onRightClicked: {
                            tabMenu.index = index;
                            tabMenu.popup();
                        }
                    }
                }
                FooterButton { label: "+ Sheet"; quiet: true; onClicked: promptDialog.ask("sheet") }

                Item { Layout.fillWidth: true }

                // What a selection of several cells adds up to.
                Text {
                    readonly property bool several: !win.sheet.isConsts && win.tabs.length > 0
                        && (win.selTop !== win.selBottom || win.selLeft !== win.selRight)
                    readonly property bool moving: win.grabbedHere
                    // `win.snap` is read so that an edit recalculates it.
                    text: win.notice.length > 0 ? win.notice
                        : moving ? "Select a " + (win.grabbed.rows ? "row" : "column") + " and paste to move here (Esc cancels)"
                        : several && win.snap
                        ? file.selectionSummary(win.tab, win.selTop, win.selLeft, win.selBottom, win.selRight)
                        : ""
                    visible: text.length > 0
                    color: win.notice.length > 0 ? win.errorColor : win.mutedColor
                    elide: Text.ElideRight
                    Layout.maximumWidth: 520
                    font.family: win.font.family
                    font.features: win.font.features
                    font.pixelSize: win.fontSize - 2
                    rightPadding: 10
                }

                Text {
                    visible: file.status.length > 0
                    text: file.status
                    color: win.mutedColor
                    font.family: win.font.family
                    font.features: win.font.features
                    font.pixelSize: win.fontSize - 2
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: 320
                }
                FooterButton {
                    readonly property int count: file.notes.length > 0 ? file.notes.split("\n").length : 0
                    visible: count > 0
                    label: count + (count === 1 ? " note" : " notes")
                    onClicked: notesDialog.open()
                }
                FooterButton {
                    visible: win.snap.problems.length > 0
                    label: win.snap.problems.length + (win.snap.problems.length === 1 ? " problem" : " problems")
                    textColor: win.errorColor
                    onClicked: problemsDialog.open()
                }
                FooterButton { label: "+ Row"; quiet: true; enabled: !win.sheet.isConsts && win.tabs.length > 0; onClicked: win.insertRow(true) }
                FooterButton { label: "+ Column"; quiet: true; enabled: !win.sheet.isConsts && win.tabs.length > 0; onClicked: promptDialog.ask("column") }
                FooterButton { label: "+ Formula"; quiet: true; enabled: !win.sheet.isConsts && win.tabs.length > 0; onClicked: promptDialog.ask("computed") }
                FooterButton { label: "+ Constant"; quiet: true; onClicked: promptDialog.ask("const") }
                FooterButton { label: "fn()"; quiet: true; onClicked: functionsDialog.open() }
                FooterButton { label: "</>"; quiet: true; active: win.showSource; onClicked: win.viewSource(!win.showSource) }
                FooterButton { label: "?"; quiet: true; onClicked: helpDialog.open() }
            }
        }
    }

    // The entries of the right-click menu, close set.
    component MenuEntry: MenuItem {
        // An entry that does not apply takes no room, and the arrow keys
        // pass it by: so each is also disabled while it is hidden.
        height: visible ? implicitHeight : 0
        activeFocusOnTab: visible
        implicitHeight: Math.round(win.fontSize * 1.75)
        topPadding: 0
        bottomPadding: 0
        leftPadding: 12
        rightPadding: 12
        font.family: win.font.family
        font.features: win.font.features
        font.pixelSize: win.fontSize - 1
    }
    // A type to declare the selected columns as: ticked if they are.
    component TypeEntry: MenuEntry {
        required property string typeName
        text: (typeMenu.current === typeName ? "\u2713 " : "  ") + typeName
        onTriggered: win.setColumnTypes(typeName)
    }
    component MenuRule: MenuSeparator {
        height: visible ? implicitHeight : 0
        topPadding: 3
        bottomPadding: 3
    }

    component FooterButton: Rectangle {
        id: button
        property string label
        property bool active: false
        property bool quiet: false
        property color textColor: win.textColor
        signal clicked()
        signal rightClicked()
        implicitWidth: buttonText.implicitWidth + 22
        implicitHeight: win.fontSize * 2
        Layout.alignment: Qt.AlignVCenter
        radius: 4
        opacity: enabled ? 1 : 0.35
        color: active ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.22)
            : area.containsMouse ? Qt.rgba(win.textColor.r, win.textColor.g, win.textColor.b, 0.08) : "transparent"
        Text {
            id: buttonText
            anchors.centerIn: parent
            text: button.label
            color: button.quiet ? win.mutedColor : button.textColor
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.fontSize - 1
            font.bold: button.active
        }
        MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton) {
                    button.rightClicked();
                    return;
                }
                button.clicked();
                grid.forceActiveFocus();
            }
        }
    }

    // ---- context menu ------------------------------------------------------

    Menu {
        id: cellMenu
        readonly property bool editable: !win.sheet.isConsts
        readonly property bool hasRows: editable && win.sheet.rows.length > 0
        readonly property int rowCount: win.selBottom - win.selTop + 1
        readonly property int columnCount: win.selRight - win.selLeft + 1
        // Which of its sections fit what is selected: whole rows or whole
        // columns. Cells are neither.
        readonly property bool rows: win.selKind === "row"
        readonly property bool columns: win.selKind === "column"
        readonly property bool whole: win.selKind === "all" && editable
        // An entry may have opened a dialog, which keeps the keyboard.
        onClosed: {
            if (promptDialog.visible)
                promptDialog.takeFocus();
            else if (!typeMenu.visible)
                grid.forceActiveFocus();
        }
        // Close set, to fit a long menu on a small window.
        topPadding: 4
        bottomPadding: 4
        width: Math.round(win.fontSize * 15)

        MenuEntry {
            text: "Cut"
            enabled: cellMenu.hasRows || win.grabKind.length > 0
            onTriggered: win.cutSelection()
        }
        MenuEntry { text: "Copy"; enabled: win.sheet.rows.length > 0; onTriggered: win.copySelection() }
        MenuEntry {
            text: "Paste"
            enabled: win.sheet.rows.length > 0 || win.grabbedHere
            onTriggered: win.pasteSelection()
        }
        MenuEntry { text: "Paste values"; enabled: cellMenu.hasRows; onTriggered: win.pasteValues() }
        MenuEntry { text: "Paste transposed"; enabled: cellMenu.hasRows && win.isBlock; onTriggered: win.pasteTransposed() }
        MenuEntry { text: "Insert cells"; enabled: cellMenu.editable && (cellMenu.rows || cellMenu.columns); onTriggered: win.insertCopied() }
        MenuEntry { text: "Clear"; enabled: cellMenu.hasRows; onTriggered: win.clearSelection() }
        // The sheet, when the whole of it is selected.
        MenuRule { visible: cellMenu.whole }
        MenuEntry {
            text: "Insert sheet before"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: win.newSheet(false)
        }
        MenuEntry {
            text: "Insert sheet after"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: win.newSheet(true)
        }
        MenuEntry {
            text: "Rename sheet…"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: promptDialog.ask("renameSheet")
        }
        MenuEntry {
            text: "Delete sheet"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: win.deleteSheet()
        }
        // Rows, when whole rows are selected.
        MenuRule { visible: cellMenu.rows }
        MenuEntry {
            text: cellMenu.rowCount > 1 ? "Insert " + cellMenu.rowCount + " rows above" : "Insert row above"
            visible: cellMenu.rows
            enabled: visible && cellMenu.editable
            onTriggered: win.insertRows(false, cellMenu.rowCount)
        }
        MenuEntry {
            text: cellMenu.rowCount > 1 ? "Insert " + cellMenu.rowCount + " rows below" : "Insert row below"
            visible: cellMenu.rows
            enabled: visible && cellMenu.editable
            onTriggered: win.insertRows(true, cellMenu.rowCount)
        }
        MenuEntry {
            text: "Insert rows…"
            visible: cellMenu.rows
            enabled: visible && cellMenu.editable
            onTriggered: promptDialog.ask("rows")
        }
        MenuEntry {
            text: cellMenu.rowCount > 1 ? "Delete " + cellMenu.rowCount + " rows" : "Delete row"
            visible: cellMenu.rows
            enabled: visible && cellMenu.hasRows
            onTriggered: win.deleteRows()
        }
        // Columns, when whole columns are selected. A column added
        // from the footer goes at the end.
        MenuRule { visible: cellMenu.columns }
        MenuEntry {
            text: "Insert column before"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable
            onTriggered: promptDialog.ask("column", win.selLeft)
        }
        MenuEntry {
            text: "Insert column after"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable
            onTriggered: promptDialog.ask("column", win.selRight + 1)
        }
        MenuEntry {
            text: "Rename column…"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.selLeft === win.selRight && win.sheet.columns.length > 0
            onTriggered: promptDialog.ask("rename")
        }
        MenuEntry {
            text: "Set type\u2026"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.sheet.columns.length > 0
            onTriggered: {
                typeMenu.current = win.sharedType(true);
                typeMenu.suggested = win.sharedType(false);
                // Where this menu was.
                typeMenu.popup(cellMenu.parent, cellMenu.x, cellMenu.y);
            }
        }
        MenuEntry {
            text: "Clear type"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.anyDeclared()
            onTriggered: win.setColumnTypes("")
        }
        MenuEntry {
            text: cellMenu.columnCount > 1 ? "Delete " + cellMenu.columnCount + " columns" : "Delete column"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.sheet.columns.length > 1
            onTriggered: win.deleteColumns()
        }
    }

    // The types a column may be declared as, from `Set type`: the one it
    // has is ticked.
    Menu {
        id: typeMenu
        property string current: ""
        // The type the columns have now, declared or taken from their
        // cells: the arrow keys start from it.
        property string suggested: ""
        onOpened: {
            for (var i = 0; i < count; i++) {
                if (itemAt(i).typeName === suggested)
                    currentIndex = i;
            }
        }
        onClosed: grid.forceActiveFocus()
        topPadding: 4
        bottomPadding: 4
        width: Math.round(win.fontSize * 11)

        TypeEntry { typeName: "Int" }
        TypeEntry { typeName: "Decimal" }
        TypeEntry { typeName: "Fraction" }
        TypeEntry { typeName: "Percent" }
        TypeEntry { typeName: "Num" }
        TypeEntry { typeName: "Complex" }
        TypeEntry { typeName: "Text" }
        TypeEntry { typeName: "Bool" }
        TypeEntry { typeName: "Date" }
        TypeEntry { typeName: "Time" }
        TypeEntry { typeName: "DateTime" }
    }

    // Right-click on a sheet's tab.
    Menu {
        id: tabMenu
        property int index: 0
        onClosed: grid.forceActiveFocus()
        topPadding: 4
        bottomPadding: 4
        width: Math.round(win.fontSize * 15)

        MenuEntry { text: "Open in new window"; onTriggered: win.newWindow(tabMenu.index) }
    }

    // ---- dialogs -----------------------------------------------------------

    Dialogs.FileDialog {
        id: openDialog
        title: "Open file"
        fileMode: Dialogs.FileDialog.OpenFile
        // A workbook, a CSV file or JSON is imported: it opens as a new file.
        nameFilters: ["Files, workbooks, CSV and JSON (*.omx *.xlsx *.csv *.json)", "Omasheet (*.omx)", "All files (*)"]
        onAccepted: {
            win.pendingUrl = selectedFile;
            win.run("openUrl");
        }
    }

    Dialogs.FileDialog {
        id: saveDialog
        title: "Save file"
        fileMode: Dialogs.FileDialog.SaveFile
        nameFilters: ["Omasheet (*.omx)"]
        defaultSuffix: "omx"
        onAccepted: {
            win.applySource();
            if (file.saveUrl(selectedFile) && win.pendingAction.length > 0) {
                var action = win.pendingAction;
                win.pendingAction = "";
                win.run(action);
            }
        }
        onRejected: win.pendingAction = ""
    }

    // What to export as. Asked here, as not every file chooser offers a
    // choice of file types.
    Dialog {
        font.pixelSize: win.dialogSize
        id: exportChoice
        anchors.centerIn: parent
        modal: true
        focus: true
        width: 460
        title: "Export"
        readonly property bool oneSheet: !win.sheet.isConsts && win.tabs.length > 0
        onClosed: grid.forceActiveFocus()
        onOpened: workbookButton.forceActiveFocus()
        function choose(kind) {
            close();
            win.applySource();
            exportDialog.kind = kind;
            // Beside the file, under its name.
            var name = win.sheet.isConsts ? "" : win.sheet.name;
            exportDialog.selectedFile = file.exportSuggestion(kind, name, exportDialog.currentFolder);
            exportDialog.open();
        }
        Shortcut { sequence: "X"; enabled: exportChoice.visible; onActivated: exportChoice.choose("xlsx") }
        Shortcut { sequence: "C"; enabled: exportChoice.visible && exportChoice.oneSheet; onActivated: exportChoice.choose("csv") }
        Shortcut { sequence: "J"; enabled: exportChoice.visible; onActivated: exportChoice.choose("json") }
        ColumnLayout {
            width: parent.width
            spacing: 10
            Button {
                id: workbookButton
                Layout.fillWidth: true
                text: "Workbook (.xlsx): every sheet    X"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onClicked: exportChoice.choose("xlsx")
                Keys.onReturnPressed: clicked()
                Keys.onEnterPressed: clicked()
            }
            Button {
                Layout.fillWidth: true
                enabled: exportChoice.oneSheet
                text: "CSV (.csv): " + (exportChoice.oneSheet ? "this sheet" : "show a sheet first") + "    C"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onClicked: exportChoice.choose("csv")
                Keys.onReturnPressed: clicked()
                Keys.onEnterPressed: clicked()
            }
            Button {
                Layout.fillWidth: true
                text: "JSON (.json): every sheet    J"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onClicked: exportChoice.choose("json")
                Keys.onReturnPressed: clicked()
                Keys.onEnterPressed: clicked()
            }
        }
        standardButtons: Dialog.Cancel
    }

    // The calculated file as a workbook or JSON, or the sheet shown as CSV.
    Dialogs.FileDialog {
        id: exportDialog
        property string kind: "xlsx"
        title: kind === "csv" ? "Export " + win.sheet.name + " as CSV"
             : kind === "json" ? "Export as JSON" : "Export as a workbook"
        fileMode: Dialogs.FileDialog.SaveFile
        nameFilters: kind === "csv" ? ["CSV (*.csv)"] : kind === "json" ? ["JSON (*.json)"] : ["Workbook (*.xlsx)"]
        defaultSuffix: kind
        onAccepted: {
            var name = win.sheet.isConsts ? "" : win.sheet.name;
            if (file.exportUrl(selectedFile, kind, name) && file.notes.length > 0)
                notesDialog.open();
        }
    }

    // What an import or export changed, rounded or left behind.
    Dialog {
        font.pixelSize: win.dialogSize
        id: notesDialog
        anchors.centerIn: parent
        modal: true
        width: Math.min(win.width - 80, 760)
        title: file.status
        standardButtons: Dialog.Close
        onClosed: grid.forceActiveFocus()
        contentItem: ListView {
            implicitHeight: Math.min(contentHeight, 360)
            clip: true
            spacing: 8
            model: file.notes.length > 0 ? file.notes.split("\n") : []
            delegate: Text {
                required property string modelData
                width: ListView.view.width
                text: modelData
                color: win.textColor
                wrapMode: Text.Wrap
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize - 1
            }
        }
    }

    // Why something could not be done, where it must not be missed.
    Dialog {
        font.pixelSize: win.dialogSize
        id: errorDialog
        anchors.centerIn: parent
        modal: true
        width: 440
        focus: true
        standardButtons: Dialog.Ok
        property string message: ""
        function say(text) {
            message = text;
            open();
        }
        onClosed: grid.forceActiveFocus()
        Shortcut { sequence: "Return"; enabled: errorDialog.visible; onActivated: errorDialog.close() }
        contentItem: Text {
            text: errorDialog.message
            color: win.textColor
            wrapMode: Text.Wrap
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
        }
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: unsavedDialog
        anchors.centerIn: parent
        modal: true
        width: 440
        focus: true
        title: "Unsaved changes"
        standardButtons: Dialog.Save | Dialog.Discard | Dialog.Cancel
        // Each button names its key.
        Component.onCompleted: {
            standardButton(Dialog.Save).text = "Save    S";
            standardButton(Dialog.Discard).text = "Discard    D";
            standardButton(Dialog.Cancel).text = "Cancel    C";
        }
        Shortcut { sequence: "S"; enabled: unsavedDialog.visible; onActivated: unsavedDialog.accept() }
        Shortcut { sequence: "D"; enabled: unsavedDialog.visible; onActivated: unsavedDialog.discarded() }
        Shortcut { sequence: "C"; enabled: unsavedDialog.visible; onActivated: unsavedDialog.reject() }
        Label {
            width: parent.width
            text: file.fileName + " has changes that are not saved."
            wrapMode: Text.Wrap
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
        }
        // Saving a new file asks for a name first; the pending action then
        // runs when that dialog is accepted.
        onAccepted: {
            if (win.save()) {
                var action = win.pendingAction;
                win.pendingAction = "";
                win.run(action);
            }
        }
        onDiscarded: {
            var action = win.pendingAction;
            win.pendingAction = "";
            close();
            win.run(action);
        }
        onRejected: win.pendingAction = ""
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: promptDialog
        anchors.centerIn: parent
        modal: true
        width: 460
        property string kind: ""
        property int column: 0
        // Where a new column or sheet goes; -1 for the end.
        property int at: -1
        readonly property bool needsExpr: kind === "computed" || kind === "const"
        title: kind === "sheet" ? "New sheet"
             : kind === "rows" ? "Insert rows in " + win.sheet.name
             : kind === "renameSheet" ? "Rename sheet " + win.sheet.name
             : kind === "rename" ? "Rename column " + (win.sheet.columns[column] ? win.sheet.columns[column].name : "")
             : kind === "column" ? "New column in " + win.sheet.name
             : kind === "computed" ? "New formula column in " + win.sheet.name
             : "New constant"
        focus: true
        // Not `standardButtons`: Ok would close the dialog before a refused
        // entry could be reported, so the buttons go through `tryAccept`.
        footer: DialogButtonBox {
            standardButtons: DialogButtonBox.Ok | DialogButtonBox.Cancel
            onAccepted: promptDialog.tryAccept()
            onRejected: promptDialog.reject()
        }

        // The name a new sheet is offered: `Sheet1`, or the first after it
        // that no sheet or constant has.
        function freeSheetName() {
            var taken = {};
            for (var t = 0; t < win.snap.sheets.length; t++)
                taken[win.snap.sheets[t].name] = true;
            for (var c = 0; c < win.snap.consts.length; c++)
                taken[win.snap.consts[c].name] = true;
            var n = 1;
            while (taken["Sheet" + n])
                n++;
            return "Sheet" + n;
        }

        // Ask for a new thing of kind `what`; a column goes in as column
        // `where`, or at the end.
        function ask(what, where) {
            kind = what;
            at = where === undefined ? -1 : where;
            column = win.curCol;
            nameField.text = what === "rename" && win.sheet.columns[column] ? win.sheet.columns[column].name
                : what === "sheet" ? freeSheetName()
                : what === "renameSheet" ? win.sheet.name : "";
            exprField.text = "";
            formulaBox.checked = what === "computed";
            countBox.text = "5";
            problem.text = "";
            open();
        }

        // Focus is taken once the dialog is up: asked for any sooner, it is
        // lost to the dialog opening.
        onOpened: takeFocus()
        function takeFocus() {
            forceActiveFocus();
            if (kind === "rows") {
                countBox.forceActiveFocus();
                countBox.selectAll();
            } else {
                nameField.forceActiveFocus();
                nameField.selectAll();
            }
        }

        // Add the rows below the selection.
        function addRows() {
            var count = Number(countBox.text.trim());
            if (!Number.isInteger(count) || count < 1 || count > 10000)
                return "Enter a number of rows from 1 to 10000";
            win.insertRows(true, count);
            return "";
        }

        function submit() {
            var message;
            var whole = win.selKind === "column";
            var all = win.selKind === "all";
            if (kind === "rows") {
                message = addRows();
            } else if (kind === "sheet") {
                // A new sheet renumbers the tabs, so nothing stays marked.
                message = at < 0 ? file.addSheet(nameField.text) : file.insertSheet(at, nameField.text);
            } else {
                // New rows go at the end, and new columns at `at` or the end.
                win.keeping(win.sheet.rows.length, at < 0 ? win.sheet.columns.length : at, function() {
                    message = kind === "rename" ? file.renameColumn(win.tab, column, nameField.text)
                        : kind === "renameSheet" ? file.renameSheet(win.tab, nameField.text)
                        : kind === "column" ? (at < 0 ? file.addColumn(win.tab, nameField.text)
                                                      : file.insertColumn(win.tab, at, nameField.text))
                        : kind === "computed" ? (at < 0 ? file.addComputed(win.tab, nameField.text, exprField.text)
                                                        : file.insertComputed(win.tab, at, nameField.text, exprField.text))
                        : file.addConst(nameField.text, exprField.text);
                });
            }
            if (message.length > 0) {
                problem.text = message;
                return false;
            }
            if (kind === "sheet") {
                win.switchTab(at < 0 ? win.snap.sheets.length - 1 : Math.min(at, win.snap.sheets.length - 1));
                // Put in beside a whole sheet, it is selected as that was.
                if (at >= 0 && all)
                    win.selectAll();
            }
            // A column put in beside whole columns is selected as they were.
            else if (at >= 0 && whole)
                win.selectLine(false, at, false);
            return true;
        }

        // Close on success; otherwise stay open, on the field to correct.
        function tryAccept() {
            if (submit()) {
                accept();
                return;
            }
            if (kind === "rows") {
                countBox.forceActiveFocus();
                countBox.selectAll();
            } else if (!exprField.activeFocus) {
                nameField.forceActiveFocus();
                nameField.selectAll();
            }
        }

        onClosed: grid.forceActiveFocus()

        contentItem: ColumnLayout {
            spacing: 6
            // How many rows, typed.
            TextField {
                id: countBox
                visible: promptDialog.kind === "rows"
                Layout.fillWidth: true
                validator: IntValidator { bottom: 1; top: 10000 }
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onAccepted: promptDialog.tryAccept()
            }
            TextField {
                id: nameField
                visible: promptDialog.kind !== "rows"
                focus: true
                Layout.fillWidth: true
                placeholderText: "Name"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onAccepted: promptDialog.needsExpr ? exprField.forceActiveFocus() : promptDialog.tryAccept()
            }
            // A column is of data, or of a formula: the one prompt asks
            // for either.
            CheckBox {
                id: formulaBox
                visible: promptDialog.kind === "column" || promptDialog.kind === "computed"
                text: "Formula"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onToggled: {
                    promptDialog.kind = checked ? "computed" : "column";
                    if (checked)
                        exprField.forceActiveFocus();
                }
            }
            TextField {
                id: exprField
                Layout.fillWidth: true
                visible: promptDialog.needsExpr
                placeholderText: promptDialog.kind === "computed" ? "Expression, e.g. Revenue - Cost" : "Expression, e.g. 20%"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onAccepted: promptDialog.tryAccept()
            }
            Label {
                id: problem
                Layout.fillWidth: true
                visible: text.length > 0
                color: win.errorColor
                wrapMode: Text.Wrap
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize - 1
            }
        }
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: problemsDialog
        anchors.centerIn: parent
        modal: true
        width: Math.min(win.width - 80, 760)
        title: "Problems"
        standardButtons: Dialog.Close
        onClosed: grid.forceActiveFocus()
        contentItem: ListView {
            implicitHeight: Math.min(contentHeight, 360)
            clip: true
            spacing: 8
            model: win.snap.problems.length
            delegate: Text {
                required property int index
                readonly property var item: win.snap.problems[index]
                width: ListView.view.width
                text: item ? "line " + item.line + ": " + item.message : ""
                color: win.textColor
                wrapMode: Text.Wrap
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize - 1
            }
        }
    }

    // The function directory: every function by category, with a search.
    Dialog {
        font.pixelSize: win.dialogSize
        id: functionsDialog
        anchors.centerIn: parent
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        Shortcut { sequence: "Esc"; enabled: functionsDialog.visible; onActivated: functionsDialog.close() }
        width: Math.min(win.width - 60, 720)
        height: Math.min(win.height - 60, 640)
        title: "Functions"
        standardButtons: Dialog.Close
        property var all: []
        // Headings and functions in one list: a heading has no `name`.
        property var shown: []
        property string copied: ""

        function refresh() {
            var words = searchField.text.toLowerCase().split(" ").filter(function(w) { return w.length > 0; });
            var out = [];
            var category = "";
            for (var i = 0; i < all.length; i++) {
                var f = all[i];
                var text = (f.name + " " + f.category + " " + f.usage + " " + f.summary + " " + f.code).toLowerCase();
                if (!words.every(function(w) { return text.indexOf(w) >= 0; }))
                    continue;
                if (f.category !== category) {
                    category = f.category;
                    out.push({ category: category });
                }
                out.push(f);
            }
            shown = out;
        }

        onOpened: {
            // Read afresh each time: the file's own functions come and go.
            all = JSON.parse(file.functionsJson());
            copied = "";
            searchField.text = "";
            refresh();
            searchField.forceActiveFocus();
        }
        onClosed: grid.forceActiveFocus()

        contentItem: ColumnLayout {
            spacing: 8
            TextField {
                id: searchField
                Layout.fillWidth: true
                placeholderText: "Search, e.g. sqrt, date, zone, custom"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onTextChanged: functionsDialog.refresh()
                Keys.onEscapePressed: functionsDialog.close()
            }
            ListView {
                id: functionsView
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: functionsDialog.shown.length
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: Item {
                    id: entry
                    required property int index
                    readonly property var item: functionsDialog.shown[index] || ({})
                    readonly property bool heading: !item.name
                    width: ListView.view.width - 12
                    height: heading ? headingText.implicitHeight + 16 : body.implicitHeight + 12

                    Text {
                        id: headingText
                        visible: entry.heading
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 4
                        text: entry.item.category || ""
                        color: win.accentColor
                        font.family: win.font.family
                        font.features: win.font.features
                        font.pixelSize: win.dialogSize
                        font.bold: true
                    }
                    Rectangle {
                        visible: !entry.heading
                        anchors.fill: parent
                        radius: 4
                        color: entryArea.containsMouse
                            ? Qt.rgba(win.textColor.r, win.textColor.g, win.textColor.b, 0.07) : "transparent"
                    }
                    Column {
                        id: body
                        visible: !entry.heading
                        anchors.verticalCenter: parent.verticalCenter
                        x: 8
                        width: parent.width - 16
                        spacing: 2
                        Text {
                            width: parent.width
                            text: (entry.item.usage || "")
                                + (functionsDialog.copied === entry.item.name ? "   copied" : "")
                            color: win.textColor
                            font.family: win.font.family
                            font.features: win.font.features
                            font.pixelSize: win.dialogSize
                            elide: Text.ElideRight
                        }
                        // The expression a function of the file's own stands for.
                        Text {
                            width: parent.width
                            visible: text.length > 0
                            text: entry.item.code || ""
                            color: win.accentColor
                            wrapMode: Text.Wrap
                            font.family: win.font.family
                            font.features: win.font.features
                            font.pixelSize: win.dialogSize - 1
                        }
                        Text {
                            width: parent.width
                            visible: text.length > 0
                            text: entry.item.summary || ""
                            color: win.mutedColor
                            wrapMode: Text.Wrap
                            font.family: win.font.family
                            font.features: win.font.features
                            font.pixelSize: win.dialogSize - 2
                        }
                    }
                    MouseArea {
                        id: entryArea
                        anchors.fill: parent
                        enabled: !entry.heading
                        hoverEnabled: true
                        // Click to copy the usage, ready to paste into a formula.
                        onClicked: {
                            clipboard.put(entry.item.usage);
                            functionsDialog.copied = entry.item.name;
                        }
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                text: functionsDialog.shown.length === 0 ? "No function matches."
                    : "Click a function to copy it."
                color: win.mutedColor
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize - 2
            }
        }
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: helpDialog
        anchors.centerIn: parent
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        Shortcut { sequence: "Esc"; enabled: helpDialog.visible; onActivated: helpDialog.close() }
        width: 560
        title: "Keyboard shortcuts"
        standardButtons: Dialog.Close
        onClosed: grid.forceActiveFocus()
        // No taller than the window: the list scrolls when it does not fit.
        height: Math.min(implicitHeight, win.height - 40)
        contentItem: Flickable {
            implicitHeight: helpText.implicitHeight
            contentHeight: helpText.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            Label {
                id: helpText
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                text: file.helpText()
            }
        }
    }
}
