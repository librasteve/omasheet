// Copyright (c) 2026 Stephen Roe

import QtQml
import io.omacom.omasheet

// The app: one sheet, and the windows open on it. Every window shows the
// same sheet, each on a table of its own choosing.
QtObject {
    id: app

    property Sheet sheet: Sheet {}
    // How many windows are open.
    property int count: 0

    property Component view: Main {
        sheet: app.sheet
        alone: app.count === 1
        onNewWindow: function(tab) { app.open(tab, fontSize); }
        onDismissed: {
            app.count--;
            destroy();
        }
    }

    function open(tab, fontSize) {
        count++;
        view.createObject(app, { tab: tab, fontSize: fontSize });
    }

    Component.onCompleted: {
        var args = Qt.application.arguments;
        if (args.length > 1)
            sheet.openPath(args[1]);
        open(0, 14);
    }
}
