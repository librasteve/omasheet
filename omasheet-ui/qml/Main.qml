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
    // The backing of what is drawn over the sheet: a cell opened out to the
    // lines it holds, the formula box open to several of them. Opaque: what
    // is under it is covered rather than read through, the border being
    // what says it is lying over the sheet.
    readonly property color overColor: Qt.rgba(pageColor.r, pageColor.g, pageColor.b, 1.0)

    // The size the desktop is set to: on Omarchy `[font] base-size`, which
    // `omarchy display text size` writes and its own shell scales from, so
    // omasheet reads as the rest of the desktop does. 14 where there is no
    // desktop to ask, which is the size it was before it asked.
    //
    // Two pixels more than the desktop: a grid of monospace reads smaller
    // than the desktop's own face at the same size, and a sheet is read a
    // cell at a time rather than a line at a time.
    readonly property int baseFontSize: {
        var desktop = file.desktopFontSize();
        return desktop > 0 ? Math.max(8, Math.min(40, desktop + 2)) : 14;
    }
    // Whether the formula box is open to four lines, for a formula written
    // over several of them.
    property bool formulaOpen: false
    // The cell opened out to every line it holds, as {tab, row, col}, or
    // null. A cell is one line tall, so one holding a newline shows its
    // first line and a caret; the caret opens it over the rows below.
    property var openCell: null
    function openCellAt(row, col) {
        openCell = openCell && openCell.tab === tab && openCell.row === row && openCell.col === col
            ? null : { tab: tab, row: row, col: col };
    }
    // Ctrl+plus and Ctrl+minus change the size, and everything scales from
    // it. The size zoomed to is kept per user, so the next launch opens at
    // it; Ctrl+0 forgets it and goes back to the desktop's.
    property int fontSize: {
        var saved = file.savedFontSize();
        return saved > 0 ? Math.max(8, Math.min(40, saved)) : baseFontSize;
    }
    // The smaller sizes are a share of it rather than so many pixels off,
    // so they keep their proportions at any size: a fixed `- 3` is a tenth
    // off at 30 and a third off at 9. The shares are Omarchy's own type
    // scale, and 8px is the floor, as it is there.
    readonly property int smallSize: Math.max(8, Math.round(fontSize * 0.917))
    readonly property int captionSize: Math.max(8, Math.round(fontSize * 0.833))
    readonly property int noteSize: Math.max(8, Math.round(fontSize * 0.786))
    readonly property int fineSize: Math.max(8, Math.round(fontSize * 0.714))
    // Dialogs are set a little smaller than the file.
    readonly property int dialogSize: captionSize
    readonly property int rowHeight: fontSize * 2
    readonly property int headerHeight: Math.round(fontSize * 3.3)
    // Wide enough for the last row's number: a digit of the caption font is
    // about 0.62 of its size, with a font-size of room beside it.
    readonly property int gutterWidth: Math.round(Math.max(fontSize * 3.4,
        captionSize * 0.62 * String(Math.max(0, (sheet ? sheet.nrows : 0) - 1)).length + fontSize * 1.2))
    onFontSizeChanged: Qt.callLater(function() { win.measure(); win.reveal(); })

    function zoom(step) {
        fontSize = step === 0 ? baseFontSize : Math.max(8, Math.min(40, fontSize + step));
        file.saveFontSize(step === 0 ? 0 : fontSize);
    }

    Material.theme: file.darkMode ? Material.Dark : Material.Light
    Material.accent: accentColor
    Material.background: pageColor
    Material.foreground: textColor
    color: pageColor
    // Qt has no "monospace" alias on macOS, where the name falls through to
    // a proportional face; Menlo ships with every Mac.
    readonly property string monoFamily: Qt.platform.os === "osx" ? "Menlo" : "monospace"
    font.family: monoFamily
    font.pixelSize: fontSize
    // No ligatures: `|>`, `==` and `:=` are shown as they are typed.
    font.features: { "calt": 0, "liga": 0 }

    FontMetrics {
        id: metrics
        font.family: win.monoFamily
        font.pixelSize: win.fontSize
        font.features: win.font.features
    }
    // The header's type line, which is set smaller.
    FontMetrics {
        id: noteMetrics
        font.family: win.monoFamily
        font.pixelSize: win.noteSize
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
    // Another sheet is another formula and another cell. A tab is pressed
    // with the mouse, which leaves the box holding the keyboard and the
    // sheet it was opened on, so the grid takes it back here.
    onTabChanged: {
        formulaOpen = false;
        grid.forceActiveFocus();
        resetBox();
    }
    readonly property var sheet: tab < tabs.length ? tabs[tab] : emptySheet
    readonly property var emptySheet: ({ name: "", columns: [], rows: [], nrows: 0, paged: false, isConsts: false })
    property var colWidths: []
    property real totalWidth: 0

    // A frame drawn alone can be shown late, when the next one comes: moving
    // up or down a row draws just the one. So each last frame is drawn twice.
    property bool drawnAgain: false
    onFrameSwapped: {
        // The spinner `busy` shows is on the screen: its work can begin.
        if (busyWork !== null)
            busyStart.drawn = true;
        drawnAgain = !drawnAgain;
        if (drawnAgain)
            update();
    }

    property int curRow: 0
    property int curCol: 0
    // A cell opened out to its lines closes when the cursor leaves it, by
    // the keyboard or by a click: the lines are drawn over the rows below,
    // and nothing in them says which cell they belong to.
    onCurRowChanged: cursorMoved()
    onCurColChanged: cursorMoved()
    function cursorMoved() {
        closeOpenCell();
        // Not mid-edit: the cell being typed in echoes into the box, and
        // that is what should be read there.
        if (!editing)
            resetBox();
    }
    function closeOpenCell() {
        if (openCell && (openCell.tab !== tab || openCell.row !== curRow || openCell.col !== curCol))
            openCell = null;
    }
    property int anchorRow: 0
    property int anchorCol: 0
    readonly property int selTop: Math.min(curRow, anchorRow)
    readonly property int selBottom: Math.max(curRow, anchorRow)
    readonly property int selLeft: Math.min(curCol, anchorCol)
    readonly property int selRight: Math.max(curCol, anchorCol)
    // What is being done that takes a while, said in the status bar beside
    // a spinner, or "". `busy` says it, lets that frame be drawn, and then
    // does the work: the work holds the app's thread, but the spinner is
    // turned by the render thread, so it goes on turning.
    property string busyText: ""
    property var busyWork: null
    function busy(text, work) {
        if (busyWork !== null) {
            work();
            return;
        }
        busyText = text;
        busyWork = work;
        busyStart.restart();
    }
    function runBusy() {
        busyStart.stop();
        var work = busyWork;
        if (work === null)
            return;
        try {
            work();
        } finally {
            busyWork = null;
            busyText = "";
        }
    }
    Timer {
        // The frame with the spinner is drawn, or a moment has passed in
        // which it would have been.
        id: busyStart
        property bool drawn: false
        interval: 16
        repeat: true
        onTriggered: if (drawn || ++tries > 6) win.runBusy()
        property int tries: 0
        function restart() { drawn = false; tries = 0; start(); }
    }

    // Count, Sum and Mean of the cells selected, for the status bar, or ""
    // for one cell. Worked out once the selection has stopped changing:
    // moving the cursor sets one end and then the other, and in between
    // the selection can be the whole of a table of millions of rows.
    property string selSummary: ""
    property int summaryAsked: 0
    readonly property string selKey: [tab, selTop, selLeft, selBottom, selRight].join(",")
    onSelKeyChanged: summarize()
    onSnapChanged: summarize()
    function summarize() {
        summaryAsked++;
        selSummary = "";
        summaryTimer.restart();
    }
    Timer {
        id: summaryTimer
        interval: 150
        onTriggered: {
            var several = !win.sheet.isConsts && win.tabs.length > 0
                && (win.selTop !== win.selBottom || win.selLeft !== win.selRight);
            if (!several || !win.snap) {
                win.selSummary = "";
                return;
            }
            // A big one is added up on a worker, and `summarized` brings it,
            // unless the selection has moved on by then.
            win.summaryAsked++;
            var text = file.askSummary(win.summaryAsked, win.tab, win.selTop, win.selLeft, win.selBottom, win.selRight);
            win.selSummary = text === "\u2026" ? "Adding up\u2026" : text;
        }
    }

    // Where the arrow keys are pointing a formula reference, while one is
    // being typed: -1 when no such pick is under way.
    property int pickRow: -1
    property int pickCol: -1
    // The other corner of a range picked with Shift: the same cell when
    // only one is picked.
    property int pickFromRow: -1
    property int pickFromCol: -1
    // The reference last placed by it, to take back out on the next move.
    property int pickAt: -1
    property int pickLen: 0

    property bool editing: false
    // The box at the top opens with an edit only where what is edited is
    // written over several lines already: one line stays one line until
    // Alt+Enter or the caret beside the box opens it. `commit` closes it
    // again.
    onEditingChanged: if (editing && manyLines()) formulaOpen = true;
    function manyLines() {
        return formulaField.source.indexOf("\n") >= 0;
    }
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
        var r = sheet.paged ? pagedRow(row) : sheet.rows[row];
        return r && r[col] ? r[col] : null;
    }

    // A paged sheet -- a big table, or a sheet calculated from one -- is
    // not in the snapshot row by row: its rows are asked for a block at a
    // time as the grid reaches them, and the blocks last asked for kept.
    // The blocks are a plain object, not a property, so filling one in the
    // middle of drawing a row changes no binding; a reload empties them.
    readonly property int pageSize: 200
    readonly property int pagesKept: 50
    property var pages: ({})
    property var pageOrder: []
    function pagedRow(row) {
        if (row < 0 || row >= sheet.nrows)
            return null;
        var block = Math.floor(row / pageSize);
        var key = tab + ":" + block;
        var page = pages[key];
        if (page === undefined) {
            page = JSON.parse(file.rowsJson(tab, block * pageSize, pageSize));
            pages[key] = page;
            pageOrder.push(key);
            if (pageOrder.length > pagesKept)
                delete pages[pageOrder.shift()];
        }
        return page[row - block * pageSize];
    }

    function reload() {
        followHistory();
        var y = rowsView.contentY;
        // Any edit may renumber the columns that were cut.
        grabbed = null;
        cutHold.grab = null;
        // What was copied stays marked through an edit that says where it
        // went, and no other.
        if (!keepMark)
            marked = null;
        // Stay on the same sheet when another window adds or removes one.
        var name = sheet.name, consts = sheet.isConsts;
        var parsed = JSON.parse(file.snapshotJson());
        // Rows of a paged sheet asked for before this are of the file as
        // it was.
        pages = {};
        pageOrder = [];
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
                rows: rows,
                nrows: rows.length,
                paged: false
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
        // Rows added or taken away start the list again at the top: it is
        // put back where it was.
        if (rowsView.contentY !== y)
            Qt.callLater(function() {
                rowsView.contentY = Math.max(0, Math.min(y, rowsView.contentHeight - rowsView.height));
            });
    }

    // Open a link from a cell: one with no scheme is a file beside the file.
    function openLink(link) {
        if (!/^[a-z][a-z0-9+.-]*:/i.test(link) && file.filePath.length > 0)
            link = "file://" + file.filePath.replace(/[^\/]*$/, "") + link;
        Qt.openUrlExternally(link);
    }

    // The width a cell asks for: a cell holding newlines shows one line at a
    // time, so it is the longest of them, not the lines run together.
    function lineWidth(text) {
        if (text.indexOf("\n") < 0)
            return text.length;
        var lines = text.split("\n");
        var longest = 0;
        for (var i = 0; i < lines.length; i++)
            longest = Math.max(longest, lines[i].length);
        return longest;
    }

    function measure() {
        var widths = [];
        var total = 0;
        var cols = sheet.columns;
        for (var c = 0; c < cols.length; c++) {
            var chars = cols[c].name.length;
            // A paged sheet is measured by its first block of rows.
            var measured = sheet.paged ? Math.min(sheet.nrows, pageSize) : sheet.nrows;
            for (var r = 0; r < measured; r++) {
                var cell = cellAt(r, c);
                if (cell)
                    chars = Math.max(chars, lineWidth(cell.d));
            }
            var w = Math.round(Math.min(46, Math.max(7, chars + 2)) * metrics.averageCharacterWidth) + 14;
            // Never narrower than the type line beside the telltale: the
            // type, a mark before it (`:`, `=` or `=:`, with a little over)
            // where it is set or the column is a formula, and `[]` after it
            // where it has a format. A type is never cut off, so this is
            // what keeps it clear of the telltale. It is set smaller, so a
            // short type such as `Integer` fits the width an empty column
            // has already.
            if (cols[c].type.length > 0) {
                var marked = cols[c].declared || (cols[c].computed && !sheet.isConsts);
                var typeChars = cols[c].type.length
                    + (marked ? 3 : 0) + (cols[c].format ? 2 : 0);
                var telltale = sheet.isConsts ? 0 : win.noteSize;
                w = Math.max(w, Math.ceil(typeChars * noteMetrics.averageCharacterWidth) + telltale + 14);
            }
            widths.push(w);
            total += w;
        }
        colWidths = widths;
        totalWidth = total;
    }

    function clampCursor() {
        var maxRow = Math.max(0, sheet.nrows - 1);
        var maxCol = Math.max(0, sheet.columns.length - 1);
        curRow = Math.min(curRow, maxRow);
        anchorRow = Math.min(anchorRow, maxRow);
        curCol = Math.min(curCol, maxCol);
        anchorCol = Math.min(anchorCol, maxCol);
    }

    // The filters in force, by sheet and then column name: each column's
    // expression, in which `*` is the cell (filter-by-expression D160). A
    // filter is a view of the sheet and nothing more -- it hides rows in
    // the grid, and the file, a save and an export all still hold every
    // row -- where a sort rearranges the rows in the file.
    property var filters: ({})
    property int filterRev: 0
    // The filters as they last worked: one that fails when it is applied
    // gives way to these, and says why.
    property var goodFilters: ({})
    // How many rows the grid lists while a filter is in force, and -1 while
    // every row shows. The document works out which, once for each change
    // of the sheet or the filter, and keeps them: a big table's are
    // millions, too many to bring here, so the grid asks it for the row at
    // a place and the place of a row. A new object each time, so that what
    // reads it follows every change of the filter, not only of the count.
    //
    // A big table's filter is worked out on a worker, so that the window
    // goes on answering the desktop: `filterRows` then says -3, the rows
    // shown stay as they were, and `filtered` brings the count.
    property var filterShown: ({ count: -1 })
    property bool filterPending: false
    // Makes each answer differ from the last: QML takes two objects of the
    // same contents to be no change, and the rows would stay as they were.
    property int filterSerial: 0
    // The filter as Rust is given it, and everything that changes what it
    // shows: the sheet, its filters, and the file's every revision.
    readonly property string filterSpec: {
        filterRev;
        var f = filters[sheet.name];
        var parts = [];
        if (f && !sheet.isConsts)
            for (var name in f)
                if (f[name] && f[name].length > 0)
                    parts.push(name + "\u001f" + f[name]);
        return parts.join("\u001e");
    }
    readonly property string filterKey: tab + "\u001d" + filterSpec + "\u001d" + file.revision
    onFilterKeyChanged: runFilter()
    function runFilter() {
        var count = file.filterRows(tab, filterSpec);
        filterPending = count === -3;
        if (!filterPending)
            filterShown = { count: count, serial: ++filterSerial };
    }
    Connections {
        target: file
        function onSummarized(asked, text) {
            if (asked === win.summaryAsked)
                win.selSummary = text;
        }
        function onFiltered(count) {
            win.filterPending = false;
            win.filterShown = { count: count, serial: ++filterSerial };
        }
        function onSorted(col, descending, problem) {
            if (problem.length > 0)
                win.notice = problem;
            else
                win.noteSort(col, descending);
        }
    }
    onFilterShownChanged: {
        if (filterShown.count === -2) {
            notice = file.filterProblem();
            filters = JSON.parse(JSON.stringify(goodFilters));
            filterRev++;
        } else {
            goodFilters = JSON.parse(JSON.stringify(filters));
        }
    }
    readonly property int shownCount: filterShown.count < 0 ? sheet.nrows : filterShown.count
    function isHidden(row) {
        return filterShown.count >= 0 && file.posOf(row) < 0;
    }
    // The place of a row in the list, and the row at a place in it.
    function posOf(row) {
        return filterShown.count < 0 ? row : Math.max(0, file.posOf(row));
    }
    function rowAt(pos) {
        return filterShown.count < 0 ? pos : file.rowAt(pos);
    }
    // The row drawn at `y` down the list, the nearest one past either end.
    function rowAtY(y) {
        if (shownCount === 0)
            return 0;
        return rowAt(Math.max(0, Math.min(shownCount - 1, Math.floor(y / rowHeight))));
    }
    function filtered(col) {
        return filterOf(col).length > 0;
    }
    // The expression column `col` is filtered by, or "".
    function filterOf(col) {
        filterRev;
        var f = filters[sheet.name];
        var expr = f && sheet.columns[col] ? f[sheet.columns[col].name] : "";
        return typeof expr === "string" ? expr.trim() : "";
    }
    function setFilter(col, expr) {
        var all = JSON.parse(JSON.stringify(filters));
        var f = all[sheet.name] || {};
        f[sheet.columns[col].name] = expr.trim();
        all[sheet.name] = f;
        busy("Filtering\u2026", function() {
            filters = all;
            filterRev++;
        });
    }
    // What each sheet has been sorted by: every column, whose telltale
    // stays solid, and the last one and which way, which is the order the
    // rows are in and so the one that shows a caret. Every sort is a step
    // to undo -- of a sheet's own rows, of a calculated one, or of a table,
    // whose order is the grid's alone -- so each is kept with the number of
    // steps to undo it left: it is in force while there are at least that
    // many, and goes and comes back with the step. An edit made after an
    // undo drops the steps it was undone past, and the sorts with them. All
    // of it is forgotten with the file, as a filter is.
    property var sortLog: []
    property int undoDepth: 0
    property int editCount: 0
    property int sortRev: 0
    function sortState(name) {
        sortRev;
        var state = null;
        for (var i = 0; i < sortLog.length; i++) {
            var e = sortLog[i];
            if (e.sheet !== name || e.mark > undoDepth)
                continue;
            if (state === null)
                state = { done: {} };
            state.col = e.col;
            state.desc = e.desc;
            state.done[e.col] = true;
        }
        return state;
    }
    function sortedAs(col) {
        var st = sortState(sheet.name);
        if (!st || !sheet.columns[col] || st.col !== sheet.columns[col].name)
            return "";
        return st.desc ? "desc" : "asc";
    }
    function wasSorted(col) {
        var st = sortState(sheet.name);
        return !!st && !!sheet.columns[col] && st.done[sheet.columns[col].name] === true;
    }
    // The steps to undo there were when the sort last asked for began.
    property int sortMark: -1
    function noteSort(col, desc) {
        // A column already in the order asked for is left as it is, with no
        // step to undo; so there is nothing for an undo to take the caret
        // back with, and it is not lit.
        if (file.undoMark() === sortMark)
            return;
        var name = sheet.columns[col].name;
        sortLog = sortLog.concat([{ mark: file.undoMark(), sheet: sheet.name, col: name, desc: desc }]);
        sortRev++;
    }
    // Keep up with the history after each change of the file: an undo or a
    // redo moves the depth, and an edit also drops what was undone past.
    function followHistory() {
        var depth = file.undoMark(), edits = file.editCount();
        if (edits !== editCount) {
            var kept = [];
            for (var i = 0; i < sortLog.length; i++)
                if (sortLog[i].mark < depth)
                    kept.push(sortLog[i]);
            sortLog = kept;
        }
        undoDepth = depth;
        editCount = edits;
        sortRev++;
    }

    // The nearest row that is shown, going `dir` from `row` and then the
    // other way if there is none.
    function shownFrom(row, dir) {
        var last = sheet.nrows - 1;
        for (var r = row; r >= 0 && r <= last; r += dir)
            if (!isHidden(r))
                return r;
        for (r = row; r >= 0 && r <= last; r -= dir)
            if (!isHidden(r))
                return r;
        return row;
    }
    // Whether more than one row is selected and a hidden one is among them:
    // what is done to the selection would be done to rows nobody can see.
    function selectionHidesRows() {
        if (filterShown.count < 0 || selTop === selBottom)
            return false;
        return file.hidesBetween(selTop, selBottom);
    }

    function selectCell(row, col, extend) {
        if (sheet.nrows === 0 || sheet.columns.length === 0)
            return;
        if (!extend) {
            byRow = false;
            byCol = false;
        }
        row = Math.max(0, Math.min(sheet.nrows - 1, row));
        curRow = isHidden(row) ? shownFrom(row, row >= curRow ? 1 : -1) : row;
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
        var maxRow = Math.max(0, sheet.nrows - 1);
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
        var max = Math.max(0, (vertical ? sheet.nrows : sheet.columns.length) - 1);
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
        anchorRow = Math.max(0, sheet.nrows - 1);
        anchorCol = Math.max(0, sheet.columns.length - 1);
        curRow = 0;
        curCol = 0;
    }

    // Let go of the selection, and of anything cut or copied: one cell is
    // left, where the cursor is.
    function deselect() {
        unmark();
        byRow = false;
        byCol = false;
        anchorRow = curRow;
        anchorCol = curCol;
    }

    // Let go of anything cut or copied, and leave the selection.
    function unmark() {
        cutHold.copy = null;
        grabbed = null;
        cutHold.grab = null;
        marked = null;
        movedOnce = false;
        file.forgetCut();
        notice = "";
    }

    function reveal() {
        revealAt(curRow, curCol);
    }

    // Scroll the rows just far enough to show place `pos` whole. Every row
    // is `rowHeight` high, so where a place is is a multiplication; a
    // ListView's own positionViewAtIndex lays rows out on its way there,
    // which on a table of millions of rows takes seconds.
    function showPlace(pos) {
        if (pos < 0)
            return;
        var top = rowsView.originY + pos * rowHeight;
        var bottom = top + rowHeight;
        var y = rowsView.contentY;
        if (top < y)
            y = top;
        else if (bottom > y + rowsView.height)
            y = bottom - rowsView.height;
        if (y !== rowsView.contentY)
            rowsView.contentY = y;
    }

    function revealAt(row, col) {
        showPlace(posOf(row));
        var x = gutterWidth;
        for (var c = 0; c < col; c++)
            x += colWidths[c];
        var w = colWidths[col] || 0;
        if (x - gutterWidth < hflick.contentX)
            hflick.contentX = x - gutterWidth;
        else if (x + w > hflick.contentX + hflick.width)
            hflick.contentX = x + w - hflick.width;
    }

    // Where each sheet was left, by its name, to come back to: the cursor,
    // the selection and how far it was scrolled.
    property var places: ({})
    function placeKey(t) {
        return tabs[t].isConsts ? "" : tabs[t].name;
    }

    function switchTab(index) {
        if (index < 0 || index >= tabs.length)
            return;
        commitEditing();
        // Not when the sheet here is no longer the one that was left, as
        // after a delete: then `index` is this tab.
        if (index !== tab && tab < tabs.length)
            places[placeKey(tab)] = {
                curRow: curRow, curCol: curCol, anchorRow: anchorRow, anchorCol: anchorCol,
                byRow: byRow, byCol: byCol, x: hflick.contentX, y: rowsView.contentY
            };
        tab = index;
        var p = places[placeKey(index)];
        curRow = p ? p.curRow : 0;
        curCol = p ? p.curCol : 0;
        anchorRow = p ? p.anchorRow : 0;
        anchorCol = p ? p.anchorCol : 0;
        byRow = p ? p.byRow : false;
        byCol = p ? p.byCol : false;
        measure();
        clampCursor();
        hflick.contentX = 0;
        rowsView.positionViewAtBeginning();
        if (p)
            Qt.callLater(function() {
                hflick.contentX = Math.max(0, Math.min(p.x, hflick.contentWidth - hflick.width));
                rowsView.contentY = Math.max(0, Math.min(p.y, rowsView.contentHeight - rowsView.height));
            });
        grid.forceActiveFocus();
    }

    // ---- editing -----------------------------------------------------------

    // Why the selected cells take no edit, or "": the data of a view is its
    // file's, and a calculated sheet has no cell of its own. A formula
    // column appended to a view, a decorator, takes its formula as any
    // other does, so a selection of those alone is free to edit.
    function readOnlyWhy() {
        var why = sheet.why || "";
        if (why.length === 0 || sheet.isConsts)
            return "";
        if (sheet.view)
            return why;
        for (var c = selLeft; c <= selRight; c++) {
            var column = sheet.columns[c];
            if (!column || !column.computed)
                return why;
        }
        return "";
    }

    // Refuse an edit of read-only cells before it is begun, and say why in
    // the status bar. True if it was refused.
    function refusedEdit() {
        var why = readOnlyWhy();
        if (why.length === 0)
            return false;
        file.status = why;
        return true;
    }

    function startEditing(seed) {
        // A calculated sheet has one formula, written in the box at the
        // top: Enter on any of its cells goes there, as for a column, and
        // an expression that gives no rows has no cell to ask about.
        if (sheet.view) {
            if (seed === null) {
                formulaField.forceActiveFocus();
                formulaField.selectAll();
            } else {
                refusedEdit();
            }
            return;
        }
        // The data of a view is its file's: no editor opens (D207).
        if (refusedEdit())
            return;
        if (!cellAt(curRow, curCol))
            return;
        if (sheet.isConsts && curCol !== 1)
            return;
        // A formula column has one formula, written in the box at the top:
        // Enter on any cell of it goes there, and typing does nothing.
        var column = sheet.columns[curCol];
        if (!sheet.isConsts && column && column.computed) {
            if (seed === null) {
                formulaField.forceActiveFocus();
                formulaField.selectAll();
            }
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

    // The cells the formula being typed refers to, highlighted until the
    // reference is taken out of the text or the edit is done:
    // `[{top, left, bottom, right}]`.
    readonly property var refBoxes: (editing || formulaField.activeFocus) && !sheet.isConsts
        ? refsIn(formulaField.text, curRow, curCol) : []

    // The blocks of this sheet that `[cols; rows]` references in `text`
    // name, as seen from the cell at `(row, col)`.
    function refsIn(text, row, col) {
        var boxes = [];
        if (text.trim().charAt(0) !== "=")
            return boxes;
        var nRows = sheet.nrows, nCols = sheet.columns.length;
        // One end of a span: `*`, `*+n`, `*-n`, a position, `-n` from the
        // end, or for columns a name.
        var end = function(t, here, n, names) {
            t = t.trim();
            var m = /^\*\s*([+-])\s*(\d+)$/.exec(t);
            if (t === "*")
                return here;
            if (m)
                return here + (m[1] === "+" ? 1 : -1) * Number(m[2]);
            if (/^\d+$/.test(t))
                return Number(t);
            if (/^-\d+$/.test(t))
                return n + Number(t);
            if (names)
                for (var i = 0; i < names.length; i++)
                    if (names[i].name === t)
                        return i;
            return null;
        };
        // A whole slot, or `a..b`: `[from, to]`, or null.
        var span = function(t, here, n, names) {
            t = t.trim();
            if (t.length === 0)
                return [0, n - 1];
            if (t.indexOf("^") >= 0)
                return null;
            var ends = t.split("..");
            if (ends.length > 2)
                return null;
            var a = end(ends[0], here, n, names);
            var b = ends.length === 2 ? end(ends[1], here, n, names) : a;
            if (a === null || b === null)
                return null;
            return [Math.max(0, Math.min(a, b)), Math.min(n - 1, Math.max(a, b))];
        };
        var re = /([A-Za-z_][A-Za-z0-9_]*)?\[([^\[\];]*);([^\[\]]*)\]/g;
        var m;
        while ((m = re.exec(text)) !== null) {
            // Another sheet's cells are not on this one.
            if (m[1] && m[1] !== sheet.name)
                continue;
            var c = span(m[2], col, nCols, sheet.columns);
            var r = span(m[3], row, nRows, null);
            if (c && r && c[0] <= c[1] && r[0] <= r[1])
                boxes.push({ top: r[0], left: c[0], bottom: r[1], right: c[1] });
        }
        return boxes;
    }

    function inRefs(row, col) {
        var boxes = refBoxes;
        for (var i = 0; i < boxes.length; i++) {
            var b = boxes[i];
            if (row >= b.top && row <= b.bottom && col >= b.left && col <= b.right)
                return true;
        }
        return false;
    }

    // The box at the top shows what is typed in a cell as it is typed, and
    // the cell's source again once the edit is done.
    // The box shows what the cursor is on, and its text is a binding so
    // that it follows the cursor, the sheet and the cell as they change.
    // Typing in the box, or in a cell that echoes into it, breaks the
    // binding; this puts it back. A written-once text would be left behind
    // by a tab or a cell pressed with the mouse, which takes no focus from
    // the box and so leaves every `activeFocus` guard to skip.
    function resetBox() {
        if (formulaField.activeFocus)
            return;
        formulaField.text = Qt.binding(function() { return formulaField.shown; });
    }

    function stopEditing() {
        editing = false;
        formulaOpen = false;
        stopPicking();
        resetBox();
        grid.forceActiveFocus();
    }

    // A click away from the cell being edited writes what is typed, as
    // Enter does, and goes where the click went: it is not an Escape.
    function commitEditing() {
        if (editing && editor)
            editor.finish(0, 0);
        else
            stopEditing();
    }

    // F2 on a whole column: its header cell takes the new name.
    signal renameColumnAsked(int col)
    // The tab whose sheet is being renamed in place, or -1.
    property int renamingTab: -1

    // The text field a formula is being typed into, if any: the cell's own
    // editor or the entry box.
    property var editor: null

    // Whether a click on a cell should add a reference to the formula
    // being typed, rather than move the selection.
    function canPick() {
        return !!editor && !sheet.isConsts && editor.text.trim().charAt(0) === "=";
    }

    // Whether an arrow key should pick a cell rather than move in the text:
    // while a pick is under way, or where a reference may go: after `=`, a
    // bracket, a separator or an operator, before the end, a closing bracket,
    // a separator or an operator, and not inside a reference's `[ ]`.
    // Anywhere else, such as on or among the references already written,
    // the arrows move in the text as usual.
    function arrowPicks() {
        if (!canPick())
            return false;
        if (pickRow >= 0)
            return true;
        // Quoted text is no part of the formula's shape.
        var text = editor.text.replace(/'(\\.|[^'\\])*'?|"(\\.|[^"\\])*"?/g, function(q) { return q.replace(/./g, "x"); });
        var before = text.slice(0, editor.cursorPosition);
        var after = text.slice(editor.cursorPosition).replace(/^\s+/, "");
        if ((before.match(/\[/g) || []).length > (before.match(/\]/g) || []).length)
            return false;
        // Nor in the middle of an operator such as `//` or `<=`.
        if (/[=<>!\/*.^]$/.test(before) && /^[=<>!\/*.^]/.test(text.slice(editor.cursorPosition)))
            return false;
        return /([=(,;+\-*\/<>!^]|\.\.|\b(and|or|not|in))\s*$/.test(before)
            && /^($|[),;\]+\-*\/<>=!^]|\.\.|(and|or|in)\b)/.test(after);
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

    // The reference to the cells from `(r0, c0)` to `(r1, c1)`, written as
    // `refFor` writes one cell, with `..` between the ends that differ.
    function refForRange(r0, c0, r1, c1, absolute) {
        if (r0 === r1 && c0 === c1)
            return refFor(r0, c0, absolute);
        var top = Math.min(r0, r1), bottom = Math.max(r0, r1);
        var left = Math.min(c0, c1), right = Math.max(c0, c1);
        var span = function(a, b, one) { return a === b ? one(a) : one(a) + ".." + one(b); };
        var step = function(n) { return n === 0 ? "*" : "*" + (n > 0 ? "+" : "-") + Math.abs(n); };
        var rows = absolute ? span(top, bottom, function(r) { return "" + r; })
                            : span(top - curRow, bottom - curRow, step);
        var cols = left === right ? sheet.columns[left].name
            : absolute ? span(left, right, function(c) { return "" + c; })
            : span(left - curCol, right - curCol, step);
        // A formula in one cell counts its columns from itself too.
        if (left === right && !absolute && !(sheet.columns[curCol] && sheet.columns[curCol].computed))
            cols = step(left - curCol);
        return "[" + cols + "; " + rows + "]";
    }

    // A press on a cell while a formula is typed: its reference goes in at
    // the cursor, and a drag from here makes it a range. With `more`, or
    // straight after another reference, it is one more in a list, after a
    // comma.
    property bool pickAbsolute: false
    function pickPress(row, col, absolute, more) {
        if (!canPick() || !sheet.columns[col])
            return;
        var at = editor.selectionStart;
        editor.remove(at, editor.selectionEnd);
        // Straight after another reference, it is one more in a list too.
        var before = editor.text.slice(0, at);
        if ((more && !/(^\s*=|[(,;])\s*$/.test(before)) || /\]\s*$/.test(before)) {
            editor.insert(at, ", ");
            at += 2;
        }
        pickAbsolute = absolute;
        pickAt = at;
        pickLen = 0;
        pickFromRow = row;
        pickFromCol = col;
        pickDrag(row, col);
        editor.forceActiveFocus();
    }

    // Drag a pick begun by `pickPress` to `(row, col)`.
    function pickDrag(row, col) {
        if (pickAt < 0 || !editor)
            return;
        row = Math.max(0, Math.min(sheet.nrows - 1, row));
        col = Math.max(0, Math.min(sheet.columns.length - 1, col));
        editor.remove(pickAt, pickAt + pickLen);
        var ref = refForRange(pickFromRow, pickFromCol, row, col, pickAbsolute);
        editor.insert(pickAt, ref);
        pickLen = ref.length;
        editor.cursorPosition = pickAt + ref.length;
        pickRow = row;
        pickCol = col;
    }

    // Move the keyboard's formula-reference pick by `dRow`/`dCol` cells,
    // writing (or rewriting, if one is already under way) the reference it
    // lands on, exactly as a click on that cell would. Returns whether
    // there was a formula to pick a cell for.
    function pickArrow(dRow, dCol, absolute, extend, toEnd) {
        if (!canPick() || sheet.nrows === 0 || sheet.columns.length === 0)
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
        var clampRow = function(r) { return Math.max(0, Math.min(sheet.nrows - 1, r)); };
        var clampCol = function(c) { return Math.max(0, Math.min(sheet.columns.length - 1, c)); };
        // Shift grows a range from where it started: the next cell, if no
        // pick was under way. Without it the pick is one cell again.
        if (extend && pickFromRow < 0) {
            pickFromRow = clampRow(pickRow + dRow);
            pickFromCol = clampCol(pickCol + dCol);
        }
        // Ctrl goes all the way to the first or last cell.
        var far = toEnd ? Math.max(sheet.nrows, sheet.columns.length) : 1;
        pickRow = clampRow(pickRow + dRow * far);
        pickCol = clampCol(pickCol + dCol * far);
        if (!extend) {
            pickFromRow = pickRow;
            pickFromCol = pickCol;
        }
        var ref = refForRange(pickFromRow, pickFromCol, pickRow, pickCol, absolute);
        editor.insert(pickAt, ref);
        pickLen = ref.length;
        editor.cursorPosition = pickAt + ref.length;
        revealAt(pickRow, pickCol);
        return true;
    }

    // Take back the reference of a keyboard pick under way, and go on
    // typing. Returns whether there was one.
    function cancelPick() {
        if (pickRow < 0)
            return false;
        editor.remove(pickAt, pickAt + pickLen);
        editor.cursorPosition = pickAt;
        stopPicking();
        return true;
    }

    // End a keyboard pick under way, so that the next arrow pressed starts
    // a fresh one from the edited cell rather than carrying on from here.
    function stopPicking() {
        pickRow = -1;
        pickCol = -1;
        pickFromRow = -1;
        pickFromCol = -1;
        pickAt = -1;
        pickLen = 0;
    }

    // Go to the box at the top, to write what the cell holds there. A
    // cell of a formula column renames the column, as the whole column does.
    function editInBox() {
        if (tabs.length === 0)
            return;
        stopEditing();
        // A whole column renames it in its header, and the whole sheet in
        // its tab.
        if (selKind === "column" || selKind === "all") {
            if (sheet.isConsts)
                return;
            notice = "";
            if (selKind === "all")
                renamingTab = tab;
            else if (selLeft === selRight && sheet.columns[selLeft])
                renameColumnAsked(selLeft);
            return;
        }
        // A calculated sheet has one formula wherever the cursor is, and an
        // expression that gives no rows leaves no cell to stand on: F2
        // still reaches the formula, which is the only way to put it right.
        if (sheet.view) {
            notice = "";
            formulaField.forceActiveFocus();
            formulaField.selectAll();
            return;
        }
        // A block of cells has no one cell to edit.
        if (!hasCursor || selTop !== selBottom || selLeft !== selRight || !cellAt(curRow, curCol))
            return;
        if (!sheet.isConsts && sheet.columns[curCol] && sheet.columns[curCol].computed) {
            notice = "";
            renameColumnAsked(curCol);
            return;
        }
        formulaField.forceActiveFocus();
        formulaField.selectAll();
    }

    function commit(text, dRow, dCol) {
        var row = curRow, col = curCol;
        editing = false;
        formulaOpen = false;
        stopPicking();
        keeping(row + 1, 0, function() {
            if (sheet.isConsts) {
                if (col === 1)
                    file.setConst(row, text);
            } else {
                file.setCell(tab, row, col, text);
            }
        });
        selectCell(row + dRow, col + dCol, false);
        grid.forceActiveFocus();
        resetBox();
    }

    // Whole rows or columns cut with Ctrl+X, waiting to be put down with
    // Ctrl+V: `{tab, rows, first, count}`; a whole sheet, `{tab, sheet}`;
    // or null.
    property var grabbed: null
    // The whole rows or columns last cut, shared by every window on the
    // file (see App.qml): a window that did not cut them can still move them.
    property QtObject cutHold: QtObject { property var grab: null; property var copy: null }
    readonly property var foreign: grabbed !== null ? grabbed : cutHold.grab
    // Why the last move was refused, shown in the footer.
    property string notice: ""
    // What the pointer is resting on, or a fault in what is being typed,
    // said in the footer for as long as it holds: where a tip would have
    // been. It is set by `hover` from whatever has something to say, and
    // cleared by the same one, the last to speak being the one heard.
    property string hint: ""
    property bool hintBad: false
    property var hintFrom: null
    function hover(from, text, bad) {
        if (text.length > 0) {
            hintFrom = from;
            hint = text;
            hintBad = !!bad;
        } else if (hintFrom === from) {
            hintFrom = null;
            hint = "";
            hintBad = false;
        }
    }
    // Whether the selection was last made from a row number or a column
    // header, to tell a row from a column where one selection is both.
    property bool byRow: false
    property bool byCol: false
    readonly property bool wholeColumns: !sheet.isConsts && sheet.columns.length > 0
        && selTop === 0 && selBottom >= sheet.nrows - 1
    readonly property bool wholeRows: !sheet.isConsts && sheet.nrows > 0
        && selLeft === 0 && selRight >= sheet.columns.length - 1
    // What is selected: whole "row"s, whole "column"s, "all" of the sheet,
    // or a block of "cells". The menu and the keys act on rows only for rows
    // and cells, and on columns only for columns and cells.
    readonly property string selKind: {
        if (sheet.isConsts || sheet.nrows === 0)
            return "cells";
        // One cell of a sheet with one row is a cell, unless its header or
        // row number was what was clicked.
        var cols = wholeColumns && (byCol || sheet.nrows > 1);
        var rows = wholeRows && (byRow || sheet.columns.length > 1);
        if (cols && rows)
            return byRow ? "row" : byCol ? "column" : "all";
        return rows ? "row" : cols ? "column" : "cells";
    }
    // Whole rows or columns are selected as a whole, with no cell of their
    // own to show at the top or to outline.
    readonly property bool hasCursor: selKind === "cells"
    // One whole column selected that is calculated: it has one formula, as
    // a cell of it has, and the box keeps showing it.
    readonly property bool oneFormula: selKind === "column" && selLeft === selRight
        && selLeft < sheet.columns.length && sheet.columns[selLeft].computed
    // Several cells that are not whole rows, columns or the sheet.
    readonly property bool isBlock: selKind === "cells" && !sheet.isConsts
        && (selTop !== selBottom || selLeft !== selRight)
    // What a cut would pick up: "row", "column", or "" for anything else.
    readonly property string grabKind: selKind === "row" || selKind === "column" ? selKind : ""
    readonly property bool grabbedHere: grabbed !== null && !grabbed.sheet && grabbed.tab === tab && !sheet.isConsts

    // The whole sheet copied or cut, outlined on its tab: `{tab, cut}`, or
    // null.
    readonly property var sheetMark: grabbed !== null && grabbed.sheet ? { tab: grabbed.tab, cut: true }
        : marked !== null && marked.sheet ? { tab: marked.tab, cut: marked.cut }
        : null

    // The cells last copied or cut, shown as they were selected for as long
    // as they can be pasted, until Esc or the next copy:
    // `{tab, top, left, bottom, right, text, cut, columns, rows, sheet}`, or
    // null; `columns` when whole columns were copied, marked down from their
    // headers, `rows` when whole rows were, marked from their numbers, and
    // `sheet` when the whole sheet was.
    property var marked: null
    property bool keepMark: false

    // What is outlined on this sheet: what was copied or cut, or the rows
    // or columns cut to be moved; null if nothing.
    readonly property var outline: sheetMark !== null
        ? (sheetMark.tab !== tab ? null
            : { top: 0, bottom: sheet.nrows - 1, left: 0, right: sheet.columns.length - 1,
                cut: sheetMark.cut, columns: true, rows: true })
        : grabbedHere
        ? (grabbed.rows
            ? { top: grabbed.first, bottom: grabbed.first + grabbed.count - 1,
                left: 0, right: sheet.columns.length - 1, cut: true, columns: false, rows: true }
            : { top: 0, bottom: sheet.nrows - 1,
                left: grabbed.first, right: grabbed.first + grabbed.count - 1, cut: true, columns: true, rows: false })
        : marked !== null && marked.tab === tab ? marked : null

    function isMarked(row, col) {
        var m = outline;
        return m !== null
            && row >= m.top && row <= m.bottom && col >= m.left && col <= m.right;
    }

    // Cells cut and pasted, still on the clipboard to paste again as copies.
    property bool movedOnce: false
    // Cells copied in this window, or text on the system clipboard from
    // anywhere: another window of the app, which shares the clipboard and
    // what it remembers of a copy but not this window's mark, or another
    // program.
    readonly property bool canPaste: marked !== null || movedOnce || clipboard.canPaste

    // Whole rows waiting to go in as rows of their own, rather than blank
    // ones: cut, to be moved there, or copied, to go in as copies.
    // `{cut, count}`, or null when none are.
    // Whole rows cut on another sheet: they move here by being copied in
    // and taken out there.
    readonly property bool grabbedElsewhereRows: foreign !== null && !foreign.sheet && foreign.rows === true
        && foreign.tab !== tab && foreign.tab < tabs.length && !sheet.isConsts
        && foreign.text !== undefined && foreign.text.length > 0
    readonly property var rowsOnHand: grabbedHere && grabbed.rows
        ? { cut: true, count: grabbed.count }
        : grabbedElsewhereRows
        ? { cut: true, count: foreign.count, elsewhere: true }
        : marked !== null && marked.rows && !sheet.isConsts && marked.tab < tabs.length
            && marked.text.length > 0
        ? { cut: false, count: marked.bottom - marked.top + 1 }
        : null
    readonly property bool grabbedElsewhereColumns: foreign !== null && !foreign.sheet && foreign.rows === false
        && foreign.tab !== tab && foreign.tab < tabs.length && !sheet.isConsts
        && foreign.text !== undefined && foreign.text.length > 0 && foreign.names !== undefined
    // The same for whole columns.
    readonly property var colsOnHand: grabbedHere && !grabbed.rows
        ? { cut: true, count: grabbed.count }
        : grabbedElsewhereColumns
        ? { cut: true, count: foreign.count, elsewhere: true }
        : marked !== null && marked.columns && !sheet.isConsts && marked.tab < tabs.length
            && marked.text.length > 0
        ? { cut: false, count: marked.right - marked.left + 1 }
        : null

    function mark(text, cut) {
        movedOnce = false;
        marked = { tab: tab, top: selTop, left: selLeft, bottom: selBottom, right: selRight, text: text, cut: !!cut, columns: selKind === "column", rows: selKind === "row",
                   sheet: selKind === "all" };
        // For the other windows: whether what is on the clipboard is whole rows or columns.
        cutHold.copy = { rows: selKind === "row", columns: selKind === "column" };
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
        if (selectionHidesRows())
            return;
        var m = marked;
        var rows = sheet.nrows, cols = sheet.columns.length;
        keepMark = true;
        edit();
        keepMark = false;
        if (m === null || m.tab !== tab)
            return;
        var v = shifted(m.top, m.bottom, row, sheet.nrows - rows);
        var h = shifted(m.left, m.right, col, sheet.columns.length - cols);
        marked = v === null || h === null ? null
            : { tab: m.tab, top: v[0], left: h[0], bottom: v[1], right: h[1], text: m.text, cut: m.cut, columns: m.columns, rows: m.rows, sheet: m.sheet };
    }

    // Paste `text` with `put`, keeping the mark if it is what was copied.
    // A paste adds rows only at the end, unless it makes room at `row` or
    // `col`.
    function pasteWith(text, put, row, col) {
        if (marked !== null && marked.text !== text)
            marked = null;
        var cut = marked !== null && marked.cut;
        var problem = "";
        keeping(row === undefined ? sheet.nrows : row, col === undefined ? sheet.columns.length : col,
                function() { problem = put() || ""; });
        // A paste that was refused says why, and what was cut is still cut.
        if (problem.length > 0) {
            errorDialog.say(problem);
            return false;
        }
        // Cells that were cut have gone to where they were pasted; pasted
        // again, they are copies.
        if (cut) {
            marked = null;
            cutHold.copy = null;
            movedOnce = true;
        }
        return true;
    }

    // `base` made free of the names `taken` with `_001`, `_002`..., and
    // then taken.
    function copyName(base, taken) {
        base = base.replace(/_\d{3}$/, "");
        var n = 1, name;
        do {
            name = base + "_" + ("00" + n).slice(-3);
            n++;
        } while (taken[name]);
        taken[name] = true;
        return name;
    }

    // The name offered for a new column going in as column `at` (-1 for the
    // end), from the one to its left: the next letters on if that is
    // lettered and they are free (B after A, AA after Z), else its name made
    // free with `_001`, `_002`... Put in before the first column there is none to its
    // left, so it is the name of the column it goes before that is made free
    // the same way.
    function suggestedColumnName(at) {
        var cols = sheet.columns;
        if (sheet.isConsts || cols.length === 0)
            return "";
        var taken = namesTaken(-1, -1);
        if (at === 0)
            return copyName(cols[0].name, taken);
        var name = cols[at < 0 ? cols.length - 1 : Math.min(at, cols.length) - 1].name;
        // Only the very next letters, and only if free: put in before B,
        // the next after A is B itself, so it is A made free instead.
        for (var i = 0; i + 1 < maxLetterColumns; i++) {
            if (columnLetters(i) === name && !taken[columnLetters(i + 1)])
                return columnLetters(i + 1);
        }
        return copyName(name, taken);
    }

    // Whole columns of another place are pasted with the whole sheet selected.
    function columnsOnSheet(text) {
        return selKind === "all" && !sheet.isConsts && marked !== null && marked.columns
            && marked.text === text && marked.tab < tabs.length;
    }

    // The name a column copied from sheet `tabFrom` is given where `taken`
    // are the names there: its own if it came from another sheet and no
    // column has it, otherwise a copy's.
    function pastedName(from, tabFrom, taken) {
        if (tabFrom !== tab && !taken[from]) {
            taken[from] = true;
            return from;
        }
        return copyName(from, taken);
    }

    // The names of this sheet's columns, but for `first` to `last`.
    function namesTaken(first, last) {
        var taken = {};
        for (var c = 0; c < sheet.columns.length; c++)
            if (c < first || c > last)
                taken[sheet.columns[c].name] = true;
        return taken;
    }

    // Columns pasted onto one column past the last are added for them, as
    // rows are: named as those they were copied from.
    function growColumns(text, t) {
        if ((selKind !== "column" && !columnsOnSheet(text)) || t[1] !== t[3])
            return "";
        var width = text.split("\n")[0].split("\t").length;
        var need = t[1] + width - sheet.columns.length;
        var m = marked !== null && marked.text === text && marked.columns && marked.tab < tabs.length ? marked : null;
        var taken = namesTaken(-1, -1);
        var names = [];
        for (var i = width - need; i < width; i++) {
            var from = m !== null ? tabs[m.tab].columns[m.left + i] : null;
            names.push(from ? pastedName(from.name, m.tab, taken) : copyName("Column", taken));
        }
        var end = sheet.columns.length;
        for (var k = 0; k < names.length; k++) {
            var problem = file.insertColumn(tab, end + k, names[k]);
            if (problem.length > 0)
                return problem;
        }
        return "";
    }

    // Columns copied and pasted onto others take their names too, each made
    // free with `_001`, `_002`...
    function pastedColumns() {
        var m = marked;
        var whole = selKind === "all";
        if (m === null || !m.columns || (selKind !== "column" && !whole) || m.tab >= tabs.length
            || (m.tab === tab && m.left === selLeft))
            return;
        var count = Math.min(m.right - m.left + 1, whole ? sheet.columns.length : selRight - selLeft + 1);
        var at = whole ? 0 : selLeft;
        var taken = namesTaken(at, at + count - 1);
        var names = [];
        for (var i = 0; i < count; i++) {
            var from = tabs[m.tab].columns[m.left + i];
            if (!from)
                return;
            names.push(pastedName(from.name, m.tab, taken));
        }
        // A name may be one another of these columns has now: each is
        // first given a name no column has, then its own.
        var spare = namesTaken(-1, -1);
        for (var k = 0; k < names.length; k++)
            spare[names[k]] = true;
        var steps = [];
        for (var j = 0; j < names.length; j++)
            if (sheet.columns[at + j].name !== names[j])
                steps.push([at + j, copyName("Pasting", spare), names[j]]);
        keeping(sheet.nrows, sheet.columns.length, function() {
            for (var pass = 1; pass <= 2; pass++) {
                for (var i = 0; i < steps.length; i++) {
                    var problem = file.renameColumn(tab, steps[i][0], steps[i][pass]);
                    if (problem.length > 0) {
                        errorDialog.say(problem);
                        return;
                    }
                }
            }
        });
    }

    // After rows are pasted onto one row, or columns onto one column, they
    // are what is selected.
    function selectPasted(text) {
        if (selKind === "column" && selLeft === selRight) {
            var width = text.split("\n")[0].split("\t").length;
            curCol = selLeft;
            anchorCol = Math.min(sheet.columns.length - 1, selLeft + width - 1);
        }
        pastedColumns();
        if (selKind !== "row" || selTop !== selBottom)
            return;
        var top = selTop;
        curRow = top;
        anchorRow = Math.min(sheet.nrows - 1, top + text.split("\n").length - 1);
        showRow(anchorRow);
    }

    // Scroll to `row` once the list has been put back where it was after an
    // edit that added rows.
    function showRow(row) {
        Qt.callLater(function() { win.showPlace(posOf(row)); });
    }


    // Cut: whole rows or columns are picked up to be moved; cells are copied
    // and stay as they are until the next paste, which moves them.
    // One cell of a formula column, which stands for the whole column.
    readonly property bool formulaCell: !sheet.isConsts && hasCursor
        && selTop === selBottom && selLeft === selRight
        && !!sheet.columns[curCol] && sheet.columns[curCol].computed

    function cutSelection() {
        // A formula column always has its formula: there is nothing to cut.
        if (formulaCell)
            return;
        // Nothing read-only is cut: it is copied, and the bar says so (D208).
        var why = readOnlyWhy();
        if (why.length > 0) {
            copySelection();
            file.status = "Copied, not cut. " + why;
            return;
        }
        notice = "";
        // A whole sheet is picked up, to be moved to where it is pasted.
        if (selKind === "all" && !sheet.isConsts) {
            copySelection();
            marked = null;
            grabbed = { tab: tab, sheet: true };
            return;
        }
        var kind = grabKind;
        if (kind === "" && !sheet.isConsts) {
            grabbed = null;
            cutHold.grab = null;
            var text = file.cutCells(tab, selTop, selLeft, selBottom, selRight);
            mark(text, true);
            clipboard.put(text);
            return;
        }
        copySelection();
        var picked = marked !== null ? marked.text : "";
        // Rows and columns picked up are outlined dashed, to be moved.
        if (kind !== "")
            marked = null;
        if (kind === "row")
            grabbed = { tab: tab, rows: true, first: selTop, count: selBottom - selTop + 1, text: picked };
        else if (kind === "column")
            grabbed = { tab: tab, rows: false, first: selLeft, count: selRight - selLeft + 1, text: picked,
                        names: sheet.columns.slice(selLeft, selRight + 1).map(function(c) { return c.name; }),
                        formulas: sheet.columns.slice(selLeft, selRight + 1).map(function(c) { return c.computed ? c.formula : ""; }) };
        else
            clearSelection();
        cutHold.grab = grabbed;
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
            anchorRow = Math.max(0, sheet.nrows - 1);
            anchorCol = to + g.count - 1;
            curRow = 0;
            curCol = to;
            byRow = false;
            byCol = true;
        }
    }

    function copySelection() {
        grabbed = null;
        cutHold.grab = null;
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
            text = file.copyBlock(tab, selTop, selLeft, selBottom, selRight);
        }
        mark(text);
        clipboard.put(text);
    }

    function clearSelection() {
        if (refusedEdit())
            return;
        if (!sheet.isConsts)
            keeping(0, 0, function() { file.clearCells(tab, selTop, selLeft, selBottom, selRight); });
    }

    // Where a paste goes: `[top, left, bottom, right]`. One whole row takes
    // several rows, and one whole column several columns, as one cell takes
    // a block: from there on, at their own size.
    function pasteTarget(text) {
        // Whole columns pasted onto a whole sheet go in from its corner.
        if (columnsOnSheet(text))
            return [0, 0, 0, 0];
        if (selKind === "row" && selTop === selBottom && text.indexOf("\n") >= 0)
            return [selTop, selLeft, selTop, selLeft];
        if (selKind === "column" && selLeft === selRight && text.split("\n")[0].indexOf("\t") >= 0)
            return [selTop, selLeft, selTop, selLeft];
        // Columns are pasted whole: rows past the sheet's last are added.
        if (selKind === "column" && (text.indexOf("\n") >= 0 || text.indexOf("\t") >= 0))
            return [selTop, selLeft, Math.max(selBottom, selTop + text.split("\n").length - 1), selRight];
        return [selTop, selLeft, selBottom, selRight];
    }

    // A view of a file: its own columns are fixed and formula columns go
    // among them.
    readonly property bool frameSheet: !sheet.isConsts && !sheet.view && (sheet.why || "").length > 0

    // The sheet shown, copied in just after it and shown.
    function duplicateShownSheet() {
        if (sheet.isConsts || tabs.length === 0)
            return;
        var at = tab;
        var problem = file.duplicateSheet(at);
        if (problem.length > 0)
            errorDialog.say(problem);
        else
            switchTab(at + 1);
    }

    // The sheet shown moved one place left or right.
    function moveShownSheet(by) {
        if (!sheet.isConsts)
            dropTab(tab, tab + by);
    }

    // A sheet's tab dragged to a new place: sheet `from` becomes sheet `to`.
    // The sheet shown stays the sheet shown, wherever it has gone to, and the
    // constants stay last, as a sheet of their own.
    function dropTab(from, to) {
        if (from === to || from < 0 || to < 0 || from >= tabs.length || to >= tabs.length
                || tabs[from].isConsts || tabs[to].isConsts)
            return;
        applySource();
        var shown = tab === from ? to
            : from < tab && to >= tab ? tab - 1
            : from > tab && to <= tab ? tab + 1
            : tab;
        var problem = file.moveSheet(from, to);
        if (problem.length > 0) {
            errorDialog.say(problem);
            return;
        }
        if (shown !== tab)
            switchTab(shown);
    }

    function pasteSelection() {
        notice = "";
        if (!pasteFits) {
            notice = pasteFitsNot;
            return;
        }
        if (grabbedHere) {
            dropGrabbed();
            return;
        }
        // A whole sheet cut is moved to where the sheet selected is.
        if (grabbed !== null && grabbed.sheet) {
            var g = grabbed;
            grabbed = null;
            if (selKind !== "all" || sheet.isConsts || g.tab === tab)
                return;
            var to = tab;
            var moved = file.moveSheet(g.tab, to);
            if (moved.length > 0) {
                errorDialog.say(moved);
                return;
            }
            switchTab(to);
            selectAll();
            return;
        }
        // Columns go in among a file's own, which take no edit themselves.
        if (!(handCols && frameSheet) && refusedEdit())
            return;
        // Rows or columns cut on another sheet go in before the selection
        // and are taken out there.
        if (rowsOnHand !== null && rowsOnHand.elsewhere) {
            insertRowsOnHand(false);
            return;
        }
        if (colsOnHand !== null && (colsOnHand.elsewhere || frameSheet)) {
            insertColumnsOnHand(false);
            return;
        }
        grabbed = null;
        var text = clipboard.take();
        // The paste and the names it gives columns are undone as one.
        var mark = file.undoMark();
        if (text.length === 0)
            return;
        // A whole sheet copied onto another whole sheet makes it a copy.
        var m = marked;
        if (m !== null && m.sheet && m.text === text && selKind === "all" && m.tab !== tab
            && m.tab < tabs.length && !tabs[m.tab].isConsts && !sheet.isConsts) {
            var problem = "";
            keeping(0, 0, function() { problem = file.pasteSheet(m.tab, tab); });
            if (problem.length > 0)
                errorDialog.say(problem);
            selectAll();
            return;
        }
        if (pasteWith(text, function() {
            var t = pasteTarget(text);
            if (!sheet.isConsts)
                return growColumns(text, t) || file.pasteCells(tab, t[0], t[1], t[2], t[3], text);
            if (curCol === 1)
                file.setConst(curRow, text.split("\n")[0]);
        }))
            selectPasted(text);
        file.joinSince(mark);
    }

    // Paste what the copied cells showed, not their formulas.
    function pasteValues() {
        notice = "";
        if (!pasteFits) {
            notice = pasteFitsNot;
            return;
        }
        if (refusedEdit())
            return;
        grabbed = null;
        var text = clipboard.take();
        // The paste and the names it gives columns are undone as one.
        var mark = file.undoMark();
        if (text.length > 0 && !sheet.isConsts && pasteWith(text, function() {
                var t = pasteTarget(text);
                return growColumns(text, t) || file.pasteValues(tab, t[0], t[1], t[2], t[3], text);
            }))
            selectPasted(text);
        file.joinSince(mark);
    }

    // Paste what was copied turned about: its rows as columns.
    // Several cells copied or cut, to be turned about onto cells: put down
    // from the one selected, or the corner of a block.
    // Not when any of them is in a formula column.
    // Text from another window or program has no mark here to say what it
    // is, so it is tried: a paste that cannot be made says why.
    readonly property bool canTranspose: selKind === "cells" && !sheet.isConsts
        && (marked !== null
            ? (marked.top !== marked.bottom || marked.left !== marked.right)
                && !spansFormula(marked)
            : clipboard.canPaste)

    // Values, not when the cells were in a formula column, or are put in one.
    // Whole rows or columns waiting go only onto whole rows or columns, as
    // many as are selected, and nothing else goes onto those.
    readonly property bool handRows: rowsOnHand !== null
        || (marked === null && grabbed === null && cutHold.copy !== null && cutHold.copy.rows)
    readonly property bool handCols: colsOnHand !== null
        || (marked === null && grabbed === null && cutHold.copy !== null && cutHold.copy.columns)
    readonly property bool pasteFits: (!handRows || selKind === "row") && (!handCols || selKind === "column")
    readonly property string pasteFitsNot: handRows ? "Whole rows paste onto whole rows."
        : "Whole columns paste onto whole columns."
    readonly property bool canPasteValues: canPaste && !formulaCell && pasteFits
        && !(marked !== null && spansFormula(marked))

    function spansFormula(m) {
        var from = m.tab < tabs.length ? tabs[m.tab] : null;
        if (!from || from.isConsts)
            return false;
        for (var c = m.left; c <= m.right; c++)
            if (from.columns[c] && from.columns[c].computed)
                return true;
        return false;
    }

    function pasteTransposed() {
        notice = "";
        if (refusedEdit())
            return;
        grabbed = null;
        var text = clipboard.take();
        if (text.length > 0 && !sheet.isConsts)
            pasteWith(text, function() { return file.pasteTransposed(tab, selTop, selLeft, selBottom, selRight, text); });
    }

    // Cells copied or cut here can be pasted as a reference to them, in a
    // sheet's data cells or in a formula column.
    // From another window it is the app's memory of the last copy that says,
    // when the paste is made, whether there are cells to refer to.
    readonly property bool canPasteReference: selKind !== "all" && !sheet.isConsts
        && (marked !== null
            ? marked.tab < tabs.length && !tabs[marked.tab].isConsts
            : clipboard.canPaste)

    // Paste a live reference to what was copied, not its values (D210).
    function pasteReference() {
        notice = "";
        if (refusedEdit())
            return;
        grabbed = null;
        var text = clipboard.take();
        if (text.length > 0 && !sheet.isConsts)
            pasteWith(text, function() { return file.pasteReference(tab, selTop, selLeft, selBottom, selRight, text, selKind === "row"); });
    }

    // A new sheet beside the one shown: asked for by name.
    function newSheet(after) {
        var here = sheet.isConsts || tabs.length === 0 ? snap.sheets.length : tab;
        promptDialog.ask("sheet", after ? here + 1 : here);
    }

    // Asked first: a sheet goes with all its cells.
    function deleteSheet() {
        if (sheet.isConsts || tabs.length === 0)
            return;
        stopEditing();
        deleteSheetDialog.open();
    }

    function deleteSheetNow() {
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

    // A constant is deleted from the Constants sheet, asked first: a
    // formula that reads it is left showing an error.
    property int constGone: -1
    function deleteConstant() {
        if (!sheet.isConsts || curRow < 0 || curRow >= sheet.nrows)
            return;
        stopEditing();
        constGone = curRow;
        deleteConstDialog.open();
    }

    function deleteConstantNow() {
        if (!sheet.isConsts || constGone < 0 || constGone >= sheet.nrows)
            return;
        var at = constGone;
        notice = file.deleteConst(at);
        if (notice.length > 0)
            return;
        // The Constants sheet is gone with its last constant.
        if (sheet.isConsts)
            selectCell(Math.min(at, sheet.nrows - 1), 0, false);
    }

    function deleteAllConstants() {
        stopEditing();
        deleteAllConstsDialog.open();
    }

    function deleteAllConstantsNow() {
        file.deleteAllConsts();
        if (tabs.length > 0 && sheet.isConsts)
            switchTab(0);
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

    // A format a column of its type takes, to show what one looks like.
    function formatExample(c) {
        var type = sheet.columns[c] ? sheet.columns[c].type : "";
        return type === "Date" ? "%d %b %Y"
             : type === "Time" ? "%H:%M"
             : type === "Datetime" ? "%d %b %Y %H:%M"
             : "'.2f";
    }

    // Whether every selected column is of a type that takes a format.
    function allFormattable() {
        for (var c = selLeft; c <= selRight; c++) {
            var column = sheet.columns[c];
            if (!column || ["Integer", "Hex", "Octal", "Binary", "Decimal", "Float", "Date", "Time", "Datetime"].indexOf(column.type) < 0)
                return false;
        }
        return true;
    }

    // Whether a calculated column is among the columns selected.
    function anyComputed() {
        for (var c = selLeft; c <= selRight; c++) {
            if (sheet.columns[c] && sheet.columns[c].computed)
                return true;
        }
        return false;
    }

    function anyFormatted() {
        for (var c = selLeft; c <= selRight; c++) {
            if (sheet.columns[c] && sheet.columns[c].format)
                return true;
        }
        return false;
    }

    function anyFiltered() {
        for (var c = selLeft; c <= selRight; c++) {
            if (sheet.columns[c] && filtered(c))
                return true;
        }
        return false;
    }

    // Take the format off the selected columns, or the filter.
    function clearFormats() {
        if (sheet.isConsts)
            return;
        var problem = file.setColumnFormat(tab, selLeft, selRight - selLeft + 1, "");
        if (problem.length > 0)
            errorDialog.say(problem);
    }
    function clearFilters() {
        var all = JSON.parse(JSON.stringify(filters));
        var f = all[sheet.name] || {};
        for (var c = selLeft; c <= selRight; c++) {
            if (sheet.columns[c])
                f[sheet.columns[c].name] = "";
        }
        all[sheet.name] = f;
        busy("Filtering\u2026", function() {
            filters = all;
            filterRev++;
        });
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
    // The whole sheet's menu, above its tab.
    function sheetMenu() {
        var tabItem = tabRepeater.itemAt(tab);
        if (tabItem) {
            cellMenu.popup(tabItem, 0, -cellMenu.implicitHeight);
            // Its sections fit what is selected only once it is open: lift
            // it clear of the tabs at its full height.
            Qt.callLater(function() {
                var at = tabItem.mapToItem(cellMenu.parent, 0, 0);
                cellMenu.x = at.x;
                cellMenu.y = at.y - cellMenu.height - 4;
            });
        } else
            cellMenu.popup();
    }

    // Show the sheet of a tab with the whole of it selected, as it would be
    // to act on it from the grid: what the entries of its menu act on, but
    // for opening it in a window of its own.
    function goToSheet(index) {
        if (index < 0 || index >= tabs.length)
            return;
        viewSource(false);
        if (index !== tab)
            switchTab(index);
        if (!sheet.isConsts)
            selectAll();
    }

    // The cell a problem is in, from the problems list. A problem no cell
    // holds has nowhere to go.
    function goToProblem(item) {
        if (!item || item.sheet < 0 || item.sheet >= tabs.length)
            return;
        viewSource(false);
        if (item.sheet !== tab)
            switchTab(item.sheet);
        selectCell(item.row, item.col, false);
    }

    function keyMenu() {
        if (tabs.length === 0)
            return;
        if (selKind === "all" && !sheet.isConsts) {
            sheetMenu();
            return;
        }
        var x = gutterWidth - hflick.contentX;
        for (var c = 0; c < curCol; c++)
            x += colWidths[c];
        var y = headerHeight + (posOf(curRow) + 1) * rowHeight - rowsView.contentY;
        cellMenu.popup(grid, Math.max(0, Math.min(grid.width - cellMenu.width, x)),
                       Math.max(headerHeight, Math.min(grid.height - rowHeight, y)));
    }

    function insertRow(below) {
        insertRows(below, 1);
    }

    // A new row after the last, wherever the selection is.
    function appendRow() {
        insertRows(true, 1, true);
    }

    function insertRows(below, count, atEnd) {
        if (sheet.isConsts || count < 1)
            return;
        var at = sheet.nrows === 0 ? 0 : (atEnd ? sheet.nrows : (below ? selBottom + 1 : selTop));
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
        // Shown down to the last of them.
        showRow(at + count - 1);
    }

    // Rows go in above or below the selection: the ones cut or copied as
    // whole rows if any are waiting, else `count` blank ones.
    function insertRowsHere(below, count) {
        if (rowsOnHand !== null)
            insertRowsOnHand(below);
        else
            insertRows(below, count);
    }

    // What to say when whole rows or columns pasted do not fit the sheet they
    // go into: rows wider than it are clipped, and columns longer than it
    // make it longer.
    function lengthNote(text, rows) {
        var lines = text.split("\n");
        if (rows)
            return lines[0].split("\t").length > sheet.columns.length
                ? "Source length is longer than target length, clipping cols." : "";
        return lines.length > sheet.nrows
            ? "Source length is longer than target length, adding rows." : "";
    }

    // Put the rows cut or copied as whole rows in above or below the
    // selection, where blank ones would go: cut rows move there, and copied
    // rows go in as copies, still on the clipboard to go in again.
    function insertRowsOnHand(below) {
        var hand = rowsOnHand;
        if (hand === null || sheet.isConsts)
            return;
        notice = "";
        var at = sheet.nrows === 0 ? 0 : (below ? selBottom + 1 : selTop);
        var count = hand.count;
        var whole = selKind === "row";
        var col = selLeft;
        var said = "";
        if (hand.elsewhere) {
            var from = foreign;
            grabbed = null;
            cutHold.grab = null;
            said = lengthNote(from.text, true);
            var m0 = file.undoMark();
            var bad = "";
            keeping(at, 0, function() {
                file.insertRows(tab, at, count);
                bad = file.pasteCells(tab, at, 0, at, 0, from.text);
                if (bad.length === 0)
                    file.deleteRows(from.tab, from.first, from.count);
            });
            file.joinSince(m0);
            if (bad.length > 0) {
                errorDialog.say(bad);
                return;
            }
        } else if (hand.cut) {
            var g = grabbed;
            grabbed = null;
            // Where they are already: nothing to move.
            if (at >= g.first && at <= g.first + g.count)
                return;
            // Once they are up, the rows above them have closed up.
            at = at < g.first ? at : at - g.count;
            notice = file.moveRows(tab, g.first, g.count, at);
            if (notice.length > 0)
                return;
        } else {
            // The rows and what goes in them are undone as one. They go
            // down from the first cell of the first new row, whole.
            var text = marked.text;
            said = lengthNote(text, true);
            var mark = file.undoMark();
            var problem = "";
            keeping(at, 0, function() {
                file.insertRows(tab, at, count);
                problem = file.pasteCells(tab, at, 0, at, 0, text);
            });
            file.joinSince(mark);
            if (problem.length > 0) {
                errorDialog.say(problem);
                return;
            }
        }
        // Leave them selected, as whole rows if whole rows were.
        if (whole) {
            selectLine(true, at, false);
            selectLine(true, at + count - 1, true);
        } else {
            selectCell(at, col, false);
            selectCell(at + count - 1, col, true);
        }
        showRow(at + count - 1);
        if (said.length > 0)
            notice = said;
    }

    // Columns go in before or after the selection: the ones cut or copied as
    // whole columns if any are waiting, else one new one, asked for by name.
    // The letters a spreadsheet gives column `n` (from 0): A to Z, then AA
    // to ZZ, which is as far as a sheet kept in letters goes.
    function columnLetters(n) {
        var a = String.fromCharCode(65 + n % 26);
        return n < 26 ? a : String.fromCharCode(65 + Math.floor(n / 26) - 1) + a;
    }
    readonly property int maxLetterColumns: 702
    // Whether the sheet's columns are named A, B, C... in order, as a new
    // sheet's are: more of them can then be added by number, carrying on
    // from the last.
    readonly property bool letterSheet: {
        var cols = sheet.columns;
        if (sheet.isConsts || cols.length === 0 || cols.length > maxLetterColumns)
            return false;
        for (var i = 0; i < cols.length; i++) {
            if (cols[i].name !== columnLetters(i))
                return false;
        }
        return true;
    }

    function insertColumnsHere(after) {
        var count = selRight - selLeft + 1;
        if (colsOnHand !== null)
            insertColumnsOnHand(after);
        else if (count > 1 && !sheet.isConsts && !frameSheet)
            insertBlankColumns(after ? selRight + 1 : selLeft, count);
        else
            promptDialog.ask("column", after ? selRight + 1 : selLeft);
    }

    // `count` blank columns at `at`, each named as the one to its left would
    // be offered, in one step to undo, and left selected.
    function insertBlankColumns(at, count) {
        var mark = file.undoMark();
        var problem = "";
        keeping(0, at, function() {
            for (var k = 0; k < count && problem.length === 0; k++)
                problem = file.insertColumn(tab, at + k, suggestedColumnName(at + k));
        });
        file.joinSince(mark);
        if (problem.length > 0)
            errorDialog.say(problem);
        selectLine(false, at, false);
        selectLine(false, at + count - 1, true);
    }

    // Put the columns cut or copied as whole columns in before or after the
    // selection: cut columns move there, and copied ones go in as copies,
    // named as those they came from, each made free with `_001`, `_002`...
    function insertColumnsOnHand(after) {
        var hand = colsOnHand;
        if (hand === null || sheet.isConsts)
            return;
        notice = "";
        var at = after ? selRight + 1 : selLeft;
        var count = hand.count;
        var said = "";
        if (hand.elsewhere) {
            var src = foreign;
            grabbed = null;
            cutHold.grab = null;
            said = lengthNote(src.text, false);
            var taken2 = namesTaken(-1, -1);
            var names2 = src.names.map(function(n) { return pastedName(n, src.tab, taken2); });
            var m0 = file.undoMark();
            var bad = "";
            var onlyFormulas = src.formulas !== undefined && src.formulas.every(function(f) { return f && f.length > 0; });
            if (frameSheet && !onlyFormulas)
                bad = "Only formula columns go into a file's sheet.";
            keeping(0, at, function() {
                if (bad.length > 0)
                    return;
                if (frameSheet) {
                    for (var j = 0; j < names2.length && bad.length === 0; j++)
                        bad = file.insertComputed(tab, at + j, names2[j], src.formulas[j]);
                } else {
                    for (var k = 0; k < names2.length && bad.length === 0; k++)
                        bad = file.insertColumn(tab, at + k, names2[k]);
                    if (bad.length === 0)
                        bad = file.pasteCells(tab, 0, at, 0, at, src.text);
                }
                if (bad.length === 0)
                    bad = file.deleteColumns(src.tab, src.first, src.count);
            });
            file.joinSince(m0);
            if (bad.length > 0) {
                errorDialog.say(bad);
                return;
            }
        } else if (hand.cut) {
            var g = grabbed;
            grabbed = null;
            if (at >= g.first && at <= g.first + g.count)
                return;
            at = at < g.first ? at : at - g.count;
            notice = file.moveColumns(tab, g.first, g.count, at);
            if (notice.length > 0)
                return;
        } else {
            var m = marked;
            var from = tabs[m.tab].columns;
            var taken = namesTaken(-1, -1);
            var names = [];
            for (var i = 0; i < count; i++)
                names.push(copyName(from[m.left + i] ? from[m.left + i].name : "Column", taken));
            var text = m.text;
            said = lengthNote(text, false);
            var mark = file.undoMark();
            var problem = "";
            var formulas = [];
            for (var q = 0; q < count; q++) {
                var fc = from[m.left + q];
                formulas.push(fc && fc.computed ? fc.formula : "");
            }
            if (frameSheet && !formulas.every(function(f) { return f.length > 0; }))
                problem = "Only formula columns go into a file's sheet.";
            keeping(0, at, function() {
                if (problem.length > 0)
                    return;
                if (frameSheet) {
                    for (var j = 0; j < names.length && problem.length === 0; j++)
                        problem = file.insertComputed(tab, at + j, names[j], formulas[j]);
                    return;
                }
                for (var k = 0; k < names.length && problem.length === 0; k++)
                    problem = file.insertColumn(tab, at + k, names[k]);
                if (problem.length === 0)
                    problem = file.pasteCells(tab, 0, at, 0, at, text);
            });
            file.joinSince(mark);
            if (problem.length > 0) {
                errorDialog.say(problem);
                return;
            }
        }
        selectLine(false, at, false);
        selectLine(false, at + count - 1, true);
        if (said.length > 0)
            notice = said;
    }

    function deleteRows() {
        if (sheet.isConsts || sheet.nrows === 0)
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
        commitEditing();
        grid.forceActiveFocus();
        var maxRow = Math.max(0, sheet.nrows - 1);
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
            places = ({});
            tab = 0;
            switchTab(0);
        } else if (action === "open") {
            openDialog.show();
        } else if (action === "openUrl") {
            var url = pendingUrl;
            busy("Opening\u2026", function() {
                if (file.openUrl(url)) {
                    places = ({});
                    tab = 0;
                    switchTab(0);
                    // What an import changed or left behind.
                    if (file.notes.length > 0)
                        notesDialog.open();
                }
            });
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
        // The file in use is the one the next Open starts from.
        function onFilePathChanged() {
            if (file.filePath.length > 0)
                file.keepLastFile(file.filePath);
        }
    }

    Component.onCompleted: {
        reload();
        if (file.filePath.length > 0)
            file.keepLastFile(file.filePath);
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
        || renamingTab >= 0

    Shortcut { sequence: "Ctrl+S"; onActivated: win.save() }
    Shortcut { sequence: "Ctrl+Shift+S"; onActivated: saveDialog.open() }
    Shortcut { sequence: "Ctrl+O"; onActivated: win.guard("open") }
    Shortcut { sequence: "Ctrl+E"; onActivated: exportChoice.open() }
    Shortcut { sequence: "Ctrl+N"; enabled: !win.typing; onActivated: win.newSheet(true) }
    Shortcut { sequence: "Ctrl+D"; enabled: !win.typing; onActivated: win.duplicateShownSheet() }
    Shortcut { sequence: "Ctrl+Alt+Shift+Left"; enabled: !win.typing; onActivated: win.moveShownSheet(-1) }
    Shortcut { sequence: "Ctrl+Alt+Shift+Right"; enabled: !win.typing; onActivated: win.moveShownSheet(1) }
    Shortcut { sequence: "Ctrl+K"; enabled: !win.typing; onActivated: promptDialog.ask("const") }
    Shortcut { sequence: "Ctrl+Shift+N"; onActivated: win.newWindow(win.tab) }
    Shortcut { sequence: "Ctrl+Alt+N"; onActivated: win.guard("new") }
    // Cmd+W on a Mac (Qt reads Cmd as Ctrl there); the unsaved-changes check
    // in onClosing still applies.
    Shortcut { sequence: "Ctrl+W"; onActivated: win.close() }
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
            // Above the grid, which comes next in the layout and would
            // otherwise be drawn over it: the formula box opens past this
            // bar's own height, and what is over the cells takes the
            // clicks there too.
            z: 2

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
                    // Where the file is, said in the footer while the pointer is on it.
                    HoverHandler {
                        id: nameHover
                        onHoveredChanged: win.hover(nameHover, hovered ? file.filePath : "", false)
                    }
                }

                Text {
                    readonly property var column: win.hasCursor || win.oneFormula ? win.sheet.columns[win.curCol] : undefined
                    // The selection, as a formula would write it. Several
                    // columns go by position: a range is not written with
                    // names.
                    readonly property string across: win.selRight > win.selLeft
                        ? win.selLeft + ".." + win.selRight
                        : win.selLeft < win.sheet.columns.length ? win.sheet.columns[win.selLeft].name : ""
                    readonly property string down: win.selBottom > win.selTop
                        ? win.selTop + ".." + win.selBottom : "" + win.selTop
                    // `:=` marks a formula: the sheet's own where the sheet
                    // is calculated from one, the column's where the cursor
                    // is in a calculated column, and neither for a cell
                    // that holds what is written in it.
                    readonly property string mark: win.sheet.view || (column && column.computed) ? " :=" : ""
                    text: win.sheet.isConsts
                        ? (column && win.curRow < win.sheet.nrows ? win.sheet.rows[win.curRow][0].d : "")
                        : win.sheet.view ? win.sheet.name + mark
                        : across.length === 0 ? ""
                        : win.sheet.name + (win.selKind === "all" ? ""
                            : win.selKind === "row" ? "[; " + down + "]"
                            : win.selKind === "column" ? "[" + across + "]"
                            : "[" + across + "; " + down + "]") + mark
                    color: win.mutedColor
                    font: win.font
                    Layout.minimumWidth: 150
                    elide: Text.ElideRight
                }

                // The formula box. One line until the caret beside it is
                // pressed, then four with the rest scrolled, for a formula
                // written over several lines. A `TextArea` rather than a
                // `TextField` for that: Enter still commits, and Alt+Enter
                // is the newline, as a spreadsheet has it.
                // The caret that opens the box, kept to the left of it: the
                // box grows downwards and would cover one on the right.
                Text {
                    text: win.formulaOpen ? "\u2303" : "\u2304"
                    color: win.mutedColor
                    font.family: win.font.family
                    font.pixelSize: win.fontSize
                    Layout.alignment: Qt.AlignVCenter
                    Layout.leftMargin: 2
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -5
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            // Open, the box takes the keyboard, so its lines
                            // can be reached; closed, the grid has it back.
                            if (win.formulaOpen) {
                                win.formulaOpen = false;
                                grid.forceActiveFocus();
                            } else {
                                win.formulaOpen = true;
                                formulaField.forceActiveFocus();
                            }
                        }
                    }
                }

                // The slot the formula box sits in. It keeps the one line's
                // height whether the box is open or not, so nothing else in
                // the header moves; the box itself is drawn over the sheet
                // below, with a background to lift it off the cells and its
                // own clicks, the grid having none of it.
                Item {
                    id: formulaSlot
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.round(win.fontSize * 1.45) + 8
                    Layout.alignment: Qt.AlignVCenter
                    z: 60

                    Rectangle {
                        anchors.fill: formulaScroll
                        visible: win.formulaOpen
                        color: win.overColor
                        border.width: 1
                        border.color: win.lineColor
                        radius: 3
                    }

                    ScrollView {
                        id: formulaScroll
                        anchors.left: parent.left
                        // To the right edge in a narrow window; in a wide one
                        // a third of the window is left clear to its right,
                        // though never so much that it is under forty
                        // characters wide.
                        width: Math.min(parent.width, Math.max(win.fontSize * 40, parent.width - win.width / 3))
                        anchors.top: parent.top
                        height: win.formulaOpen
                            ? Math.round(win.fontSize * 4 * 1.45) + 8
                            : parent.height
                        clip: true
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                        ScrollBar.vertical.policy: win.formulaOpen
                            ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff

                        TextArea {
                            id: formulaField
                            font: win.font
                            color: win.textColor
                            selectByMouse: true
                            topPadding: 2
                            bottomPadding: 2
                            readonly property var cell: win.hasCursor || win.oneFormula ? win.cellAt(win.curRow, win.curCol) : null
                            // A calculated sheet has one formula, as a calculated
                            // column does: the box shows it whichever cell the
                            // cursor is in, and an edit is of the formula.
                            // A formula over several lines is shown with the
                            // four spaces the file joins its continuations
                            // by, whatever it was indented with: the box is
                            // where it is written, so the indent is kept
                            // here rather than read.
                            readonly property string source: win.sheet.view
                                ? win.sheet.view.replace(/\n[ \t]*/g, "\n    ")
                                : cell ? cell.s : ""
                            // Closed, the box is one line tall and holds the
                            // first line: the others would show as the top
                            // of a second line under it. The glyph on the
                            // right is what says there is more.
                            readonly property string shown: win.formulaOpen ? source : source.split("\n")[0]
                            readonly property bool locked: win.sheet.isConsts && win.curCol !== 1
                            readOnly: locked || (!cell && !win.sheet.view)
                            // Shown too while a cell is edited, where its cursor is there.
                            cursorVisible: activeFocus || win.editing
                            placeholderText: cell || win.sheet.view || win.selKind !== "cells"
                                ? "" : "No cell selected"
                            background: Item {}
                            // Closed, it is one line: the text runs on rather
                            // than wrapping, and the caret says there is more.
                            wrapMode: win.formulaOpen ? TextEdit.Wrap : TextEdit.NoWrap
                            onSourceChanged: if (!activeFocus) win.resetBox()
                            // It opens when it takes the keyboard holding
                            // more than one line, and closes when it loses
                            // it, so a click anywhere else closes it. A
                            // formula of one line is edited in the closed
                            // box. Open it holds every line, closed
                            // the first, and what is committed is what the
                            // box held while it had the keyboard.
                            onActiveFocusChanged: {
                                if (activeFocus) {
                                    win.editor = formulaField;
                                    if (win.manyLines())
                                        win.formulaOpen = true;
                                    text = source;
                                } else {
                                    if (win.editor === formulaField)
                                        win.editor = null;
                                    win.formulaOpen = false;
                                    win.resetBox();
                                }
                            }
                            // Enter commits, whether the box is open or closed.
                            // Alt+Enter puts a newline in, which the file reads
                            // as a continuation of the line above.
                            Keys.onPressed: function(event) {
                                if (event.key !== Qt.Key_Return && event.key !== Qt.Key_Enter)
                                    return;
                                event.accepted = true;
                                if (event.modifiers & Qt.AltModifier) {
                                    // The four spaces a continuation line is
                                    // indented by go in with the newline, so
                                    // what is typed here is what the file
                                    // holds.
                                    formulaField.insert(formulaField.cursorPosition, "\n    ");
                                    if (!win.formulaOpen)
                                        win.formulaOpen = true;
                                    return;
                                }
                                if (!formulaField.readOnly)
                                    win.commit(formulaField.text, 0, 0);
                                win.resetBox();
                            }
                            // The grid taking the keyboard back is what puts
                            // the box's text and its height back with it.
                            Keys.onEscapePressed: grid.forceActiveFocus()
                        }
                    }

                    // More formula than the one line shows.
                    Text {
                        visible: !win.formulaOpen && (formulaField.contentWidth > formulaField.width
                            || formulaField.source.indexOf("\n") >= 0)
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        z: 2
                        text: "\u2026"
                        color: win.mutedColor
                        font: win.font
                    }
                }

                // What the column accepts, in the locale's way of writing it:
                // nothing for a formula column, whose cells are not typed in.
                Text {
                    readonly property var column: win.hasCursor ? win.sheet.columns[win.curCol] : undefined
                    readonly property string hint: column && column.type && !column.computed && !win.sheet.isConsts
                        ? file.entryHint(column.type) : ""
                    visible: hint.length > 0
                    text: hint
                    color: win.textColor
                    font: win.font
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
                case Qt.Key_PageUp: win.selectCell(win.rowAt(Math.max(0, win.posOf(win.curRow) - page)), win.curCol, shift); break;
                case Qt.Key_PageDown: win.selectCell(win.rowAt(Math.min(win.shownCount - 1, win.posOf(win.curRow) + page)), win.curCol, shift); break;
                case Qt.Key_Home: win.selectCell(ctrl ? 0 : win.curRow, 0, shift); break;
                case Qt.Key_End: win.selectCell(ctrl ? win.sheet.nrows - 1 : win.curRow, win.sheet.columns.length - 1, shift); break;
                case Qt.Key_Tab: win.selectCell(win.curRow, win.curCol + 1, false); break;
                case Qt.Key_Backtab: win.selectCell(win.curRow, win.curCol - 1, false); break;
                // F2 edits the cell in place, as Enter does a single cell.
                case Qt.Key_F2:
                    if (win.selKind === "cells" && win.selTop === win.selBottom && win.selLeft === win.selRight)
                        win.startEditing(null);
                    break;
                case Qt.Key_Return:
                case Qt.Key_Enter:
                    // A row goes in beside whole rows, or one cell, and a
                    // column beside whole columns, and nothing else.
                    if (ctrl) {
                        if (win.selKind === "row")
                            win.insertRowsHere(!shift, win.selBottom - win.selTop + 1);
                        else if (win.selKind === "column")
                            win.insertColumnsHere(!shift);
                        else if (win.selKind === "cells" && win.selTop === win.selBottom && win.selLeft === win.selRight)
                            win.insertRowsHere(!shift, 1);
                    }
                    // Only a single cell is edited in place: a block, rows,
                    // columns or the sheet are left as they are.
                    else if (win.selKind === "cells" && win.selTop === win.selBottom && win.selLeft === win.selRight)
                        win.startEditing(null);
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
                case Qt.Key_Escape:
                    // Rows, columns or the sheet copied or cut: Esc lets go of
                    // them, and leaves them selected for another Esc.
                    if (win.outline !== null && ((win.outline.columns && win.selKind === "column")
                                                 || (win.outline.rows && win.selKind === "row")
                                                 || (win.sheetMark !== null && win.selKind === "all")))
                        win.unmark();
                    else
                        win.deselect();
                    break;
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
                    } else if (ctrl && shift && event.key === Qt.Key_T) {
                        if (win.canTranspose) win.pasteTransposed();
                    } else if (ctrl && shift && event.key === Qt.Key_R) {
                        if (win.canPasteReference) win.pasteReference();
                    } else if (ctrl && event.key === Qt.Key_V) {
                        if (shift) {
                            if (!win.marked || !win.spansFormula(win.marked)) win.pasteValues();
                        }
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
                anchors.bottomMargin: hbar.needed ? hbar.height : 0
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
                            // The whole sheet copied or cut is outlined round its margins.
                            readonly property bool marked: win.outline !== null && win.outline.rows && win.outline.columns
                            MarkEdge { visible: parent.marked; width: parent.width; height: 1 }
                            MarkEdge { visible: parent.marked; width: 1; height: parent.height }
                            MouseArea {
                                anchors.fill: parent
                                enabled: !win.sheet.isConsts
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                // A right-click opens the sheet's menu, above its tab.
                                onClicked: function(mouse) {
                                    win.commitEditing();
                                    win.selectAll();
                                    if (mouse.button === Qt.RightButton) {
                                        grid.forceActiveFocus();
                                        win.sheetMenu();
                                    }
                                }
                            }
                        }
                        Repeater {
                            id: headCells
                            model: win.sheet.columns.length
                            Rectangle {
                                id: headCell
                                required property int index
                                readonly property var column: win.sheet.columns[index]
                                // The name is being typed over, in place.
                                property bool renaming: false
                                Connections {
                                    target: win
                                    function onRenameColumnAsked(col) {
                                        if (col === headCell.index)
                                            headCell.renaming = true;
                                    }
                                }
                                width: win.colWidths[index] || 0
                                height: win.headerHeight
                                // Stronger while the sort and filter box beside it is open.
                                color: columnPopup.visible && columnPopup.col === index
                                    ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.4)
                                    : index >= win.selLeft && index <= win.selRight
                                    ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.16)
                                    : win.panelColor
                                Column {
                                    id: headLabels
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: 7
                                    width: parent.width - 14
                                    Text {
                                        id: nameLine
                                        // Clear of the telltale, which stands to its right.
                                        width: parent.width - filterButton.width
                                        text: headCell.column ? headCell.column.name : ""
                                        color: win.textColor
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.fontSize
                                        font.bold: true
                                        elide: Text.ElideRight
                                    }
                                    // The type, always muted, with a mark right before it in
                                    // the accent colour: `:` where the type is set, `=` in a
                                    // formula column, and `:=` where it is both. `[]` follows
                                    // it where it has a format, which the context menu shows
                                    // and changes.
                                    Text {
                                        id: typeLine
                                        readonly property bool computed: !!headCell.column && headCell.column.computed && !win.sheet.isConsts
                                        readonly property bool declared: !!headCell.column && headCell.column.declared
                                        readonly property string mark: (computed ? "=" : "") + (declared ? ":" : "")
                                        readonly property bool formatted: !!headCell.column && !!headCell.column.format
                                        width: parent.width
                                        textFormat: Text.StyledText
                                        text: !headCell.column ? ""
                                            : (mark.length > 0 ? "<font color=\"" + win.accentColor + "\">" + mark + "</font>" : "")
                                              + headCell.column.type + (formatted ? "[]" : "")
                                        color: win.mutedColor
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.noteSize
                                        elide: Text.ElideNone
                                    }
                                }
                                // The telltale to the right of the name and a little
                                // below its middle, which opens the column's sort and
                                // filter, with room for a caret above or below it: an
                                // empty circle unless
                                // the column is filtered, when it is solid; a sort
                                // is shown by the caret alone, an up caret above the
                                // circle once sorted ascending and a down caret
                                // below it once sorted descending.
                                Item {
                                    id: filterButton
                                    // Above the header's own mouse area, which
                                    // comes after it and covers the whole cell.
                                    z: 2
                                    visible: !win.sheet.isConsts
                                    readonly property bool active: win.filtered(headCell.index)
                                    readonly property string sorted: win.sortedAs(headCell.index)
                                    readonly property bool solid: active
                                    width: visible ? win.noteSize : 0
                                    height: win.noteSize + 2
                                    anchors.right: parent.right
                                    anchors.rightMargin: 4
                                    y: headLabels.y + nameLine.height / 2 + 3 - height / 2
                                    Text {
                                        id: dot
                                        anchors.centerIn: parent
                                        text: filterButton.solid ? "\u25CF" : "\u25CB"
                                        color: filterButton.solid ? win.accentColor : win.mutedColor
                                        font.pixelSize: win.noteSize
                                    }
                                    // The sort's caret: one chevron, drawn here rather than
                                    // set in a font, whose up and down carets differ in
                                    // weight and in where they sit. Descending is the same
                                    // chevron turned over, and it stands as far below the
                                    // circle's middle as the ascending one stands above it,
                                    // so the two are mirror images.
                                    Item {
                                        id: caret
                                        visible: filterButton.sorted.length > 0
                                        readonly property real span: Math.round(win.noteSize * 0.95)
                                        readonly property real arm: span / 2 / Math.cos(40 * Math.PI / 180)
                                        readonly property real thick: Math.max(2, Math.round(win.noteSize / 6))
                                        width: span
                                        height: Math.ceil(arm * Math.sin(40 * Math.PI / 180) + thick)
                                        x: dot.x + (dot.width - width) / 2
                                        y: dot.y + dot.height / 2 - height / 2
                                            + (filterButton.sorted === "asc" ? -1 : 1) * (win.noteSize * 0.5 + height / 2 - 1)
                                        rotation: filterButton.sorted === "asc" ? 0 : 180
                                        Rectangle {
                                            x: caret.span / 2 - caret.arm
                                            y: 0
                                            width: caret.arm
                                            height: caret.thick
                                            radius: caret.thick / 2
                                            color: win.accentColor
                                            transformOrigin: Item.Right
                                            rotation: -40
                                        }
                                        Rectangle {
                                            x: caret.span / 2
                                            y: 0
                                            width: caret.arm
                                            height: caret.thick
                                            radius: caret.thick / 2
                                            color: win.accentColor
                                            transformOrigin: Item.Left
                                            rotation: 40
                                        }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        anchors.margins: -4
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: columnPopup.openFor(headCell.index, headCell)
                                    }
                                }
                                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.lineColor }
                                // Columns copied or cut are marked from their headers down.
                                readonly property bool marked: win.outline !== null && win.outline.columns
                                    && index >= win.outline.left && index <= win.outline.right
                                MarkEdge { visible: headCell.marked; width: parent.width; height: 1 }
                                MarkEdge { visible: headCell.marked && headCell.index === win.outline.left && !win.outline.rows; width: 1; height: parent.height }
                                MarkEdge { visible: headCell.marked && headCell.index === win.outline.right; anchors.right: parent.right; width: 1; height: parent.height }
                                MouseArea {
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    onPressed: function(mouse) {
                                        if (mouse.button === Qt.RightButton) {
                                            win.openMenu(-1, headCell.index);
                                            return;
                                        }
                                        // Select the whole column.
                                        win.commitEditing();
                                        win.byRow = false;
                                        win.byCol = true;
                                        // The cursor waits in the first cell.
                                        win.anchorRow = Math.max(0, win.sheet.nrows - 1);
                                        win.anchorCol = (mouse.modifiers & Qt.ShiftModifier) ? win.anchorCol : headCell.index;
                                        win.curRow = 0;
                                        win.curCol = headCell.index;
                                    }
                                    preventStealing: true
                                    // Drag along the headers for several columns.
                                    onPositionChanged: function(mouse) {
                                        if (!(pressedButtons & Qt.LeftButton))
                                            return;
                                        var x = mapToItem(rowsView, mouse.x, mouse.y).x - win.gutterWidth;
                                        var c = 0;
                                        while (c < win.colWidths.length - 1 && x > win.colWidths[c]) {
                                            x -= win.colWidths[c];
                                            c++;
                                        }
                                        win.selectLine(false, c, true);
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

                    // The rows: one screen of them, not a list as tall as
                    // the sheet (open-a-big-csv D156). A list of millions
                    // of rows is hundreds of millions of pixels tall, and
                    // the scene graph places items in 32-bit floats, which
                    // past 2^24 px land rows and gridlines on steps of 2,
                    // 4, 8 px and more. Here the scroll is a number,
                    // `contentY`, and each row drawn sits within a screen
                    // of the top: the first is at place `first`, `offset`
                    // px up.
                    Item {
                        id: rowsView
                        width: hflick.contentWidth
                        height: hflick.height - win.headerHeight
                        clip: true
                        property real contentY: 0
                        readonly property real originY: 0
                        readonly property real contentHeight: win.shownCount * win.rowHeight
                        readonly property int first: Math.floor(contentY / win.rowHeight)
                        readonly property real offset: contentY - first * win.rowHeight
                        readonly property int slots: Math.ceil(height / win.rowHeight) + 1
                        // Always a number: NaN, before the sizes are known,
                        // would never equal itself, and set it again forever.
                        function clamp(y) {
                            var most = contentHeight - height;
                            if (!(most > 0))
                                most = 0;
                            if (!(y > 0))
                                return 0;
                            return Math.min(y, most);
                        }
                        function positionViewAtBeginning() {
                            contentY = 0;
                        }
                        // Never past either end, however the scroll was set.
                        onContentYChanged: if (contentY !== clamp(contentY)) contentY = clamp(contentY)
                        onContentHeightChanged: contentY = clamp(contentY)
                        onHeightChanged: contentY = clamp(contentY)

                        // The wheel scrolls; a wheel the cells do not take
                        // comes here. Sideways is the sheet's, left to it.
                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.NoButton
                            onWheel: function(wheel) {
                                var dx = wheel.pixelDelta.x !== 0 ? wheel.pixelDelta.x : wheel.angleDelta.x;
                                var dy = wheel.pixelDelta.y !== 0 ? wheel.pixelDelta.y
                                    : wheel.angleDelta.y / 120 * 3 * win.rowHeight;
                                if (Math.abs(dx) > Math.abs(dy)) {
                                    wheel.accepted = false;
                                    return;
                                }
                                rowsView.contentY = rowsView.clamp(rowsView.contentY - dy);
                            }

                        Repeater {
                        model: Math.max(0, Math.min(rowsView.slots, win.shownCount - rowsView.first))
                        delegate: Row {
                            id: rowItem
                            // Which of the rows drawn this is, from the top.
                            required property int index
                            // The place in the list it shows, and the row of
                            // the sheet at that place, which a filter makes
                            // other than the place.
                            readonly property int place: rowsView.first + index
                            readonly property int row: win.rowAt(place)
                            y: index * win.rowHeight - rowsView.offset
                            height: win.rowHeight
                            // The row holding a cell opened out to its lines
                            // is drawn above the others: a list paints its
                            // delegates in the order it made them, which is
                            // not the order of the rows.
                            z: win.openCell !== null && win.openCell.tab === win.tab
                                && win.openCell.row === rowItem.row ? 2 : 0

                            // The row number.
                            Rectangle {
                                width: win.gutterWidth
                                height: win.rowHeight
                                color: rowItem.row >= win.selTop && rowItem.row <= win.selBottom
                                    ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.16)
                                    : win.panelColor
                                Text {
                                    anchors.centerIn: parent
                                    text: rowItem.row
                                    color: win.mutedColor
                                    font.family: win.font.family
                                    font.features: win.font.features
                                    font.pixelSize: win.captionSize
                                }
                                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                // Rows copied or cut are marked from their numbers across.
                                readonly property bool marked: win.outline !== null && win.outline.rows
                                    && rowItem.row >= win.outline.top && rowItem.row <= win.outline.bottom
                                MarkEdge { visible: parent.marked; width: 1; height: parent.height }
                                MarkEdge { visible: parent.marked && rowItem.row === win.outline.top && !win.outline.columns; width: parent.width; height: 1 }
                                MarkEdge { visible: parent.marked && rowItem.row === win.outline.bottom; anchors.bottom: parent.bottom; width: parent.width; height: 1 }
                                MouseArea {
                                    anchors.fill: parent
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    onPressed: function(mouse) {
                                        if (mouse.button === Qt.RightButton) {
                                            win.openMenu(rowItem.row, -1);
                                            return;
                                        }
                                        // Select the whole row.
                                        win.commitEditing();
                                        win.byRow = true;
                                        win.byCol = false;
                                        // The cursor waits in the first cell.
                                        win.anchorCol = Math.max(0, win.sheet.columns.length - 1);
                                        win.anchorRow = (mouse.modifiers & Qt.ShiftModifier) ? win.anchorRow : rowItem.row;
                                        win.curCol = 0;
                                        win.curRow = rowItem.row;
                                    }
                                    preventStealing: true
                                    // Drag down the numbers for several rows.
                                    onPositionChanged: function(mouse) {
                                        if (!(pressedButtons & Qt.LeftButton))
                                            return;
                                        var p = mapToItem(rowsView, mouse.x, mouse.y);
                                        win.selectLine(true, win.rowAtY(p.y + rowsView.contentY), true);
                                    }
                                }
                            }

                            Repeater {
                                model: win.sheet.columns.length
                                Rectangle {
                                    id: cellItem
                                    required property int index
                                    readonly property int row: rowItem.row
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
                                    // Referred to by the formula being typed.
                                    readonly property bool inPick: win.refBoxes.length > 0 && win.inRefs(row, index)
                                    width: win.colWidths[index] || 0
                                    height: win.rowHeight
                                    // A cell opened out to its lines is drawn
                                    // over the cells beside it as well as the
                                    // rows below: the lines are longer than
                                    // the cell is wide, and a cell made after
                                    // it would otherwise be drawn over them.
                                    z: win.openCell !== null && win.openCell.tab === win.tab
                                        && win.openCell.row === row && win.openCell.col === index ? 1
                                        : (win.editing && current ? 1 : 0)
                                    color: (selected && !alone) || marked
                                        ? Qt.rgba(win.selectionFill.r, win.selectionFill.g, win.selectionFill.b, 0.55)
                                        : column && column.computed ? win.computedTint : "transparent"

                                    Text {
                                        id: cellText
                                        // Left, right and top, not `fill`:
                                        // filling drives the height from the
                                        // cell, and an opened cell sets its
                                        // own from the lines it holds.
                                        anchors.left: parent.left
                                        anchors.right: parent.right
                                        anchors.top: parent.top
                                        anchors.leftMargin: 7
                                        anchors.rightMargin: 7
                                        visible: !(win.editing && cellItem.current)
                                        // A cell is one line tall, so one
                                        // holding a newline shows its first
                                        // line until the caret opens it.
                                        readonly property bool lines: !!cellItem.cell
                                            && cellItem.cell.d.indexOf("\n") >= 0
                                        readonly property bool opened: lines && win.openCell !== null
                                            && win.openCell.tab === win.tab
                                            && win.openCell.row === cellItem.row
                                            && win.openCell.col === cellItem.index
                                        // Text written bare is Markdown; in quotes it is as it is.
                                        readonly property bool marked: !!cellItem.cell && cellItem.cell.h !== undefined
                                        text: marked ? cellItem.cell.h
                                            : !cellItem.cell ? ""
                                            : lines && !opened ? cellItem.cell.d.split("\n")[0]
                                            : cellItem.cell.d
                                        textFormat: marked ? Text.StyledText : Text.PlainText
                                        // Opened, it is drawn over the rows
                                        // below rather than growing the row.
                                        height: opened ? Math.max(parent.height, contentHeight + 6) : parent.height
                                        z: opened ? 30 : 0
                                        verticalAlignment: opened ? Text.AlignTop : Text.AlignVCenter
                                        linkColor: win.accentColor
                                        clip: marked
                                        color: cellItem.cell && cellItem.cell.e ? win.errorColor : win.textColor
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.fontSize
                                        horizontalAlignment: cellItem.cell && cellItem.cell.n ? Text.AlignRight : Text.AlignLeft
                                        // Opened, every line is shown whole;
                                        // closed, the one line is cut to fit.
                                        elide: opened ? Text.ElideNone : Text.ElideRight
                                    }

                                    // Behind an opened cell, so the rows it
                                    // is drawn over are set back and its
                                    // lines can be read.
                                    Rectangle {
                                        visible: cellText.opened
                                        anchors.left: parent.left
                                        anchors.top: parent.top
                                        // As wide as the longest line, which
                                        // runs past the cell's own edge: the
                                        // backing stops where the lines do,
                                        // not where the column does.
                                        width: Math.max(parent.width, cellText.contentWidth + 14)
                                        height: cellText.height
                                        z: 29
                                        color: win.overColor
                                        border.width: 1
                                        border.color: win.lineColor
                                    }

                                    // A cell holding more lines than it shows
                                    // says so, and the caret opens it.
                                    Text {
                                        visible: cellText.lines && !(win.editing && cellItem.current)
                                        anchors.right: parent.right
                                        anchors.rightMargin: 2
                                        anchors.verticalCenter: parent.verticalCenter
                                        z: 31
                                        text: cellText.opened ? "\u2303" : "\u2304"
                                        color: win.mutedColor
                                        font.family: win.font.family
                                        font.pixelSize: win.captionSize
                                        MouseArea {
                                            anchors.fill: parent
                                            anchors.margins: -4
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.openCellAt(cellItem.row, cellItem.index)
                                        }
                                    }

                                    Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.lineColor }
                                    // The outline of what was copied or cut.
                                    // Copied is outlined, cut is dashed.
                                    MarkEdge { visible: cellItem.marked && cellItem.row === win.outline.top && !win.outline.columns; width: parent.width; height: 1 }
                                    MarkEdge { visible: cellItem.marked && cellItem.row === win.outline.bottom; anchors.bottom: parent.bottom; width: parent.width; height: 1 }
                                    MarkEdge { visible: cellItem.marked && cellItem.index === win.outline.left && !win.outline.rows; width: 1; height: parent.height }
                                    MarkEdge { visible: cellItem.marked && cellItem.index === win.outline.right; anchors.right: parent.right; width: 1; height: parent.height }
                                    Rectangle {
                                        anchors.fill: parent
                                        visible: cellItem.alone
                                        color: "transparent"
                                        border.width: 2
                                        border.color: win.accentColor
                                    }
                                    Rectangle {
                                        anchors.fill: parent
                                        visible: cellItem.inPick
                                        color: Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.16)
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
                                        id: cellArea
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        // A drag selects or picks: the views do not take it to scroll.
                                        preventStealing: true
                                        property bool picking: false
                                        // Why the cell failed, or a number shown with fewer
                                        // digits than it has, in full; or what is cut off at
                                        // the cell's edge, except in a String column, where
                                        // text running past the edge is what is expected.
                                        // Said in the footer while the pointer is on it.
                                        readonly property string said: cellItem.cell
                                            ? cellItem.cell.e || cellItem.cell.x
                                                || (cellText.truncated && !(cellItem.column && cellItem.column.type === "String")
                                                    ? cellItem.cell.d : "")
                                            : ""
                                        readonly property bool failed: !!cellItem.cell && !!cellItem.cell.e
                                        function say() { win.hover(cellArea, containsMouse && !cellMenu.visible ? said : "", failed); }
                                        onContainsMouseChanged: say()
                                        onSaidChanged: say()
                                        Component.onDestruction: win.hover(cellArea, "", false)
                                        // A link in the cell under the pointer, opened with Ctrl and a click.
                                        readonly property string link: containsMouse
                                            ? cellText.linkAt(mouseX - cellText.x, mouseY - cellText.y) : ""
                                        cursorShape: link.length > 0 ? Qt.PointingHandCursor : Qt.ArrowCursor
                                        onPressed: function(mouse) {
                                            if (mouse.button === Qt.RightButton) {
                                                win.openMenu(cellItem.row, cellItem.index);
                                                return;
                                            }
                                            // While a formula is typed, a click picks this cell,
                                            // a drag a range, and Ctrl adds one more to a list.
                                            picking = win.canPick();
                                            if (picking) {
                                                win.pickPress(cellItem.row, cellItem.index,
                                                    (mouse.modifiers & Qt.AltModifier) !== 0,
                                                    (mouse.modifiers & Qt.ControlModifier) !== 0);
                                                return;
                                            }
                                            if (link.length > 0 && (mouse.modifiers & Qt.ControlModifier)) {
                                                win.openLink(link);
                                                return;
                                            }
                                            if (win.editing)
                                                win.commitEditing();
                                            grid.forceActiveFocus();
                                            win.selectCell(cellItem.row, cellItem.index, mouse.modifiers & Qt.ShiftModifier);
                                        }
                                        // A pick by the mouse is done when it is let go: the
                                        // arrows start afresh from the edited cell.
                                        onReleased: if (picking) win.stopPicking()
                                        onPositionChanged: function(mouse) {
                                            if (!(pressedButtons & Qt.LeftButton))
                                                return;
                                            // Drag to extend the selection, or the pick.
                                            var p = mapToItem(rowsView, mouse.x, mouse.y);
                                            var r = win.rowAtY(p.y + rowsView.contentY);
                                            var x = p.x - win.gutterWidth;
                                            var c = 0;
                                            while (c < win.colWidths.length - 1 && x > win.colWidths[c]) {
                                                x -= win.colWidths[c];
                                                c++;
                                            }
                                            if (picking)
                                                win.pickDrag(r, c);
                                            else
                                                win.selectCell(r, c, true);
                                        }
                                        onDoubleClicked: function(mouse) {
                                            if (mouse.button === Qt.LeftButton && !picking)
                                                win.startEditing(null);
                                        }
                                    }

                                    Loader {
                                        // Text too wide for the cell carries on
                                        // over the cells to the right, on an
                                        // opaque ground (the editor's own).
                                        anchors.left: parent.left
                                        anchors.top: parent.top
                                        anchors.bottom: parent.bottom
                                        width: item ? Math.max(parent.width, item.contentWidth + item.leftPadding + item.rightPadding + 4) : parent.width
                                        active: win.editing && cellItem.current
                                        // A `TextArea`, a cell being able to
                                        // hold a newline: the box at the top
                                        // echoes the text, so opening it shows
                                        // every line while one is typed here.
                                        sourceComponent: TextArea {
                                            font.family: win.font.family
                                            font.features: win.font.features
                                            font.pixelSize: win.fontSize
                                            color: win.textColor
                                            leftPadding: 7
                                            rightPadding: 7
                                            topPadding: 0
                                            bottomPadding: 0
                                            verticalAlignment: TextEdit.AlignVCenter
                                            wrapMode: TextEdit.NoWrap
                                            selectByMouse: true
                                            background: Rectangle {
                                                color: win.pageColor
                                                border.width: 2
                                                border.color: win.accentColor
                                            }
                                            property bool done: false
                                            // The arrow key last pressed with nowhere further to go:
                                            // pressed again it leaves the cell. 0 for none.
                                            property int nudged: 0
                                            // The box at the top echoes the text and where the cursor is in it.
                                            onTextChanged: if (!formulaField.activeFocus) {
                                                formulaField.text = text;
                                                formulaField.cursorPosition = cursorPosition;
                                            }
                                            onCursorPositionChanged: {
                                                nudged = 0;
                                                if (!formulaField.activeFocus)
                                                    formulaField.cursorPosition = cursorPosition;
                                            }
                                            Component.onDestruction: if (win.editor === this) win.editor = null
                                            Component.onCompleted: {
                                                win.editor = this;
                                                // Typing started the edit: replace. Otherwise edit the source.
                                                text = win.editSeed !== null && win.editSeed !== undefined && win.editSeed.length > 0
                                                    ? win.editSeed : (cellItem.cell ? cellItem.cell.s : "");
                                                forceActiveFocus();
                                                if (!(win.editSeed && win.editSeed.length > 0))
                                                    selectAll();
                                                else
                                                    cursorPosition = text.length;
                                            }
                                            function finish(dRow, dCol) {
                                                if (done)
                                                    return;
                                                done = true;
                                                win.commit(text, dRow, dCol);
                                            }
                                            Keys.onTabPressed: finish(0, 1)
                                            Keys.onBacktabPressed: finish(0, -1)
                                            // Escape takes back a pick under way, else the edit.
                                            Keys.onEscapePressed: {
                                                if (win.cancelPick())
                                                    return;
                                                done = true;
                                                win.stopEditing();
                                            }
                                            onActiveFocusChanged: if (!activeFocus) finish(0, 0)
                                            // In a formula, the arrows pick a cell's reference
                                            // where one may go, and with Alt anywhere, by row
                                            // number: Shift picks a range, Ctrl goes to the end.
                                            // Elsewhere they move in the text, and pressed twice
                                            // with nowhere further to go leave the cell for the grid.
                                            Keys.priority: Keys.BeforeItem
                                            Keys.onPressed: function(event) {
                                                // Ctrl+Enter puts in a row only from the grid, not mid-edit.
                                                if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                                                        && (event.modifiers & Qt.ControlModifier)) {
                                                    event.accepted = true;
                                                    return;
                                                }
                                                // Enter while the arrows are picking takes the reference
                                                // as it stands and goes on typing: the next one writes
                                                // the cell.
                                                if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                                                        && win.pickRow >= 0) {
                                                    event.accepted = true;
                                                    win.stopPicking();
                                                    return;
                                                }
                                                // Alt+Enter writes a newline into the cell, which the
                                                // file holds as `"A\nB"`; the box opens to show the
                                                // lines, the cell itself being one line tall. Plain
                                                // Enter writes the cell, as it always has.
                                                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                                                    event.accepted = true;
                                                    if (event.modifiers & Qt.AltModifier) {
                                                        insert(cursorPosition, "\n");
                                                        win.formulaOpen = true;
                                                    } else {
                                                        finish(1, 0);
                                                    }
                                                    return;
                                                }
                                                var shift = (event.modifiers & Qt.ShiftModifier) !== 0;
                                                var d = event.key === Qt.Key_Up ? [-1, 0]
                                                    : event.key === Qt.Key_Down ? [1, 0]
                                                    : event.key === Qt.Key_Left ? [0, -1]
                                                    : event.key === Qt.Key_Right ? [0, 1] : null;
                                                if (d) {
                                                    var picks = (event.modifiers & Qt.AltModifier) ? win.canPick() : win.arrowPicks();
                                                    if (picks && win.pickArrow(d[0], d[1], (event.modifiers & Qt.AltModifier) !== 0, shift,
                                                            (event.modifiers & Qt.ControlModifier) !== 0)) {
                                                        nudged = 0;
                                                        event.accepted = true;
                                                        return;
                                                    }
                                                    // Up and Down have nowhere to go in the cell's one
                                                    // line, and so write what is typed and select the
                                                    // cell above or below, as Enter and Tab do. Left at
                                                    // the start of the text or Right at its end have
                                                    // nowhere further: the first press stays where it
                                                    // is; the same key pressed again does the same.
                                                    // Held down, it does neither.
                                                    if ((event.modifiers & (Qt.ShiftModifier | Qt.ControlModifier | Qt.AltModifier)) === 0
                                                            && (d[0] !== 0 || (selectedText.length === 0
                                                                && cursorPosition === (d[1] < 0 ? 0 : text.length)))) {
                                                        event.accepted = true;
                                                        if (!event.isAutoRepeat) {
                                                            if (nudged === event.key || (d[0] !== 0 && text.indexOf("\n") < 0))
                                                                finish(d[0], d[1]);
                                                            else
                                                                nudged = event.key;
                                                        }
                                                        return;
                                                    }
                                                    nudged = 0;
                                                    event.accepted = false;
                                                    return;
                                                }
                                                // A modifier on its own leaves a pick going.
                                                if ([Qt.Key_Shift, Qt.Key_Alt, Qt.Key_Control, Qt.Key_Meta].indexOf(event.key) < 0) {
                                                    win.stopPicking();
                                                    nudged = 0;
                                                }
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
            }

            // A click beside the columns or below the rows deselects.
            MouseArea {
                id: beside
                visible: hflick.visible
                x: Math.min(hflick.width, win.gutterWidth + win.totalWidth - hflick.contentX)
                width: hflick.width - x
                height: hflick.height
                onPressed: {
                    win.commitEditing();
                    win.deselect();
                }
            }
            MouseArea {
                visible: hflick.visible
                y: Math.min(hflick.height, win.headerHeight + rowsView.contentHeight - rowsView.contentY)
                width: beside.x
                height: hflick.height - y
                onPressed: {
                    win.commitEditing();
                    win.deselect();
                }
            }

            ScrollBar {
                id: vbar
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.topMargin: win.headerHeight
                anchors.bottom: parent.bottom
                anchors.bottomMargin: hbar.needed ? hbar.height : 0
                policy: ScrollBar.AsNeeded
                // The rows' own scroll, which no Flickable keeps (D156). The
                // handle stays big enough to take hold of on millions of rows.
                minimumSize: 0.04
                size: rowsView.contentHeight > 0 ? Math.min(1, rowsView.height / rowsView.contentHeight) : 1
                Binding on position {
                    when: !vbar.pressed
                    value: rowsView.contentHeight > 0 ? rowsView.contentY / rowsView.contentHeight : 0
                    restoreMode: Binding.RestoreNone
                }
                onPositionChanged: if (pressed) rowsView.contentY = rowsView.clamp(position * rowsView.contentHeight)
            }

            ScrollBar {
                id: hbar
                // The bar takes no room until the sheet is wider than the view.
                readonly property bool needed: size < 1
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

                FooterButton {
                    label: "\u2039"
                    quiet: true
                    visible: tabStrip.scrolls
                    enabled: tabStrip.contentX > 0
                    onClicked: tabStrip.step(-1)
                }

                // The tabs, which scroll when there are more of them than
                // the bar has room for: the one shown is kept in view.
                Flickable {
                    id: tabStrip
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredWidth: tabRow.implicitWidth
                    Layout.maximumWidth: tabRow.implicitWidth
                    contentWidth: tabRow.implicitWidth
                    clip: true
                    flickableDirection: Flickable.HorizontalFlick
                    boundsBehavior: Flickable.StopAtBounds
                    readonly property bool scrolls: contentWidth > width + 1
                    readonly property real furthest: Math.max(0, contentWidth - width)
                    // An arrow moves it by most of what is in view.
                    function step(by) {
                        contentX = Math.max(0, Math.min(furthest, contentX + by * Math.max(40, width * 0.8)));
                    }
                    // Enough of the tab shown to see it all, and no more.
                    function reveal() {
                        var item = tabRepeater.itemAt(win.tab);
                        if (!item)
                            return;
                        var x = item.x < contentX ? item.x
                            : item.x + item.width > contentX + width ? item.x + item.width - width
                            : contentX;
                        contentX = Math.max(0, Math.min(furthest, x));
                    }
                    readonly property int shown: win.tab
                    onShownChanged: Qt.callLater(reveal)
                    onWidthChanged: Qt.callLater(reveal)
                    // A sheet added, taken away or renamed changes how much
                    // room the tabs take.
                    onContentWidthChanged: Qt.callLater(reveal)
                    Component.onCompleted: Qt.callLater(reveal)

                    RowLayout {
                        id: tabRow
                        height: tabStrip.height
                        width: implicitWidth
                        spacing: 2
                        // The tab being dragged, or -1, and the gap it is over:
                        // 0 is before the first sheet, and the number of sheets
                        // is after the last.
                        property int dragFrom: -1
                        property int gap: -1
                        readonly property int sheets: {
                            var n = 0;
                            while (n < win.tabs.length && !win.tabs[n].isConsts)
                                n++;
                            return n;
                        }
                        // The gap for a pointer at `x` along the row: before the
                        // first tab whose middle it has not passed. The constants
                        // stay last, so it never goes past the sheets.
                        function gapAt(x) {
                            for (var i = 0; i < sheets; i++) {
                                var item = tabRepeater.itemAt(i);
                                if (item && x < item.x + item.width / 2)
                                    return i;
                            }
                            return sheets;
                        }
                        Repeater {
                            id: tabRepeater
                            model: win.tabs.length
                            FooterButton {
                                id: tabButton
                                required property int index
                                // A table's rows are in a Parquet file, not in this
                                // one: its tab is marked, and says which file.
                                readonly property string from: win.tabs[index].from || ""
                                label: (from ? "\u25A4 " : "") + win.tabs[index].name
                                tip: from ? from + ", " + win.tabs[index].nrows
                                    + (win.tabs[index].nrows === 1 ? " row" : " rows") + ", read-only" : ""
                                active: index === win.tab && !win.showSource
                                // The whole sheet copied or cut, outlined here too.
                                readonly property bool marked: win.sheetMark !== null && win.sheetMark.tab === index
                                MarkEdge { visible: tabButton.marked; dashed: win.sheetMark !== null && win.sheetMark.cut; width: parent.width; height: 1 }
                                MarkEdge { visible: tabButton.marked; dashed: win.sheetMark !== null && win.sheetMark.cut; anchors.bottom: parent.bottom; width: parent.width; height: 1 }
                                MarkEdge { visible: tabButton.marked; dashed: win.sheetMark !== null && win.sheetMark.cut; width: 1; height: parent.height }
                                MarkEdge { visible: tabButton.marked; dashed: win.sheetMark !== null && win.sheetMark.cut; anchors.right: parent.right; width: 1; height: parent.height }
                                onClicked: {
                                    win.viewSource(false);
                                    win.switchTab(index);
                                }
                                // Dragged along the row to another place among the
                                // sheets; a click that does not move is still a click.
                                opacity: dragger.active ? 0.5 : 1
                                // Where the tab being dragged would land: a line in
                                // the gap nearest the pointer, which is the left of a
                                // tab, or the right of the last sheet. Drawn by the tab
                                // beside the gap, so it stays with it.
                                readonly property bool landing: tabRow.dragFrom >= 0
                                    && tabRow.gap !== tabRow.dragFrom && tabRow.gap !== tabRow.dragFrom + 1
                                Rectangle {
                                    visible: tabButton.landing && tabRow.gap === tabButton.index
                                    x: tabButton.index === 0 ? 0 : -2
                                    y: 3
                                    width: 2
                                    height: parent.height - 6
                                    radius: 1
                                    color: win.accentColor
                                    z: 2
                                }
                                Rectangle {
                                    visible: tabButton.landing && tabRow.gap === tabRow.sheets
                                        && tabButton.index === tabRow.sheets - 1
                                    x: parent.width - 2
                                    y: 3
                                    width: 2
                                    height: parent.height - 6
                                    radius: 1
                                    color: win.accentColor
                                    z: 2
                                }
                                DragHandler {
                                    id: dragger
                                    target: null
                                    acceptedButtons: Qt.LeftButton
                                    yAxis.enabled: false
                                    enabled: !win.tabs[tabButton.index].isConsts && win.renamingTab !== tabButton.index
                                    onActiveChanged: {
                                        if (active) {
                                            tabRow.dragFrom = tabButton.index;
                                            tabRow.gap = tabButton.index;
                                        } else {
                                            // Taking the tab out leaves the gaps after
                                            // it one place nearer the front.
                                            var from = tabRow.dragFrom, gap = tabRow.gap;
                                            tabRow.dragFrom = -1;
                                            tabRow.gap = -1;
                                            win.dropTab(from, gap > from ? gap - 1 : gap);
                                        }
                                    }
                                    onCentroidChanged: {
                                        if (active)
                                            tabRow.gap = tabRow.gapAt(tabButton.mapToItem(tabRow, centroid.position.x, 0).x);
                                    }
                                }
                                // Double-clicked, a sheet's name is typed over in place,
                                // as from F2. The first click has made it the sheet
                                // shown; the constants have no name to change.
                                onDoubleClicked: {
                                    if (win.tabs[index].isConsts)
                                        return;
                                    win.viewSource(false);
                                    win.switchTab(index);
                                    // After the click's own focus change, which would
                                    // otherwise take the keyboard back from the box.
                                    Qt.callLater(function() { win.renamingTab = index; });
                                }
                                // The tab's own menu: the sheet shown stays as it is,
                                // so that this one can be opened in a window beside it.
                                onRightClicked: {
                                    tabMenu.index = index;
                                    tabMenu.popup();
                                }
                                // Renamed in place, from F2 with the whole sheet selected.
                                Loader {
                                    anchors.verticalCenter: parent.verticalCenter
                                    height: parent.height
                                    active: win.renamingTab === tabButton.index
                                    sourceComponent: TextField {
                                        font.family: win.font.family
                                        font.features: win.font.features
                                        font.pixelSize: win.smallSize
                                        font.bold: true
                                        color: win.textColor
                                        width: Math.max(tabButton.width, contentWidth + leftPadding + rightPadding + 4)
                                        leftPadding: 10
                                        rightPadding: 10
                                        topPadding: 0
                                        bottomPadding: 0
                                        verticalAlignment: TextInput.AlignVCenter
                                        selectByMouse: true
                                        background: Rectangle {
                                            color: win.pageColor
                                            radius: 4
                                            border.width: 2
                                            border.color: win.accentColor
                                        }
                                        property bool done: false
                                        Component.onCompleted: {
                                            text = win.tabs[tabButton.index].name;
                                            forceActiveFocus();
                                            selectAll();
                                        }
                                        // A name that is refused is said why, and the old one stays.
                                        function finish(keep) {
                                            if (done)
                                                return;
                                            done = true;
                                            var name = text.trim();
                                            var old = win.tabs[tabButton.index].name;
                                            if (keep && name.length > 0 && name !== old)
                                                win.keeping(0, 0, function() { win.notice = file.renameSheet(win.tab, name); });
                                            win.renamingTab = -1;
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
                }
                FooterButton {
                    label: "\u203a"
                    quiet: true
                    visible: tabStrip.scrolls
                    enabled: tabStrip.contentX < tabStrip.furthest
                    onClicked: tabStrip.step(1)
                }
                FooterButton { label: "+ Sheet"; quiet: true; onClicked: promptDialog.ask("sheet") }

                Item { Layout.fillWidth: true }

                FooterButton { label: "+ Row"; quiet: true; enabled: !win.sheet.isConsts && win.tabs.length > 0; onClicked: win.appendRow() }
                FooterButton { label: "+ Column"; quiet: true; enabled: !win.sheet.isConsts && win.tabs.length > 0; onClicked: promptDialog.ask("column") }
                FooterButton { label: "+ Constant"; quiet: true; onClicked: promptDialog.ask("const") }
                FooterButton { label: "fn()"; quiet: true; onClicked: functionsDialog.open() }
                FooterButton { label: "</>"; quiet: true; active: win.showSource; onClicked: win.viewSource(!win.showSource) }
                FooterButton { label: "?"; quiet: true; onClicked: helpDialog.open() }
            }
        }

        // Below it, a bar of its own for everything the app has to say.
        // The bar above is for what the user presses; this one is for what
        // it answers. In the middle, the message: why a move was refused,
        // how to put down what is being moved, that a formula column is
        // being changed, or what the app has just done or could not do. On
        // the right, what is counted rather than said, and the notes to
        // read. Most of the time there is nothing to say and it is empty.
        Rectangle {
            id: messageBar
            Layout.fillWidth: true
            Layout.preferredHeight: win.fontSize + 10
            color: win.panelColor

            Rectangle { width: parent.width; height: 1; color: win.lineColor }

            // A message that has been said (a refusal, what was just done)
            // fades after a few seconds; resting the mouse on the bar
            // brings it back, and holds it while there. States that are
            // still true (working, moving, a formula column) do not fade.
            HoverHandler {
                id: barHover
                onHoveredChanged: {
                    barMessage.faded = false;
                    if (!hovered)
                        fadeTimer.restart();
                }
            }
            Timer {
                id: fadeTimer
                interval: 5000
                running: barMessage.passing && !barHover.hovered
                onTriggered: barMessage.faded = barMessage.passing && !barHover.hovered
            }

            // What the app has to say, centred in the bar whatever else
            // sits beside it, so it reads as the answer to what was done.
            Text {
                id: barMessage
                anchors.centerIn: parent
                width: Math.max(0, Math.min(implicitWidth,
                    parent.width - 2 * barRight.width - 48))
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                readonly property bool moving: win.grabbedHere
                // Said once, as against a state that holds.
                readonly property bool passing: win.busyText.length === 0 && !win.filterPending
                    && !moving && !win.formulaCell
                    && (win.notice.length > 0 || file.status.length > 0)
                property bool faded: false
                opacity: faded && win.hint.length === 0 ? 0 : 1
                Behavior on opacity { NumberAnimation { duration: 700 } }
                onTextChanged: {
                    faded = false;
                    fadeTimer.restart();
                }
                onPassingChanged: faded = false
                text: win.busyText.length > 0 ? win.busyText
                    : win.filterPending ? "Filtering\u2026"
                    : win.hint.length > 0 ? win.hint
                    : win.notice.length > 0 ? win.notice
                    : moving ? "Select a " + (win.grabbed.rows ? "row" : "column") + " and paste to move here (Esc cancels)"
                    : win.formulaCell ? "This is a formula column, this change will apply to the entire column"
                    : file.status.length > 0 ? file.status
                    // With nothing more pressing to say, a calculated sheet
                    // says what it is calculated from.
                    // A formula written over several lines is said in one
                    // here, the bar being one line tall.
                    : win.sheet.view ? win.sheet.name + " := " + win.sheet.view.replace(/\s+/g, " ")
                    : ""
                color: win.hint.length > 0 ? (win.hintBad ? win.errorColor : win.mutedColor)
                    : win.notice.length > 0 ? win.errorColor : win.mutedColor
                elide: Text.ElideRight
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.captionSize
            }

            // Turning while something takes a while: work in hand, or a
            // file being read in the background.
            BusyIndicator {
                anchors.right: barMessage.left
                anchors.rightMargin: 6
                anchors.verticalCenter: parent.verticalCenter
                height: parent.height - 6
                width: height
                padding: 0
                running: win.busyText.length > 0 || file.working || win.filterPending
                visible: running
                Material.accent: win.accentColor
            }

            // On the right, what is counted rather than said: what a
            // selection of several cells adds up to, and the notes to read.
            RowLayout {
                id: barRight
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                spacing: 12

                Text {
                    readonly property bool several: !win.sheet.isConsts && win.tabs.length > 0
                        && (win.selTop !== win.selBottom || win.selLeft !== win.selRight)
                    visible: several && !!win.snap
                    Layout.fillHeight: true
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: win.selSummary
                    color: win.mutedColor
                    elide: Text.ElideRight
                    font.family: win.font.family
                    font.features: win.font.features
                    font.pixelSize: win.captionSize
                }

                FooterButton {
                    readonly property int count: file.notes.length > 0 ? file.notes.split("\n").length : 0
                    visible: count > 0
                    // The bar is one line tall, where the bar above is two.
                    implicitHeight: win.fontSize + 6
                    label: count + (count === 1 ? " note" : " notes")
                    onClicked: notesDialog.open()
                }

                FooterButton {
                    readonly property int count: win.snap.problems.length
                    visible: count > 0
                    implicitHeight: win.fontSize + 6
                    label: count + (count === 1 ? " problem" : " problems")
                    textColor: win.errorColor
                    onClicked: problemsDialog.open()
                }
            }
        }
    }

    // The entries of the right-click menu, close set.
    // One side of the cells copied or cut: a line, or dashes when cut.
    component MarkEdge: Item {
        property bool dashed: win.outline !== null && win.outline.cut
        readonly property bool across: width >= height
        readonly property real length: across ? width : height
        Rectangle {
            visible: !parent.dashed
            anchors.fill: parent
            color: win.accentColor
        }
        Repeater {
            model: parent.dashed ? Math.ceil(parent.length / 8) : 0
            Rectangle {
                required property int index
                x: parent.across ? index * 8 : 0
                y: parent.across ? 0 : index * 8
                width: parent.across ? Math.min(4, parent.length - x) : parent.width
                height: parent.across ? parent.height : Math.min(4, parent.length - y)
                color: win.accentColor
            }
        }
    }

    // A small button of the column popup, which looks like one whether the
    // mouse is over it or not: filled with the accent when it is the one
    // Enter would mean, outlined otherwise.
    component PopupButton: Rectangle {
        id: popupButton
        property string text: ""
        property bool primary: false
        signal clicked()
        implicitWidth: Math.max(label.implicitWidth + 16, Math.round(win.captionSize * 4.2))
        implicitHeight: Math.round(win.captionSize * 2)
        radius: 3
        color: primary
            ? (press.containsMouse ? Qt.lighter(win.accentColor, 1.15) : win.accentColor)
            : (press.containsMouse ? win.lineColor : "transparent")
        border.width: primary ? 0 : 1
        border.color: win.lineColor
        Text {
            id: label
            anchors.centerIn: parent
            text: popupButton.text
            color: popupButton.primary ? win.pageColor : win.textColor
            font.family: win.font.family
            font.pixelSize: win.captionSize
            font.bold: popupButton.primary
        }
        MouseArea {
            id: press
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: popupButton.clicked()
        }
    }
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
        font.pixelSize: win.smallSize
    }
    // A type to declare the selected columns as: ticked if they are.
    component TypeEntry: MenuEntry {
        required property string typeName
        /// A narrower kind of the type above it, shown indented under it.
        property bool narrower: false
        text: (typeMenu.current === typeName ? "\u2713 " : "  ")
              + (narrower ? "   " : "") + typeName
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
        // Said in the footer while the pointer is on it, where there is anything to say.
        property string tip: ""
        signal clicked()
        signal rightClicked()
        signal doubleClicked()
        implicitWidth: buttonText.implicitWidth + 22
        implicitHeight: win.fontSize * 2
        // In the footer's row these keep their width: the tabs, which can
        // scroll, are what gives way when there is not room for everything.
        Layout.minimumWidth: implicitWidth
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
            font.pixelSize: win.smallSize
            font.bold: button.active
        }
        MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onContainsMouseChanged: win.hover(button, containsMouse ? button.tip : "", false)
            Component.onDestruction: win.hover(button, "", false)
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton) {
                    button.rightClicked();
                    return;
                }
                button.clicked();
                grid.forceActiveFocus();
            }
            onDoubleClicked: function(mouse) {
                if (mouse.button === Qt.LeftButton)
                    button.doubleClicked();
            }
        }
    }

    // ---- context menu ------------------------------------------------------


    // A column's sort and filter. Sorting a sheet of its own rows rearranges
    // them in the file at once, as one step to undo; a calculated sheet is
    // sorted by its expression, which comes to end in `|> sort(...)`; and a
    // table, whose rows are its file's, is sorted in the grid alone. The
    // filter is an expression in which `*` is the cell (filter-by-expression
    // D160, D171): checked as it is typed, with no row read. Apply applies
    // it and stays open, to try another; OK or Enter applies it and closes.
    // Esc, or a click elsewhere, leaves what was last applied.
    Popup {
        id: columnPopup
        property int col: 0
        // What is wrong with the expression typed, or "".
        property string fault: ""
        readonly property bool sortable: !win.sheet.isConsts
        padding: 8
        width: Math.round(win.fontSize * 16.5)
        // Smaller than the grid: the controls of Material are made for touch.
        readonly property int textSize: win.captionSize
        readonly property int lineHeight: Math.round(textSize * 1.9)
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        onClosed: grid.forceActiveFocus()

        function openFor(c, item) {
            col = c;
            parent = item;
            x = 0;
            y = item.height;
            applied = win.filterOf(c);
            filterField.text = applied;
            check();
            open();
            filterField.forceActiveFocus();
            filterField.selectAll();
        }
        function check() {
            fault = file.checkFilter(win.tab, col, filterField.text);
        }
        function sortBy(desc) {
            var c = col;
            close();
            win.busy("Sorting\u2026", function() {
                win.sortMark = file.undoMark();
                var problem = file.sortRows(win.tab, c, desc);
                // "…": a big table, sorted on a worker; `sorted` says when.
                if (problem === "\u2026")
                    return;
                if (problem.length > 0)
                    win.notice = problem;
                else
                    win.noteSort(c, desc);
            });
        }
        // Whether the expression typed passes and is not the one in force:
        // what Apply would change.
        readonly property bool changed: fault.length === 0
            && filterField.text.trim() !== applied
        // What this popup last applied: the filter in force, without waiting
        // for it to be set, which happens a frame later behind the spinner.
        property string applied: ""
        // Apply the expression and stay open, to try another.
        function apply() {
            check();
            if (fault.length > 0)
                return false;
            if (changed) {
                applied = filterField.text.trim();
                win.setFilter(col, filterField.text);
            }
            return true;
        }
        // Apply it and close: OK, and Enter.
        function accept() {
            if (apply())
                close();
        }

        background: Rectangle {
            color: win.overColor
            border.color: win.lineColor
            radius: 3
        }
        contentItem: ColumnLayout {
            spacing: 4
            MenuEntry {
                visible: columnPopup.sortable
                Layout.fillWidth: true
                text: "Sort Ascending"
                implicitHeight: columnPopup.lineHeight
                font.pixelSize: columnPopup.textSize
                onClicked: columnPopup.sortBy(false)
            }
            MenuEntry {
                visible: columnPopup.sortable
                Layout.fillWidth: true
                text: "Sort Descending"
                implicitHeight: columnPopup.lineHeight
                font.pixelSize: columnPopup.textSize
                onClicked: columnPopup.sortBy(true)
            }
            Rectangle {
                visible: columnPopup.sortable
                Layout.fillWidth: true
                height: 1
                color: win.lineColor
            }
            Text {
                Layout.fillWidth: true
                Layout.maximumWidth: columnPopup.availableWidth
                Layout.topMargin: 4
                text: "Filter"
                color: win.mutedColor
                font.family: win.font.family
                font.pixelSize: columnPopup.textSize
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.maximumWidth: columnPopup.availableWidth
                implicitHeight: columnPopup.lineHeight + 4
                color: "transparent"
                radius: 2
                border.color: columnPopup.fault.length > 0 ? win.errorColor
                    : filterField.activeFocus ? win.accentColor : win.lineColor
                // What is wrong, for a pointer held over the red box.
                HoverHandler { id: filterHover }
                ToolTip.visible: filterHover.hovered && columnPopup.fault.length > 0
                ToolTip.text: columnPopup.fault
                ToolTip.delay: 300
                TextInput {
                    id: filterField
                    anchors.fill: parent
                    anchors.leftMargin: 6
                    anchors.rightMargin: 6
                    verticalAlignment: TextInput.AlignVCenter
                    clip: true
                    color: win.textColor
                    selectionColor: win.selectionFill
                    selectByMouse: true
                    font.family: win.monoFamily
                    font.pixelSize: columnPopup.textSize
                    onTextChanged: columnPopup.check()
                    Keys.onReturnPressed: columnPopup.accept()
                    Keys.onEnterPressed: columnPopup.accept()
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: filterField.text.length === 0
                        text: "e.g. * >= 3"
                        color: win.mutedColor
                        font: filterField.font
                    }
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.topMargin: 4
                height: 1
                color: win.lineColor
            }
            MenuEntry {
                Layout.fillWidth: true
                text: "Clear filter"
                implicitHeight: columnPopup.lineHeight
                font.pixelSize: columnPopup.textSize
                enabled: columnPopup.applied.length > 0
                // The filter goes, and the popup stays, to write another.
                onClicked: {
                    columnPopup.applied = "";
                    filterField.text = "";
                    win.setFilter(columnPopup.col, "");
                    filterField.forceActiveFocus();
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.maximumWidth: columnPopup.availableWidth
                Layout.topMargin: 4
                spacing: 6
                Item { Layout.fillWidth: true }
                // Lit while there is a passing expression to apply; OK is
                // the filled one otherwise.
                PopupButton {
                    text: "Apply"
                    primary: columnPopup.changed
                    enabled: columnPopup.changed
                    opacity: enabled ? 1 : 0.4
                    onClicked: columnPopup.apply()
                }
                PopupButton {
                    text: "OK"
                    primary: !columnPopup.changed
                    enabled: columnPopup.fault.length === 0
                    opacity: enabled ? 1 : 0.4
                    onClicked: columnPopup.accept()
                }
                PopupButton {
                    text: "Cancel"
                    onClicked: columnPopup.close()
                }
            }
        }
    }

    // What was filtered belongs to the file it was filtered in.
    Connections {
        target: file
        function onFilePathChanged() {
            win.filters = ({});
            win.goodFilters = ({});
            win.filterRev++;
            win.sortLog = [];
            win.undoDepth = file.undoMark();
            win.editCount = file.editCount();
            win.sortRev++;
        }
    }

    Menu {
        id: cellMenu
        readonly property bool editable: !win.sheet.isConsts
        readonly property bool hasRows: editable && win.sheet.nrows > 0
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
            else if (!typeMenu.visible && !columnPopup.visible)
                grid.forceActiveFocus();
        }
        // Close set, to fit a long menu on a small window.
        topPadding: 4
        bottomPadding: 4
        width: Math.round(win.fontSize * 15)

        MenuEntry {
            text: "Cut"
            visible: !win.sheet.isConsts
            enabled: (cellMenu.hasRows || win.grabKind.length > 0) && !win.formulaCell
            onTriggered: win.cutSelection()
        }
        MenuEntry { text: "Copy"; visible: !win.sheet.isConsts; enabled: win.sheet.nrows > 0; onTriggered: win.copySelection() }
        MenuEntry {
            text: "Paste"
            visible: !win.sheet.isConsts
            // Only while something copied or cut is waiting to go down.
            enabled: ((win.sheet.nrows > 0 && win.canPaste) || win.grabbedHere) && win.pasteFits
            onTriggered: win.pasteSelection()
        }
        MenuEntry {
            // One cell copied is one value.
            text: win.marked !== null && win.marked.top === win.marked.bottom
                && win.marked.left === win.marked.right ? "Paste value" : "Paste values"
            visible: !win.sheet.isConsts
            enabled: cellMenu.hasRows && win.canPasteValues
            onTriggered: win.pasteValues()
        }
        MenuEntry { text: "Paste references"; visible: !win.sheet.isConsts; enabled: cellMenu.hasRows && win.canPasteReference; onTriggered: win.pasteReference() }
        MenuEntry { text: "Paste transposed"; visible: !win.sheet.isConsts; enabled: cellMenu.hasRows && win.canTranspose; onTriggered: win.pasteTransposed() }
        // A calculated column is its formula: there is nothing in it to clear.
        MenuEntry {
            text: "Clear"
            visible: !win.sheet.isConsts
            enabled: cellMenu.hasRows && !win.formulaCell && !(cellMenu.columns && win.anyComputed())
            onTriggered: win.clearSelection()
        }
        MenuEntry {
            text: "Delete constant"
            visible: win.sheet.isConsts
            enabled: visible && win.sheet.nrows > 0
            onTriggered: win.deleteConstant()
        }
        // Delete, for what is selected as a whole.
        MenuRule { visible: cellMenu.rows }
        MenuEntry {
            text: cellMenu.rowCount > 1 ? "Delete " + cellMenu.rowCount + " rows" : "Delete row"
            visible: cellMenu.rows
            enabled: visible && cellMenu.hasRows
            onTriggered: win.deleteRows()
        }
        MenuRule { visible: cellMenu.columns }
        MenuEntry {
            text: cellMenu.columnCount > 1 ? "Delete " + cellMenu.columnCount + " columns" : "Delete column"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.sheet.columns.length > 1
            onTriggered: win.deleteColumns()
        }
        MenuEntry {
            text: "Delete sheet"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: win.deleteSheet()
        }
        // The sheet, when the whole of it is selected.
        MenuRule { visible: cellMenu.whole }
        MenuEntry {
            text: "Insert sheet"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: win.newSheet(true)
        }
        MenuEntry {
            text: "Rename sheet"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: promptDialog.ask("renameSheet")
        }
        MenuEntry {
            text: "Open in new window"
            visible: cellMenu.whole
            enabled: visible
            onTriggered: win.newWindow(win.tab)
        }
        // Rows, when whole rows are selected.
        MenuEntry {
            text: win.rowsOnHand !== null ? "Insert cells above"
                : cellMenu.rowCount > 1 ? "Insert " + cellMenu.rowCount + " rows above" : "Insert row above"
            visible: cellMenu.rows
            enabled: visible && cellMenu.editable
            onTriggered: win.insertRowsHere(false, cellMenu.rowCount)
        }
        MenuEntry {
            text: win.rowsOnHand !== null ? "Insert cells below"
                : cellMenu.rowCount > 1 ? "Insert " + cellMenu.rowCount + " rows below" : "Insert row below"
            visible: cellMenu.rows
            enabled: visible && cellMenu.editable
            onTriggered: win.insertRowsHere(true, cellMenu.rowCount)
        }
        MenuEntry {
            text: "Insert rows…"
            visible: cellMenu.rows
            enabled: visible && cellMenu.editable
            onTriggered: promptDialog.ask("rows")
        }
        // Columns, when whole columns are selected. A column added
        // from the footer goes at the end.
        MenuEntry {
            text: win.colsOnHand !== null ? "Insert cells before"
                : cellMenu.columnCount > 1 ? "Insert " + cellMenu.columnCount + " columns before" : "Insert column before"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable
            onTriggered: win.insertColumnsHere(false)
        }
        MenuEntry {
            text: win.colsOnHand !== null ? "Insert cells after"
                : cellMenu.columnCount > 1 ? "Insert " + cellMenu.columnCount + " columns after" : "Insert column after"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable
            onTriggered: win.insertColumnsHere(true)
        }
        // A sheet still in letters: a number of columns, named on from the
        // last, at the end so that the letters stay in order.
        MenuEntry {
            text: "Insert columns\u2026"
            visible: cellMenu.columns && win.letterSheet
            enabled: visible && cellMenu.editable && win.sheet.columns.length < win.maxLetterColumns
            onTriggered: promptDialog.ask("columns")
        }
        MenuRule { visible: cellMenu.columns }
        MenuEntry {
            text: "Rename column"
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
            text: "Set format\u2026"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.sheet.columns.length > 0 && win.allFormattable()
            onTriggered: promptDialog.ask("format")
        }
        // The filter box from the column's header, for the one column.
        MenuEntry {
            text: "Set filter\u2026"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.selLeft === win.selRight
                && win.sheet.columns.length > 0
            onTriggered: {
                var head = headCells.itemAt(win.selLeft);
                if (head)
                    columnPopup.openFor(win.selLeft, head);
            }
        }
        MenuEntry {
            text: "Clear type"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.anyDeclared()
            onTriggered: win.setColumnTypes("")
        }
        MenuEntry {
            text: "Clear format"
            visible: cellMenu.columns
            enabled: visible && cellMenu.editable && win.anyFormatted()
            onTriggered: win.clearFormats()
        }
        MenuEntry {
            text: "Clear filter"
            visible: cellMenu.columns
            enabled: visible && win.anyFiltered()
            onTriggered: win.clearFilters()
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

        TypeEntry { typeName: "Integer" }
        TypeEntry { typeName: "Binary"; narrower: true }
        TypeEntry { typeName: "Octal"; narrower: true }
        TypeEntry { typeName: "Hex"; narrower: true }
        TypeEntry { typeName: "Decimal" }
        TypeEntry { typeName: "Fraction"; narrower: true }
        TypeEntry { typeName: "Percent"; narrower: true }
        TypeEntry { typeName: "Float" }
        TypeEntry { typeName: "Complex" }
        TypeEntry { typeName: "String" }
        TypeEntry { typeName: "Email"; narrower: true }
        TypeEntry { typeName: "Url"; narrower: true }
        TypeEntry { typeName: "Uuid"; narrower: true }
        TypeEntry { typeName: "Boolean" }
        TypeEntry { typeName: "Datetime" }
        TypeEntry { typeName: "Date"; narrower: true }
        TypeEntry { typeName: "Duration"; narrower: true }
        TypeEntry { typeName: "Time"; narrower: true }
    }

    // Right-click on a sheet's tab.
    Menu {
        id: tabMenu
        // The tab this menu was opened on, which is not the tab shown unless
        // it already was.
        property int index: 0
        // The constants are a sheet of their own, which is not edited here.
        readonly property bool isSheet: index < win.tabs.length && !win.tabs[index].isConsts
        // An entry may have opened a dialog, which keeps the keyboard.
        onClosed: {
            if (promptDialog.visible)
                promptDialog.takeFocus();
            else
                grid.forceActiveFocus();
        }
        topPadding: 4
        bottomPadding: 4
        width: Math.round(win.fontSize * 15)

        // Each of these shows the sheet first, with the whole of it
        // selected, as the same entry does from the grid.
        MenuEntry {
            text: "Insert sheet"
            visible: tabMenu.isSheet
            enabled: visible
            onTriggered: { win.goToSheet(tabMenu.index); win.newSheet(true); }
        }
        MenuEntry {
            text: "Duplicate sheet"
            visible: tabMenu.isSheet
            enabled: visible
            onTriggered: {
                win.goToSheet(tabMenu.index);
                var problem = file.duplicateSheet(tabMenu.index);
                if (problem.length > 0)
                    errorDialog.say(problem);
                else
                    win.switchTab(tabMenu.index + 1);
            }
        }
        MenuEntry {
            text: "Rename sheet"
            visible: tabMenu.isSheet
            enabled: visible
            onTriggered: { win.goToSheet(tabMenu.index); promptDialog.ask("renameSheet"); }
        }
        MenuEntry {
            text: "Delete sheet"
            visible: tabMenu.isSheet
            enabled: visible
            onTriggered: { win.goToSheet(tabMenu.index); win.deleteSheet(); }
        }
        MenuEntry {
            text: "Delete All"
            visible: !tabMenu.isSheet
            enabled: visible
            onTriggered: { win.goToSheet(tabMenu.index); win.deleteAllConstants(); }
        }
        MenuRule { visible: tabMenu.isSheet }
        // This one leaves the sheet shown as it is: the two are then side
        // by side, a window each.
        MenuEntry { text: "Open in new window"; onTriggered: win.newWindow(tabMenu.index) }
    }

    // ---- dialogs -----------------------------------------------------------

    Dialogs.FileDialog {
        id: openDialog
        title: "Open file"
        fileMode: Dialogs.FileDialog.OpenFile
        // A workbook, a CSV file, JSON or Parquet is imported: it opens as a new file.
        nameFilters: ["Files, workbooks, CSV, JSON and Parquet (*.omx *.xlsx *.csv *.json *.parquet)", "Omasheet (*.omx)", "All files (*)"]
        // Opens in the folder of the last file opened or saved, even after
        // a restart, with that file picked. The folder is set as well as the
        // file: a native dialog goes by the folder.
        function show() {
            var last = file.lastFile();
            if (last.length > 0) {
                currentFolder = Qt.url("file://" + encodeURI(last.substring(0, last.lastIndexOf("/"))));
                selectedFile = Qt.url("file://" + encodeURI(last));
            }
            open();
        }
        onAccepted: {
            win.pendingUrl = selectedFile;
            win.run("openUrl");
        }
    }

    Dialogs.FileDialog {
        id: frameDialog
        title: "Dataframe file"
        fileMode: Dialogs.FileDialog.OpenFile
        nameFilters: ["Parquet and CSV (*.parquet *.csv)", "All files (*)"]
        onAccepted: promptDialog.dataUrl = selectedFile
        onRejected: if (promptDialog.dataUrl.toString().length === 0) frameBox.checked = false
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
        Shortcut { sequence: "P"; enabled: exportChoice.visible && exportChoice.oneSheet; onActivated: exportChoice.choose("parquet") }
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
            Button {
                Layout.fillWidth: true
                enabled: exportChoice.oneSheet
                text: "Parquet (.parquet): " + (exportChoice.oneSheet ? "this sheet" : "show a sheet first") + "    P"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onClicked: exportChoice.choose("parquet")
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
             : kind === "parquet" ? "Export " + win.sheet.name + " as Parquet"
             : kind === "json" ? "Export as JSON" : "Export as a workbook"
        fileMode: Dialogs.FileDialog.SaveFile
        nameFilters: kind === "csv" ? ["CSV (*.csv)"]
                   : kind === "parquet" ? ["Parquet (*.parquet)"]
                   : kind === "json" ? ["JSON (*.json)"] : ["Workbook (*.xlsx)"]
        defaultSuffix: kind
        onAccepted: write(false)
        // Run the export to see what it would lose; where it would write a
        // null in place of a number, nothing is written until that is
        // answered, and answering it runs the export again.
        function write(force) {
            var name = win.sheet.isConsts ? "" : win.sheet.name;
            if (file.exportUrl(selectedFile, kind, name, force)) {
                if (file.notes.length > 0)
                    notesDialog.open();
            } else if (file.exportWarning.length > 0) {
                exportWarning.open();
            }
        }
    }

    // What an export would lose, put before it writes anything.
    Dialog {
        font.pixelSize: win.dialogSize
        id: exportWarning
        anchors.centerIn: parent
        modal: true
        focus: true
        width: Math.min(win.width - 80, 620)
        title: "Export"
        standardButtons: Dialog.Ok | Dialog.Cancel
        onClosed: grid.forceActiveFocus()
        onAccepted: exportDialog.write(true)
        contentItem: Text {
            text: file.exportWarning
            wrapMode: Text.WordWrap
            color: win.textColor
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
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
        standardButtons: Dialog.Ok
        onAccepted: file.notes = ""
        onClosed: grid.forceActiveFocus()
        // Selectable, to be copied: Ctrl+C copies what is selected, or all
        // of it.
        contentItem: Flickable {
            implicitHeight: Math.min(contentHeight, 360)
            contentHeight: notesText.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            TextEdit {
                id: notesText
                width: parent.width
                text: file.notes
                readOnly: true
                selectByMouse: true
                selectByKeyboard: true
                focus: true
                color: win.textColor
                selectionColor: win.accentColor
                wrapMode: Text.Wrap
                Keys.onPressed: function(event) {
                    if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_C) {
                        if (selectedText.length === 0)
                            selectAll();
                        copy();
                        event.accepted = true;
                    }
                }
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.noteSize
            }
        }
    }

    // Why something could not be done, where it must not be missed.
    Dialog {
        font.pixelSize: win.dialogSize
        id: errorDialog
        anchors.centerIn: parent
        modal: true
        // Over the sheet, which is left as it is rather than dimmed.
        dim: false
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
        // Selectable, to be copied: Ctrl+C copies what is selected, or all
        // of it.
        contentItem: TextEdit {
            id: errorText
            text: errorDialog.message
            readOnly: true
            selectByMouse: true
            selectByKeyboard: true
            focus: true
            color: win.textColor
            selectionColor: win.accentColor
            wrapMode: Text.Wrap
            Keys.onPressed: function(event) {
                if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_C) {
                    if (selectedText.length === 0)
                        selectAll();
                    copy();
                    event.accepted = true;
                }
            }
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
        }
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: deleteSheetDialog
        anchors.centerIn: parent
        modal: true
        width: 440
        focus: true
        title: "Delete sheet"
        standardButtons: Dialog.Ok | Dialog.Cancel
        // Each button names its key.
        Component.onCompleted: {
            standardButton(Dialog.Ok).text = "Delete    D";
            standardButton(Dialog.Cancel).text = "Cancel    C";
        }
        Shortcut { sequence: "D"; enabled: deleteSheetDialog.visible; onActivated: deleteSheetDialog.accept() }
        Shortcut { sequence: "C"; enabled: deleteSheetDialog.visible; onActivated: deleteSheetDialog.reject() }
        Label {
            width: parent.width
            text: "Delete " + win.sheet.name + " and all its cells?"
            wrapMode: Text.Wrap
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
        }
        onAccepted: win.deleteSheetNow()
        onClosed: grid.forceActiveFocus()
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: deleteConstDialog
        anchors.centerIn: parent
        modal: true
        width: 440
        focus: true
        title: "Delete constant"
        standardButtons: Dialog.Ok | Dialog.Cancel
        // Each button names its key.
        Component.onCompleted: {
            standardButton(Dialog.Ok).text = "Delete    D";
            standardButton(Dialog.Cancel).text = "Cancel    C";
        }
        Shortcut { sequence: "D"; enabled: deleteConstDialog.visible; onActivated: deleteConstDialog.accept() }
        Shortcut { sequence: "C"; enabled: deleteConstDialog.visible; onActivated: deleteConstDialog.reject() }
        Label {
            width: parent.width
            text: "Delete the constant " + (win.constGone >= 0 && win.constGone < win.sheet.nrows ? win.sheet.rows[win.constGone][0].d : "") + "? A formula that reads it will show an error."
            wrapMode: Text.Wrap
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
        }
        onAccepted: win.deleteConstantNow()
        onClosed: grid.forceActiveFocus()
    }

    Dialog {
        font.pixelSize: win.dialogSize
        id: deleteAllConstsDialog
        anchors.centerIn: parent
        modal: true
        width: 440
        focus: true
        title: "Delete all constants"
        standardButtons: Dialog.Ok | Dialog.Cancel
        // Each button names its key.
        Component.onCompleted: {
            standardButton(Dialog.Ok).text = "Delete    D";
            standardButton(Dialog.Cancel).text = "Cancel    C";
        }
        Shortcut { sequence: "D"; enabled: deleteAllConstsDialog.visible; onActivated: deleteAllConstsDialog.accept() }
        Shortcut { sequence: "C"; enabled: deleteAllConstsDialog.visible; onActivated: deleteAllConstsDialog.reject() }
        Label {
            width: parent.width
            text: "Delete all " + win.sheet.nrows + " constants? Formulas that read them will show errors."
            wrapMode: Text.Wrap
            font.family: win.font.family
            font.features: win.font.features
            font.pixelSize: win.dialogSize
        }
        onAccepted: win.deleteAllConstantsNow()
        onClosed: grid.forceActiveFocus()
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
        // The file a new sheet reads its rows from, when it is a dataframe.
        property url dataUrl
        // Asked for a number, in the one box.
        readonly property bool counted: kind === "rows" || kind === "columns"
        // A file's sheet holds no data of its own: a column added to it is a formula.
        readonly property bool formulaOnly: (kind === "column" || kind === "computed")
            && !win.sheet.isConsts && !win.sheet.view && (win.sheet.why || "").length > 0
        readonly property bool needsExpr: kind === "computed" || kind === "const"
            || (kind === "sheet" && formulaBox.checked)
        title: kind === "sheet" ? "New sheet"
             : kind === "rows" ? "Insert rows in " + win.sheet.name
             : kind === "columns" ? "Insert columns in " + win.sheet.name
             : kind === "renameSheet" ? "Rename sheet " + win.sheet.name
             : kind === "rename" ? "Rename column " + (win.sheet.columns[column] ? win.sheet.columns[column].name : "")
             : kind === "format" ? "Format of column " + (win.sheet.columns[column] ? win.sheet.columns[column].name : "")
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
                : what === "renameSheet" ? win.sheet.name
                : what === "column" ? win.suggestedColumnName(at)
                : what === "format" && win.sheet.columns[column] ? win.sheet.columns[column].format : "";
            exprField.text = "";
            if (what === "column" && !win.sheet.isConsts && !win.sheet.view && (win.sheet.why || "").length > 0)
                kind = "computed";
            formulaBox.checked = kind === "computed";
            frameBox.checked = false;
            dataUrl = "";
            countBox.text = "5";
            problem.text = "";
            open();
        }

        // Focus is taken once the dialog is up: asked for any sooner, it is
        // lost to the dialog opening.
        onOpened: takeFocus()
        function takeFocus() {
            forceActiveFocus();
            if (counted) {
                countBox.forceActiveFocus();
                countBox.deselect();
                countBox.cursorPosition = countBox.text.length;
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

        // What the number typed comes to, as the rows or columns it would
        // add are labelled in the sheet: "5 means 4..8" for rows, which are
        // numbered from 0 as the gutter has them, and "5 means D..H" for
        // columns. Blank while the number is not one that would be taken.
        readonly property string meaning: {
            var text = countBox.text.trim();
            var n = Number(text);
            if (!counted || text.length === 0 || !Number.isInteger(n) || n < 1)
                return "";
            if (kind === "rows") {
                if (n > 10000)
                    return "";
                var first = win.sheet.nrows === 0 ? 0 : win.selBottom + 1;
                return n + (n === 1 ? " means row " + first : " means " + first + ".." + (first + n - 1));
            }
            var have = win.sheet.columns.length;
            if (have + n > win.maxLetterColumns)
                return "";
            return n + (n === 1 ? " means column " + win.columnLetters(have)
                : " means " + win.columnLetters(have) + ".." + win.columnLetters(have + n - 1));
        }

        // Add the columns after the last, lettered on from it.
        function addLetterColumns() {
            var have = win.sheet.columns.length;
            var room = win.maxLetterColumns - have;
            var count = Number(countBox.text.trim());
            if (!Number.isInteger(count) || count < 1 || count > room)
                return "Enter a number of columns from 1 to " + room;
            var mark = file.undoMark();
            var problem = "";
            // At the end, so nothing marked moves.
            for (var k = 0; k < count && problem.length === 0; k++)
                problem = file.addColumn(win.tab, win.columnLetters(have + k));
            file.joinSince(mark);
            return problem;
        }

        function submit() {
            var message;
            var whole = win.selKind === "column";
            var all = win.selKind === "all";
            if (kind === "rows") {
                message = addRows();
            } else if (kind === "columns") {
                message = addLetterColumns();
            } else if (kind === "sheet") {
                // A new sheet renumbers the tabs, so nothing stays marked.
                var formula = formulaBox.checked ? exprField.text : "";
                if (frameBox.checked)
                    message = dataUrl.toString().length === 0 ? "Choose a file"
                        : file.insertView(at < 0 ? win.snap.sheets.length : at, nameField.text, dataUrl);
                else
                    message = at < 0 ? file.addSheet(nameField.text, formula)
                                     : file.insertSheet(at, nameField.text, formula);
            } else {
                // New rows go at the end, and new columns at `at` or the end.
                win.keeping(win.sheet.nrows, at < 0 ? win.sheet.columns.length : at, function() {
                    message = kind === "rename" ? file.renameColumn(win.tab, column, nameField.text)
                        : kind === "format" ? file.setColumnFormat(win.tab, win.selLeft, win.selRight - win.selLeft + 1, nameField.text)
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
            if (counted) {
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
                visible: promptDialog.counted
                Layout.fillWidth: true
                validator: IntValidator { bottom: 1; top: 10000 }
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onAccepted: promptDialog.tryAccept()
            }
            // What that number comes to, in the labels the sheet shows.
            Label {
                visible: promptDialog.counted && promptDialog.meaning.length > 0
                Layout.fillWidth: true
                text: promptDialog.meaning
                color: win.mutedColor
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.noteSize
            }
            TextField {
                id: nameField
                visible: !promptDialog.counted
                focus: true
                Layout.fillWidth: true
                placeholderText: promptDialog.kind === "format" ? win.formatExample(promptDialog.column) : "Name"
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
                    || promptDialog.kind === "sheet"
                text: "Formula"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onToggled: {
                    if (!checked && promptDialog.formulaOnly) {
                        checked = true;
                        problem.text = "A column of a file's sheet must be a formula: its data is read-only";
                        return;
                    }
                    // A sheet keeps its kind: it is one sheet either way,
                    // with rows of its own or a view of another's.
                    if (promptDialog.kind !== "sheet")
                        promptDialog.kind = checked ? "computed" : "column";
                    if (checked) {
                        frameBox.checked = false;
                        exprField.forceActiveFocus();
                    }
                }
            }
            // A sheet may be a dataframe: its rows read from a Parquet or CSV file.
            CheckBox {
                id: frameBox
                visible: promptDialog.kind === "sheet"
                text: "Dataframe"
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                onToggled: {
                    if (checked) {
                        formulaBox.checked = false;
                        frameDialog.open();
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                visible: promptDialog.kind === "sheet" && frameBox.checked && promptDialog.dataUrl.toString().length > 0
                text: promptDialog.dataUrl.toString().split("/").pop()
                elide: Text.ElideLeft
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.noteSize
            }
            TextField {
                id: exprField
                Layout.fillWidth: true
                visible: promptDialog.needsExpr
                placeholderText: promptDialog.kind === "computed" ? "Expression, e.g. Revenue - Cost"
                    : promptDialog.kind === "sheet" ? "Expression, e.g. Sales |> head(3)"
                    : "Expression, e.g. 20%"
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
                font.pixelSize: win.noteSize
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
        // A click goes to the cell, so Ctrl+C copies every problem.
        focus: true
        Shortcut {
            sequence: "Ctrl+C"
            enabled: problemsDialog.visible
            onActivated: {
                var lines = [];
                for (var i = 0; i < win.snap.problems.length; i++)
                    lines.push("line " + win.snap.problems[i].line + ": " + win.snap.problems[i].message);
                clipboard.put(lines.join("\n"));
            }
        }
        contentItem: ListView {
            implicitHeight: Math.min(contentHeight, 360)
            clip: true
            spacing: 8
            model: win.snap.problems.length
            delegate: Text {
                required property int index
                readonly property var item: win.snap.problems[index]
                // Whether a cell holds it, and so whether it can be gone to.
                readonly property bool goes: item && item.sheet >= 0
                width: ListView.view.width
                text: item ? "line " + item.line + ": " + item.message : ""
                color: win.textColor
                wrapMode: Text.Wrap
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.noteSize
                font.underline: goes && toCell.containsMouse
                MouseArea {
                    id: toCell
                    anchors.fill: parent
                    enabled: parent.goes
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        problemsDialog.close();
                        win.goToProblem(parent.item);
                    }
                }
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
        // The categories closed with their caret; a search shows them all.
        property var closed: ({})

        function toggle(category) {
            var c = Object.assign({}, closed);
            c[category] = !c[category];
            closed = c;
            refresh();
        }

        // Whether every category is closed with its caret.
        readonly property bool allClosed: {
            for (var i = 0; i < all.length; i++) {
                if (!closed[all[i].category])
                    return false;
            }
            return all.length > 0;
        }

        // Close every category, or open them all where all are closed.
        function toggleAll() {
            var c = {};
            if (!allClosed) {
                for (var i = 0; i < all.length; i++)
                    c[all[i].category] = true;
            }
            closed = c;
            refresh();
            functionsView.positionViewAtBeginning();
        }

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
                    out.push({ category: category, about: f.about });
                }
                if (words.length === 0 && closed[category])
                    continue;
                out.push(f);
            }
            shown = out;
        }

        onOpened: {
            // Read afresh each time: the file's own functions come and go.
            all = JSON.parse(file.functionsJson());
            copied = "";
            closed = {};
            searchField.text = "";
            refresh();
            searchField.forceActiveFocus();
        }
        onClosed: grid.forceActiveFocus()

        contentItem: ColumnLayout {
            spacing: 8
            Label {
                Layout.fillWidth: true
                text: file.functionsIntro()
                wrapMode: Text.Wrap
                color: win.mutedColor
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.fineSize
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                TextField {
                    id: searchField
                    Layout.fillWidth: true
                    placeholderText: "Search, e.g. sqrt, date, zone"
                    font.family: win.font.family
                    font.features: win.font.features
                    font.pixelSize: win.dialogSize
                    onTextChanged: functionsDialog.refresh()
                    Keys.onEscapePressed: functionsDialog.close()
                }
                // One caret for every category, as each heading has its own.
                // A search shows every category, so it does nothing while
                // one is typed.
                Text {
                    readonly property bool active: searchField.text.trim() === ""
                    text: functionsDialog.allClosed && active ? "\u25B8" : "\u25BE"
                    Layout.alignment: Qt.AlignVCenter
                    color: active ? win.accentColor : win.mutedColor
                    font.family: win.font.family
                    font.features: win.font.features
                    // As tall as the search box beside it.
                    font.pixelSize: Math.round(searchField.height * 0.8)
                    font.bold: true
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -6
                        enabled: parent.active
                        cursorShape: Qt.PointingHandCursor
                        onClicked: functionsDialog.toggleAll()
                    }
                }
            }
            ListView {
                id: functionsView
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: functionsDialog.shown.length
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                // The wheel scrolls the list, three lines a notch.
                WheelHandler {
                    onWheel: (event) => {
                        var step = event.pixelDelta.y !== 0 ? event.pixelDelta.y
                            : event.angleDelta.y / 120 * 3 * win.dialogSize * 1.5;
                        var bottom = functionsView.originY + functionsView.contentHeight - functionsView.height;
                        functionsView.contentY = Math.max(functionsView.originY,
                            Math.min(bottom, functionsView.contentY - step));
                        event.accepted = true;
                    }
                }
                delegate: Item {
                    id: entry
                    required property int index
                    readonly property var item: functionsDialog.shown[index] || ({})
                    readonly property bool heading: !item.name
                    width: ListView.view.width - 12
                    height: heading ? headingBody.implicitHeight + 16 : body.implicitHeight + 12

                    Column {
                        id: headingBody
                        visible: entry.heading
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 4
                        width: parent.width
                        spacing: 2
                        Text {
                            // The caret says whether the category is open.
                            text: (functionsDialog.closed[entry.item.category] && searchField.text.trim() === ""
                                ? "\u25B8 " : "\u25BE ") + (entry.item.category || "")
                            color: win.accentColor
                            font.family: win.font.family
                            font.features: win.font.features
                            font.pixelSize: win.dialogSize
                            font.bold: true
                        }
                        Text {
                            width: parent.width
                            visible: text.length > 0
                            text: entry.item.about || ""
                            color: win.mutedColor
                            wrapMode: Text.Wrap
                            font.family: win.font.family
                            font.features: win.font.features
                            font.pixelSize: win.fineSize
                        }
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
                            font.pixelSize: win.noteSize
                        }
                        Text {
                            width: parent.width
                            visible: text.length > 0
                            text: entry.item.summary || ""
                            color: win.mutedColor
                            wrapMode: Text.Wrap
                            font.family: win.font.family
                            font.features: win.font.features
                            font.pixelSize: win.fineSize
                        }
                    }
                    MouseArea {
                        id: entryArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: entry.heading ? Qt.PointingHandCursor : Qt.ArrowCursor
                        // Click a heading to open or close it, a function to
                        // copy the usage, ready to paste into a formula.
                        onClicked: {
                            if (entry.heading) {
                                functionsDialog.toggle(entry.item.category);
                                return;
                            }
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
                font.pixelSize: win.fineSize
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
            // Selectable, to be copied: Ctrl+C copies what is selected, or
            // all of it.
            TextEdit {
                id: helpText
                readOnly: true
                selectByMouse: true
                selectByKeyboard: true
                focus: true
                color: win.textColor
                selectionColor: win.accentColor
                Keys.onPressed: function(event) {
                    if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_C) {
                        if (selectedText.length === 0)
                            selectAll();
                        copy();
                        event.accepted = true;
                    }
                }
                font.family: win.font.family
                font.features: win.font.features
                font.pixelSize: win.dialogSize
                text: file.helpText()
            }
        }
    }
}
