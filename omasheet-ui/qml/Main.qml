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
    title: (sheet.modified ? "* " : "") + sheet.fileName + " - Omasheet"

    // ---- theme -------------------------------------------------------------

    readonly property color pageColor: sheet.themeBackground
    readonly property color textColor: sheet.themeForeground
    readonly property color accentColor: sheet.themeAccent
    readonly property color selectionFill: sheet.themeSelection
    readonly property color mutedColor: sheet.themeMuted
    readonly property color errorColor: sheet.themeError
    readonly property color lineColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.14)
    readonly property color panelColor: Qt.rgba(textColor.r, textColor.g, textColor.b, 0.05)
    readonly property color computedTint: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.07)

    // Ctrl+plus and Ctrl+minus change the size; everything scales from it.
    property int fontSize: 14
    readonly property int rowHeight: fontSize * 2
    readonly property int headerHeight: Math.round(fontSize * 3.3)
    readonly property int gutterWidth: Math.round(fontSize * 3.4)
    onFontSizeChanged: Qt.callLater(function() { win.measure(); win.reveal(); })

    function zoom(step) {
        fontSize = step === 0 ? 14 : Math.max(8, Math.min(40, fontSize + step));
    }

    Material.theme: sheet.darkMode ? Material.Dark : Material.Light
    Material.accent: accentColor
    Material.background: pageColor
    Material.foreground: textColor
    color: pageColor
    font.family: "monospace"
    font.pixelSize: fontSize

    FontMetrics {
        id: metrics
        font.family: "monospace"
        font.pixelSize: win.fontSize
    }

    // ---- document state ----------------------------------------------------

    Sheet {
        id: sheet
    }

    // The parsed snapshot, and the tabs made from it: one per table, plus
    // the constants shown as a table of their own.
    property var snap: ({ tables: [], consts: [], problems: [] })
    property var tabs: []
    property int tab: 0
    readonly property var table: tab < tabs.length ? tabs[tab] : emptyTable
    readonly property var emptyTable: ({ name: "", columns: [], rows: [], isConsts: false })
    property var colWidths: []
    property real totalWidth: 0

    property int curRow: 0
    property int curCol: 0
    property int anchorRow: 0
    property int anchorCol: 0
    readonly property int selTop: Math.min(curRow, anchorRow)
    readonly property int selBottom: Math.max(curRow, anchorRow)
    readonly property int selLeft: Math.min(curCol, anchorCol)
    readonly property int selRight: Math.max(curCol, anchorCol)

    property bool editing: false
    property string editSeed: ""
    property bool closeConfirmed: false
    property string pendingAction: ""
    property url pendingUrl

    function cellAt(row, col) {
        var r = table.rows[row];
        return r && r[col] ? r[col] : null;
    }

    function reload() {
        var parsed = JSON.parse(sheet.snapshotJson());
        var list = [];
        for (var i = 0; i < parsed.tables.length; i++) {
            parsed.tables[i].isConsts = false;
            list.push(parsed.tables[i]);
        }
        if (parsed.consts.length > 0) {
            var rows = [];
            for (var k = 0; k < parsed.consts.length; k++) {
                var c = parsed.consts[k];
                rows.push([
                    { d: c.name, s: c.name, e: "", n: false, f: false },
                    { d: c.source, s: c.source, e: c.error, n: false, f: true },
                    { d: c.error ? "#ERROR" : c.display, s: c.source, e: c.error, n: false, f: false }
                ]);
            }
            list.push({
                name: "Constants",
                isConsts: true,
                columns: [
                    { name: "Name", type: "", formula: "", computed: false },
                    { name: "Expression", type: "", formula: "", computed: false },
                    { name: "Value", type: "", formula: "", computed: true }
                ],
                rows: rows
            });
        }
        snap = parsed;
        tabs = list;
        if (tab >= tabs.length)
            tab = Math.max(0, tabs.length - 1);
        measure();
        clampCursor();
    }

    function measure() {
        var widths = [];
        var total = 0;
        var cols = table.columns;
        for (var c = 0; c < cols.length; c++) {
            var chars = Math.max(cols[c].name.length, cols[c].type.length + 2);
            // Leave room to read a short column formula in the header.
            if (cols[c].computed && !table.isConsts)
                chars = Math.max(chars, Math.round(Math.min(24, cols[c].formula.length + 3) * 0.8));
            for (var r = 0; r < table.rows.length; r++)
                chars = Math.max(chars, table.rows[r][c].d.length);
            var w = Math.round(Math.min(46, Math.max(7, chars + 2)) * metrics.averageCharacterWidth) + 14;
            widths.push(w);
            total += w;
        }
        colWidths = widths;
        totalWidth = total;
    }

    function clampCursor() {
        var maxRow = Math.max(0, table.rows.length - 1);
        var maxCol = Math.max(0, table.columns.length - 1);
        curRow = Math.min(curRow, maxRow);
        anchorRow = Math.min(anchorRow, maxRow);
        curCol = Math.min(curCol, maxCol);
        anchorCol = Math.min(anchorCol, maxCol);
    }

    function selectCell(row, col, extend) {
        if (table.rows.length === 0 || table.columns.length === 0)
            return;
        curRow = Math.max(0, Math.min(table.rows.length - 1, row));
        curCol = Math.max(0, Math.min(table.columns.length - 1, col));
        if (!extend) {
            anchorRow = curRow;
            anchorCol = curCol;
        }
        reveal();
    }

    function reveal() {
        rowsView.positionViewAtIndex(curRow, ListView.Contain);
        var x = gutterWidth;
        for (var c = 0; c < curCol; c++)
            x += colWidths[c];
        var w = colWidths[curCol] || 0;
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
        if (table.isConsts && curCol !== 1)
            return;
        anchorRow = curRow;
        anchorCol = curCol;
        editSeed = seed;
        editing = true;
    }

    function stopEditing() {
        editing = false;
        grid.forceActiveFocus();
    }

    function commit(text, dRow, dCol) {
        var row = curRow, col = curCol;
        editing = false;
        if (table.isConsts) {
            if (col === 1)
                sheet.setConst(row, text);
        } else {
            sheet.setCell(tab, row, col, text);
        }
        // Enter on the last row of a table opens a new row to carry on typing.
        if (dRow > 0 && row === table.rows.length - 1 && !table.isConsts)
            sheet.insertRows(tab, row + 1, 1);
        selectCell(row + dRow, col + dCol, false);
        grid.forceActiveFocus();
    }

    function copySelection() {
        if (table.isConsts) {
            var lines = [];
            for (var r = selTop; r <= selBottom; r++) {
                var parts = [];
                for (var c = selLeft; c <= selRight; c++)
                    parts.push(table.rows[r][c].d);
                lines.push(parts.join("\t"));
            }
            clipboard.put(lines.join("\n"));
        } else {
            clipboard.put(sheet.copyCells(tab, selTop, selLeft, selBottom, selRight));
        }
    }

    function clearSelection() {
        if (!table.isConsts)
            sheet.clearCells(tab, selTop, selLeft, selBottom, selRight);
    }

    function pasteSelection() {
        var text = clipboard.take();
        if (text.length === 0)
            return;
        if (table.isConsts) {
            if (curCol === 1)
                sheet.setConst(curRow, text.split("\n")[0]);
            return;
        }
        sheet.pasteCells(tab, selTop, selLeft, selBottom, selRight, text);
    }

    function insertRow(below) {
        insertRows(below, 1);
    }

    function insertRows(below, count) {
        if (table.isConsts || count < 1)
            return;
        var at = table.rows.length === 0 ? 0 : (below ? selBottom + 1 : selTop);
        // The left of the selection: a whole-row selection ends at the last
        // column, and following it would pan the view to the right.
        var col = selLeft;
        sheet.insertRows(tab, at, count);
        // Leave the new rows selected.
        selectCell(at, col, false);
        selectCell(at + count - 1, col, true);
    }

    function deleteRows() {
        if (table.isConsts || table.rows.length === 0)
            return;
        sheet.deleteRows(tab, selTop, selBottom - selTop + 1);
        // Back to the first column, with the view panned fully left.
        selectCell(selTop, 0, false);
        hflick.contentX = 0;
    }

    // Right-click: keep a selection that contains the cell, otherwise move
    // to it. A negative row or column means the whole column or row.
    function openMenu(row, col) {
        stopEditing();
        grid.forceActiveFocus();
        var maxRow = Math.max(0, table.rows.length - 1);
        var maxCol = Math.max(0, table.columns.length - 1);
        var inRows = row < 0 || (row >= selTop && row <= selBottom);
        var inCols = col < 0 || (col >= selLeft && col <= selRight);
        if (!(inRows && inCols)) {
            anchorRow = row < 0 ? 0 : row;
            curRow = row < 0 ? maxRow : row;
            anchorCol = col < 0 ? 0 : col;
            curCol = col < 0 ? maxCol : col;
        }
        cellMenu.popup();
    }

    // ---- files -------------------------------------------------------------

    function guard(action) {
        if (!sheet.modified) {
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
            sheet.newDocument();
            switchTab(0);
        } else if (action === "open") {
            openDialog.open();
        } else if (action === "openUrl") {
            if (sheet.openUrl(pendingUrl))
                switchTab(0);
        }
    }

    function save() {
        if (sheet.filePath.length === 0) {
            saveDialog.open();
            return false;
        }
        return sheet.save();
    }

    onClosing: function(close) {
        if (closeConfirmed || !sheet.modified)
            return;
        close.accepted = false;
        guard("close");
    }

    Connections {
        target: sheet
        function onRevisionChanged() { win.reload(); }
    }

    Component.onCompleted: {
        reload();
        var args = Qt.application.arguments;
        if (args.length > 1 && sheet.openPath(args[1]))
            switchTab(0);
        grid.forceActiveFocus();
    }

    // Follow a theme change made while the window was in the background.
    onActiveChanged: if (active) sheet.reloadTheme()

    // Development aid: OMASHEET_UI_SNAPSHOT=<file.png> saves a picture and quits.
    Timer {
        interval: 600
        running: sheet.snapshotPath.length > 0
        onTriggered: root.grabToImage(function(result) {
            result.saveToFile(sheet.snapshotPath);
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

    readonly property bool typing: editing || formulaField.activeFocus || promptDialog.opened

    Shortcut { sequence: "Ctrl+S"; context: Qt.ApplicationShortcut; onActivated: win.save() }
    Shortcut { sequence: "Ctrl+Shift+S"; context: Qt.ApplicationShortcut; onActivated: saveDialog.open() }
    Shortcut { sequence: "Ctrl+O"; context: Qt.ApplicationShortcut; onActivated: win.guard("open") }
    Shortcut { sequence: "Ctrl+N"; context: Qt.ApplicationShortcut; onActivated: win.guard("new") }
    Shortcut { sequence: "Ctrl+Z"; enabled: !win.typing; onActivated: sheet.undo() }
    Shortcut { sequences: ["Ctrl+Shift+Z", "Ctrl+Y"]; enabled: !win.typing; onActivated: sheet.redo() }
    Shortcut { sequences: ["Ctrl++", "Ctrl+="]; context: Qt.ApplicationShortcut; onActivated: win.zoom(1) }
    Shortcut { sequence: "Ctrl+-"; context: Qt.ApplicationShortcut; onActivated: win.zoom(-1) }
    Shortcut { sequence: "Ctrl+0"; context: Qt.ApplicationShortcut; onActivated: win.zoom(0) }
    Shortcut { sequence: "Ctrl+PgDown"; onActivated: win.switchTab(win.tab + 1) }
    Shortcut { sequence: "Ctrl+PgUp"; onActivated: win.switchTab(win.tab - 1) }
    Shortcut { sequence: "Ctrl+?"; context: Qt.ApplicationShortcut; onActivated: helpDialog.open() }
    Shortcut { sequences: ["Meta+F", "F11"]; context: Qt.ApplicationShortcut
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

                Text {
                    readonly property var column: win.table.columns[win.curCol]
                    text: column
                        ? (win.table.isConsts ? win.table.rows[win.curRow][0].d
                           : win.table.name + "[" + win.curRow + "; " + column.name + "]")
                        : ""
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
                    readonly property var cell: win.cellAt(win.curRow, win.curCol)
                    readonly property string source: cell ? cell.s : ""
                    readonly property bool locked: win.table.isConsts && win.curCol !== 1
                    readOnly: locked || !cell
                    placeholderText: cell ? "" : "No cell selected"
                    background: Item {}
                    onSourceChanged: if (!activeFocus) text = source
                    onActiveFocusChanged: if (!activeFocus) text = source
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
                    readonly property var column: win.table.columns[win.curCol]
                    visible: !!column && column.computed && !win.table.isConsts
                    text: "column formula"
                    color: win.accentColor
                    font: win.font
                }

                // What the column accepts, in the locale's way of writing it.
                Text {
                    readonly property var column: win.table.columns[win.curCol]
                    readonly property string hint: column && column.type && !win.table.isConsts
                        ? sheet.entryHint(column.type) : ""
                    visible: hint.length > 0
                    text: hint
                    color: win.mutedColor
                    font.family: win.font.family
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
            focus: true

            Keys.onPressed: function(event) {
                if (win.editing)
                    return;
                var ctrl = event.modifiers & Qt.ControlModifier;
                var shift = event.modifiers & Qt.ShiftModifier;
                var page = Math.max(1, Math.floor(rowsView.height / win.rowHeight) - 1);
                event.accepted = true;
                switch (event.key) {
                case Qt.Key_Up: win.selectCell(ctrl ? 0 : win.curRow - 1, win.curCol, shift); break;
                case Qt.Key_Down: win.selectCell(ctrl ? win.table.rows.length - 1 : win.curRow + 1, win.curCol, shift); break;
                case Qt.Key_Left: win.selectCell(win.curRow, ctrl ? 0 : win.curCol - 1, shift); break;
                case Qt.Key_Right: win.selectCell(win.curRow, ctrl ? win.table.columns.length - 1 : win.curCol + 1, shift); break;
                case Qt.Key_PageUp: win.selectCell(win.curRow - page, win.curCol, shift); break;
                case Qt.Key_PageDown: win.selectCell(win.curRow + page, win.curCol, shift); break;
                case Qt.Key_Home: win.selectCell(ctrl ? 0 : win.curRow, 0, shift); break;
                case Qt.Key_End: win.selectCell(ctrl ? win.table.rows.length - 1 : win.curRow, win.table.columns.length - 1, shift); break;
                case Qt.Key_Tab: win.selectCell(win.curRow, win.curCol + 1, false); break;
                case Qt.Key_Backtab: win.selectCell(win.curRow, win.curCol - 1, false); break;
                case Qt.Key_Return:
                case Qt.Key_Enter:
                    if (ctrl) win.insertRow(!shift);
                    else win.startEditing(null);
                    break;
                case Qt.Key_F2: win.startEditing(null); break;
                case Qt.Key_Delete:
                    if (ctrl) win.deleteRows();
                    else win.clearSelection();
                    break;
                case Qt.Key_Backspace: win.clearSelection(); break;
                case Qt.Key_Escape:
                    win.anchorRow = win.curRow;
                    win.anchorCol = win.curCol;
                    break;
                default:
                    if (ctrl && event.key === Qt.Key_C) {
                        win.copySelection();
                    } else if (ctrl && event.key === Qt.Key_X) {
                        win.copySelection();
                        win.clearSelection();
                    } else if (ctrl && event.key === Qt.Key_V) {
                        win.pasteSelection();
                    } else if (ctrl && event.key === Qt.Key_A) {
                        win.anchorRow = 0;
                        win.anchorCol = 0;
                        win.curRow = Math.max(0, win.table.rows.length - 1);
                        win.curCol = Math.max(0, win.table.columns.length - 1);
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
                text: "This sheet has no tables yet.\nUse + Table below to add one."
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
                        Rectangle {
                            width: win.gutterWidth
                            height: win.headerHeight
                            color: win.panelColor
                        }
                        Repeater {
                            model: win.table.columns.length
                            Rectangle {
                                id: headCell
                                required property int index
                                readonly property var column: win.table.columns[index]
                                width: win.colWidths[index] || 0
                                height: win.headerHeight
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
                                        font.pixelSize: win.fontSize
                                        font.bold: true
                                        elide: Text.ElideRight
                                    }
                                    Text {
                                        width: parent.width
                                        text: !headCell.column ? ""
                                            : headCell.column.computed && !win.table.isConsts
                                                ? ":= " + headCell.column.formula
                                                : headCell.column.type
                                        color: headCell.column && headCell.column.computed
                                            ? win.accentColor : win.mutedColor
                                        font.family: win.font.family
                                        font.pixelSize: win.fontSize - 3
                                        elide: Text.ElideRight
                                    }
                                }
                                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.lineColor }
                                MouseArea {
                                    anchors.fill: parent
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    onClicked: function(mouse) {
                                        if (mouse.button === Qt.RightButton) {
                                            win.openMenu(-1, headCell.index);
                                            return;
                                        }
                                        // Select the whole column.
                                        win.stopEditing();
                                        win.anchorRow = 0;
                                        win.anchorCol = (mouse.modifiers & Qt.ShiftModifier) ? win.anchorCol : headCell.index;
                                        win.curRow = Math.max(0, win.table.rows.length - 1);
                                        win.curCol = headCell.index;
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
                        model: win.table.rows.length
                        reuseItems: true
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
                                        win.anchorCol = 0;
                                        win.anchorRow = (mouse.modifiers & Qt.ShiftModifier) ? win.anchorRow : rowItem.index;
                                        win.curCol = Math.max(0, win.table.columns.length - 1);
                                        win.curRow = rowItem.index;
                                    }
                                }
                            }

                            Repeater {
                                model: win.table.columns.length
                                Rectangle {
                                    id: cellItem
                                    required property int index
                                    readonly property int row: rowItem.index
                                    readonly property var cell: win.cellAt(row, index)
                                    readonly property var column: win.table.columns[index]
                                    readonly property bool current: row === win.curRow && index === win.curCol
                                    readonly property bool selected: row >= win.selTop && row <= win.selBottom
                                        && index >= win.selLeft && index <= win.selRight
                                    width: win.colWidths[index] || 0
                                    height: win.rowHeight
                                    color: selected && !current
                                        ? Qt.rgba(win.selectionFill.r, win.selectionFill.g, win.selectionFill.b, 0.55)
                                        : column && column.computed ? win.computedTint : "transparent"

                                    Text {
                                        anchors.fill: parent
                                        anchors.leftMargin: 7
                                        anchors.rightMargin: 7
                                        visible: !(win.editing && cellItem.current)
                                        text: cellItem.cell ? cellItem.cell.d : ""
                                        color: cellItem.cell && cellItem.cell.e ? win.errorColor : win.textColor
                                        opacity: cellItem.cell && cellItem.cell.f && cellItem.column && !cellItem.column.computed ? 0.92 : 1
                                        font.family: win.font.family
                                        font.pixelSize: win.fontSize
                                        font.italic: !!cellItem.cell && cellItem.cell.f && !!cellItem.column && !cellItem.column.computed
                                        verticalAlignment: Text.AlignVCenter
                                        horizontalAlignment: cellItem.cell && cellItem.cell.n ? Text.AlignRight : Text.AlignLeft
                                        elide: Text.ElideRight
                                    }

                                    Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.lineColor }
                                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.lineColor }
                                    Rectangle {
                                        anchors.fill: parent
                                        visible: cellItem.current
                                        color: "transparent"
                                        border.width: 2
                                        border.color: win.accentColor
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        ToolTip.visible: containsMouse && !cellMenu.visible && !!cellItem.cell && cellItem.cell.e.length > 0
                                        ToolTip.delay: 400
                                        ToolTip.text: cellItem.cell ? cellItem.cell.e : ""
                                        onPressed: function(mouse) {
                                            if (mouse.button === Qt.RightButton) {
                                                win.openMenu(cellItem.row, cellItem.index);
                                                return;
                                            }
                                            if (win.editing)
                                                win.stopEditing();
                                            grid.forceActiveFocus();
                                            win.selectCell(cellItem.row, cellItem.index, mouse.modifiers & Qt.ShiftModifier);
                                        }
                                        onPositionChanged: function(mouse) {
                                            if (!(pressedButtons & Qt.LeftButton))
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
                                            if (mouse.button === Qt.LeftButton)
                                                win.startEditing(null);
                                        }
                                    }

                                    Loader {
                                        anchors.fill: parent
                                        active: win.editing && cellItem.current
                                        sourceComponent: TextField {
                                            font.family: win.font.family
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
                                            Component.onCompleted: {
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
                                        }
                                    }
                                }
                            }
                        }
                    }
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

        // Footer: table tabs on the left, actions and status on the right.
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
                        active: index === win.tab
                        onClicked: win.switchTab(index)
                    }
                }
                FooterButton { label: "+ Table"; quiet: true; onClicked: promptDialog.ask("table") }

                Item { Layout.fillWidth: true }

                // The locale, and how it writes a date.
                Text {
                    id: localeLabel
                    text: sheet.localeHint()
                    color: win.mutedColor
                    font.family: win.font.family
                    font.pixelSize: win.fontSize - 2
                    rightPadding: 10
                    MouseArea {
                        id: localeArea
                        anchors.fill: parent
                        hoverEnabled: true
                        ToolTip.visible: containsMouse
                        ToolTip.delay: 400
                        ToolTip.text: "Type dates and times this way, or the ISO way.\n"
                            + sheet.entryHint("Date") + "\n"
                            + sheet.entryHint("Time") + "\n"
                            + sheet.entryHint("DateTime")
                    }
                }

                Text {
                    visible: sheet.status.length > 0
                    text: sheet.status
                    color: win.mutedColor
                    font.family: win.font.family
                    font.pixelSize: win.fontSize - 2
                    elide: Text.ElideMiddle
                    Layout.maximumWidth: 320
                }
                FooterButton {
                    visible: win.snap.problems.length > 0
                    label: win.snap.problems.length + (win.snap.problems.length === 1 ? " problem" : " problems")
                    textColor: win.errorColor
                    onClicked: problemsDialog.open()
                }
                FooterButton { label: "+ Row"; quiet: true; enabled: !win.table.isConsts && win.tabs.length > 0; onClicked: win.insertRow(true) }
                FooterButton { label: "+ Column"; quiet: true; enabled: !win.table.isConsts && win.tabs.length > 0; onClicked: promptDialog.ask("column") }
                FooterButton { label: "+ Formula"; quiet: true; enabled: !win.table.isConsts && win.tabs.length > 0; onClicked: promptDialog.ask("computed") }
                FooterButton { label: "+ Constant"; quiet: true; onClicked: promptDialog.ask("const") }
                FooterButton { label: "?"; quiet: true; onClicked: helpDialog.open() }
            }
        }
    }

    component FooterButton: Rectangle {
        id: button
        property string label
        property bool active: false
        property bool quiet: false
        property color textColor: win.textColor
        signal clicked()
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
            font.pixelSize: win.fontSize - 1
            font.bold: button.active
        }
        MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            onClicked: {
                button.clicked();
                grid.forceActiveFocus();
            }
        }
    }

    // ---- context menu ------------------------------------------------------

    Menu {
        id: cellMenu
        readonly property bool editable: !win.table.isConsts
        readonly property bool hasRows: editable && win.table.rows.length > 0
        readonly property int rowCount: win.selBottom - win.selTop + 1
        onClosed: grid.forceActiveFocus()

        MenuItem { text: "Cut"; enabled: cellMenu.hasRows; onTriggered: { win.copySelection(); win.clearSelection(); } }
        MenuItem { text: "Copy"; enabled: win.table.rows.length > 0; onTriggered: win.copySelection() }
        MenuItem { text: "Paste"; enabled: win.table.rows.length > 0; onTriggered: win.pasteSelection() }
        MenuItem { text: "Clear"; enabled: cellMenu.hasRows; onTriggered: win.clearSelection() }
        MenuSeparator {}
        MenuItem { text: "Edit cell"; enabled: win.table.rows.length > 0; onTriggered: win.startEditing(null) }
        MenuSeparator {}
        MenuItem {
            text: cellMenu.rowCount > 1 ? "Insert " + cellMenu.rowCount + " rows above" : "Insert row above"
            enabled: cellMenu.editable
            onTriggered: win.insertRows(false, cellMenu.rowCount)
        }
        MenuItem {
            text: cellMenu.rowCount > 1 ? "Insert " + cellMenu.rowCount + " rows below" : "Insert row below"
            enabled: cellMenu.editable
            onTriggered: win.insertRows(true, cellMenu.rowCount)
        }
        MenuItem { text: "Insert rows…"; enabled: cellMenu.editable; onTriggered: promptDialog.ask("rows") }
        MenuItem {
            text: cellMenu.rowCount > 1 ? "Delete " + cellMenu.rowCount + " rows" : "Delete row"
            enabled: cellMenu.hasRows
            onTriggered: win.deleteRows()
        }
        MenuSeparator {}
        MenuItem {
            text: "Rename column…"
            enabled: cellMenu.editable && win.selLeft === win.selRight && win.table.columns.length > 0
            onTriggered: promptDialog.ask("rename")
        }
        MenuItem { text: "Add column…"; enabled: cellMenu.editable; onTriggered: promptDialog.ask("column") }
        MenuItem { text: "Add formula column…"; enabled: cellMenu.editable; onTriggered: promptDialog.ask("computed") }
        MenuSeparator {}
        MenuItem { text: "Undo"; enabled: sheet.canUndo; onTriggered: sheet.undo() }
        MenuItem { text: "Redo"; enabled: sheet.canRedo; onTriggered: sheet.redo() }
    }

    // ---- dialogs -----------------------------------------------------------

    Dialogs.FileDialog {
        id: openDialog
        title: "Open sheet"
        fileMode: Dialogs.FileDialog.OpenFile
        nameFilters: ["Omasheet (*.omx)", "All files (*)"]
        onAccepted: {
            win.pendingUrl = selectedFile;
            win.run("openUrl");
        }
    }

    Dialogs.FileDialog {
        id: saveDialog
        title: "Save sheet"
        fileMode: Dialogs.FileDialog.SaveFile
        nameFilters: ["Omasheet (*.omx)"]
        defaultSuffix: "omx"
        onAccepted: {
            if (sheet.saveUrl(selectedFile) && win.pendingAction.length > 0) {
                var action = win.pendingAction;
                win.pendingAction = "";
                win.run(action);
            }
        }
        onRejected: win.pendingAction = ""
    }

    Dialog {
        id: unsavedDialog
        anchors.centerIn: parent
        modal: true
        width: 440
        title: "Unsaved changes"
        standardButtons: Dialog.Save | Dialog.Discard | Dialog.Cancel
        Label {
            width: parent.width
            text: sheet.fileName + " has changes that are not saved."
            wrapMode: Text.Wrap
            font: win.font
        }
        // Saving a new sheet asks for a name first; the pending action then
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
        id: promptDialog
        anchors.centerIn: parent
        modal: true
        width: 460
        property string kind: ""
        property int column: 0
        readonly property bool needsExpr: kind === "computed" || kind === "const"
        title: kind === "table" ? "New table"
             : kind === "rows" ? "Insert rows in " + win.table.name
             : kind === "rename" ? "Rename column " + (win.table.columns[column] ? win.table.columns[column].name : "")
             : kind === "column" ? "New column in " + win.table.name
             : kind === "computed" ? "New formula column in " + win.table.name
             : "New constant"
        standardButtons: Dialog.Ok | Dialog.Cancel

        function ask(what) {
            kind = what;
            column = win.curCol;
            nameField.text = what === "rename" && win.table.columns[column] ? win.table.columns[column].name : "";
            exprField.text = "";
            problem.text = "";
            open();
            nameField.forceActiveFocus();
            nameField.selectAll();
        }

        // Add the rows below the selection.
        function addRows() {
            var count = Number(nameField.text.trim());
            if (!Number.isInteger(count) || count < 1 || count > 10000)
                return "Enter a number of rows from 1 to 10000";
            win.insertRows(true, count);
            return "";
        }

        function submit() {
            var message = kind === "rows" ? addRows()
                : kind === "rename" ? sheet.renameColumn(win.tab, column, nameField.text)
                : kind === "table" ? sheet.addTable(nameField.text)
                : kind === "column" ? sheet.addColumn(win.tab, nameField.text)
                : kind === "computed" ? sheet.addComputed(win.tab, nameField.text, exprField.text)
                : sheet.addConst(nameField.text, exprField.text);
            if (message.length > 0) {
                problem.text = message;
                return false;
            }
            if (kind === "table")
                win.switchTab(win.snap.tables.length - 1);
            return true;
        }

        onAccepted: if (!submit()) open()
        onClosed: grid.forceActiveFocus()

        contentItem: ColumnLayout {
            spacing: 6
            TextField {
                id: nameField
                Layout.fillWidth: true
                placeholderText: promptDialog.kind === "rows" ? "How many rows" : "Name"
                font: win.font
                onAccepted: promptDialog.needsExpr ? exprField.forceActiveFocus() : promptDialog.accept()
            }
            TextField {
                id: exprField
                Layout.fillWidth: true
                visible: promptDialog.needsExpr
                placeholderText: promptDialog.kind === "computed" ? "Expression, e.g. Revenue - Cost" : "Expression, e.g. 20%"
                font: win.font
                onAccepted: promptDialog.accept()
            }
            Label {
                id: problem
                Layout.fillWidth: true
                visible: text.length > 0
                color: win.errorColor
                wrapMode: Text.Wrap
                font.family: win.font.family
                font.pixelSize: win.fontSize - 1
            }
        }
    }

    Dialog {
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
                font.pixelSize: win.fontSize - 1
            }
        }
    }

    Dialog {
        id: helpDialog
        anchors.centerIn: parent
        modal: true
        width: 560
        title: "Keyboard shortcuts"
        standardButtons: Dialog.Close
        onClosed: grid.forceActiveFocus()
        Label {
            font: win.font
            text: "Arrows, Tab          Move\n"
                + "Shift+Arrows, drag   Select a block\n"
                + "Right click          Menu: insert rows, rename a column, …\n"
                + "Enter, F2, typing    Edit the cell\n"
                + "Enter / Tab          Commit and move down / right\n"
                + "Esc                  Cancel the edit\n"
                + "Delete               Clear the selection\n"
                + "Ctrl+C / X / V       Copy / cut / paste\n"
                + "Ctrl+Enter           Insert a row below (Shift: above)\n"
                + "Ctrl+Delete          Delete the selected rows\n"
                + "Ctrl+Z / Ctrl+Y      Undo / redo\n"
                + "Ctrl+PgUp / PgDn     Previous / next table\n"
                + "Ctrl+O / S           Open / save\n"
                + "Ctrl+Shift+S         Save as\n"
                + "Ctrl+N               New sheet\n"
                + "Ctrl++ / Ctrl+-      Larger / smaller text (Ctrl+0 resets)\n"
                + "F11                  Fullscreen\n\n"
                + "A cell starting with = is a formula.\n"
                + "The entry box shows what the column accepts.\n"
                + "Dates and times: " + sheet.localeHint() + "\n"
                + "Editing a shaded column changes its formula for every row."
        }
    }
}
