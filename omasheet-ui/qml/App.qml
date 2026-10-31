// Copyright (c) 2026 Stephen Roe

import QtQml
import QtQuick
import io.omacom.omasheet

// The app: one file, and the windows open on it. Every window shows the
// same file, each on a sheet of its own choosing.
QtObject {
    id: app

    property File file: File {}
    // How many windows are open.
    property int count: 0

    property Component view: Main {
        file: app.file
        alone: app.count === 1
        onNewWindow: function(tab) { app.open(tab, fontSize); }
        onDismissed: {
            app.count--;
            destroy();
        }
    }

    // `fontSize` is the size the window that asked for this one is at, so a
    // second window opens at the zoom of the first. Zero leaves it unset, and
    // the window takes the size the desktop is at, which is what the first
    // window opens at.
    function open(tab, fontSize) {
        count++;
        var props = { tab: tab };
        if (fontSize > 0)
            props.fontSize = fontSize;
        view.createObject(app, props);
    }

    // The theme is Omarchy's; elsewhere it goes with the system, light or dark.
    function followTheme() {
        file.followTheme(Application.styleHints.colorScheme !== Qt.Light);
    }

    property Connections scheme: Connections {
        target: Application.styleHints
        function onColorSchemeChanged() { app.followTheme(); }
    }

    Component.onCompleted: {
        followTheme();
        var args = Qt.application.arguments;
        if (args.length > 1)
            file.openPath(args[1]);
        open(0, 0);
    }
}
