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

    // The first window opened, the one whose size and place are kept.
    property var primary: null

    property Component view: Main {
        id: window
        file: app.file
        alone: app.count === 1
        onNewWindow: function(tab) { app.open(tab, fontSize, 0, 0, null, null); }
        onWidthChanged: if (window === app.primary) keep.restart()
        onHeightChanged: if (window === app.primary) keep.restart()
        onXChanged: if (window === app.primary) keep.restart()
        onYChanged: if (window === app.primary) keep.restart()
        onDismissed: {
            // Written as it closes, while it still has its size and place,
            // rather than left to a timer that may not get to run.
            if (window === app.primary) {
                app.keepPrimary();
                app.primary = null;
            }
            app.count--;
            destroy();
        }
    }

    // The size and place of the first window, kept per user so the next
    // launch opens it the same. Written a moment after it stops moving or
    // changing size, since dragging changes it many times, and again as the
    // window closes. Only ever a window's own geometry: nothing is written
    // once it has gone.
    function keepPrimary() {
        var w = primary;
        if (w === null)
            return;
        file.saveWindows(Math.round(w.width) + "," + Math.round(w.height)
            + "," + Math.round(w.x) + "," + Math.round(w.y));
    }

    property Timer keep: Timer {
        interval: 500
        onTriggered: app.keepPrimary()
    }

    // Autosave: a file that already has a name is saved once it has been
    // left alone after an edit for the seconds in the `autosave` file of
    // the user's config (`~/.config/omasheet/autosave`), ten if it says
    // nothing and never if it says 0. One that has never
    // been saved is left for the user to name, so no dialog comes up on its
    // own, and a formula still being typed is not in the file until it is
    // entered.
    readonly property int autosaveSeconds: file.autosaveSeconds()
    property Timer autosave: Timer {
        interval: Math.max(1, app.autosaveSeconds) * 1000
        onTriggered: {
            if (app.file.modified && app.file.filePath.length > 0)
                app.file.save();
        }
    }

    property Connections edits: Connections {
        target: app.file
        function onRevisionChanged() {
            if (app.file.modified && app.autosaveSeconds > 0)
                app.autosave.restart();
        }
    }

    // Whether a window kept at (x, y) would still have its title bar on a
    // screen, as when a monitor it was on has been unplugged since.
    function onScreen(x, y, width) {
        var screens = Qt.application.screens;
        for (var i = 0; i < screens.length; i++) {
            var s = screens[i];
            if (x + width > s.virtualX + 100 && x < s.virtualX + s.width - 100
                    && y >= s.virtualY && y < s.virtualY + s.height - 60)
                return true;
        }
        return false;
    }

    // The window kept from last time, as { width, height, x, y }, or null
    // if there is none; x and y are null where there is no place to put the
    // window back.
    function savedWindow() {
        var wh = file.savedWindows().split(";")[0].split(",");
        var w = parseInt(wh[0]), h = parseInt(wh[1]);
        if (!(w >= 640 && h >= 420 && w <= 10000 && h <= 10000))
            return null;
        var x = parseInt(wh[2]), y = parseInt(wh[3]);
        var placed = !isNaN(x) && !isNaN(y) && onScreen(x, y, w);
        return { width: w, height: h, x: placed ? x : null, y: placed ? y : null };
    }

    // `fontSize` is the size the window that asked for this one is at, so a
    // second window opens at the zoom of the first. Zero leaves it unset, and
    // the window takes the size the desktop is at, which is what the first
    // window opens at.
    function open(tab, fontSize, width, height, x, y) {
        count++;
        var props = { tab: tab };
        if (fontSize > 0)
            props.fontSize = fontSize;
        if (width > 0 && height > 0) {
            props.width = width;
            props.height = height;
        }
        if (x !== null && y !== null) {
            props.x = x;
            props.y = y;
        }
        var created = view.createObject(app, props);
        if (primary === null)
            primary = created;
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
        var saved = savedWindow();
        if (saved === null)
            open(0, 0, 0, 0, null, null);
        else
            open(0, 0, saved.width, saved.height, saved.x, saved.y);
    }
}
