// Copyright (c) 2026 Stephen Roe

import QtQml
import QtQuick
import io.omacom.omasheet

// The app: a number of documents, each a file, with the windows open on it.
// Every window of a document shows that file, each on a sheet of its own
// choosing; a new file or an opened one is a document, and a window, of its
// own, in this one process, as in most apps.
QtObject {
    id: app

    // Each document: { file, windows }.
    property var docs: []
    property Component fileType: Component { File {} }
    // Whole rows or columns cut in any window, for the others to move.
    property QtObject cutHold: QtObject { property var grab: null; property var copy: null }

    // The first window opened, the one whose size and place are kept.
    property var primary: null

    property Component view: Main {
        id: window
        cutHold: app.cutHold
        onNewWindow: function(tab) { app.open(app.docOf(window.file), tab, fontSize, 0, 0, null, null); }
        onNewFile: app.openDocument("", fontSize)
        onOpenFile: function(url) { app.openDocument(url, fontSize); }
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
            app.release(window);
            destroy();
        }
    }

    // The document that has this file.
    function docOf(file) {
        for (var i = 0; i < docs.length; i++) {
            if (docs[i].file === file)
                return docs[i];
        }
        return null;
    }

    // A window has gone: the last window of a document takes the document
    // with it, and the one left of a document is its alone.
    function release(window) {
        var doc = docOf(window.file);
        if (doc === null)
            return;
        doc.windows = doc.windows.filter(function(w) { return w !== window; });
        if (doc.windows.length === 0) {
            docs = docs.filter(function(d) { return d !== doc; });
            Qt.callLater(function() { doc.file.destroy(); });
        } else {
            tell(doc);
        }
    }

    function tell(doc) {
        for (var i = 0; i < doc.windows.length; i++)
            doc.windows[i].alone = doc.windows.length === 1;
    }

    // A new document, and a window on it: blank for an empty `where`, or the
    // file at that URL or path. A file already open is brought forward, not
    // opened twice.
    function openDocument(where, fontSize) {
        var text = where.toString();
        var file = fileType.createObject(app);
        if (text.length > 0) {
            var path = text.indexOf("://") >= 0 ? file.localPath(where) : text;
            for (var i = 0; i < docs.length; i++) {
                if (docs[i].file.filePath === path) {
                    file.destroy();
                    docs[i].windows[0].raise();
                    docs[i].windows[0].requestActivate();
                    return docs[i];
                }
            }
            if (text.indexOf("://") >= 0)
                file.openUrl(where);
            else
                file.openPath(path);
        }
        file.followTheme(Application.styleHints.colorScheme !== Qt.Light);
        var doc = { file: file, windows: [] };
        docs = docs.concat([doc]);
        open(doc, 0, fontSize, 0, 0, null, null);
        return doc;
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
        w.file.saveWindows(Math.round(w.width) + "," + Math.round(w.height)
            + "," + Math.round(w.x) + "," + Math.round(w.y));
    }

    property Timer keep: Timer {
        interval: 500
        onTriggered: app.keepPrimary()
    }

    // Whether a window kept at (x, y) would still have its title bar on a
    // screen, as when a monitor it was on has been unplugged since.
    // The screen that has it, or null. The window is put on that screen
    // by name, as well as at the place: a place on another monitor than the
    // primary, said without the screen, is "outside any known screen" to Qt,
    // which then leaves the window out of sight.
    function screenFor(x, y, width) {
        var screens = Qt.application.screens;
        for (var i = 0; i < screens.length; i++) {
            var s = screens[i];
            if (x + width > s.virtualX + 100 && x < s.virtualX + s.width - 100
                    && y >= s.virtualY && y < s.virtualY + s.height - 60)
                return s;
        }
        return null;
    }

    // The window kept from last time, as { width, height, x, y }, or null
    // if there is none; x and y are null where there is no place to put the
    // window back.
    function savedWindow(file) {
        var wh = file.savedWindows().split(";")[0].split(",");
        var w = parseInt(wh[0]), h = parseInt(wh[1]);
        if (!(w >= 640 && h >= 420 && w <= 10000 && h <= 10000))
            return null;
        var x = parseInt(wh[2]), y = parseInt(wh[3]);
        var on = isNaN(x) || isNaN(y) ? null : screenFor(x, y, w);
        if (on) {
            // Qt wants the whole window inside the one screen: a window that
            // hangs over an edge, into the next screen or off the desktop, is
            // "outside any known screen". Pulled back in, and made smaller if
            // it is bigger than the screen.
            // What was kept is what the window had when it closed, which the
            // system had already fitted under the menu bar and the title
            // bar, so it is put back as it was: no room is left for them, or
            // a window tiled to the full height came back shorter, and lower,
            // each time.
            w = Math.min(w, on.width);
            h = Math.min(h, on.height);
            x = Math.max(on.virtualX, Math.min(x, on.virtualX + on.width - w));
            y = Math.max(on.virtualY, Math.min(y, on.virtualY + on.height - h));
        }
        return { width: w, height: h, x: on ? x : null, y: on ? y : null, screen: on };
    }

    // `fontSize` is the size the window that asked for this one is at, so a
    // second window opens at the zoom of the first. Zero leaves it unset, and
    // the window takes the size the desktop is at, which is what the first
    // window opens at.
    function open(doc, tab, fontSize, width, height, x, y, screen) {
        var props = { tab: tab, file: doc.file };
        if (fontSize > 0)
            props.fontSize = fontSize;
        if (width > 0 && height > 0) {
            props.width = width;
            props.height = height;
        }
        if (x !== null && y !== null) {
            if (screen)
                props.screen = screen;
            props.x = x;
            props.y = y;
        }
        var created = view.createObject(app, props);
        doc.windows = doc.windows.concat([created]);
        tell(doc);
        if (primary === null)
            primary = created;
    }

    // The theme is Omarchy's; elsewhere it goes with the system, light or dark.
    function followTheme() {
        for (var i = 0; i < docs.length; i++)
            docs[i].file.followTheme(Application.styleHints.colorScheme !== Qt.Light);
    }

    property Connections scheme: Connections {
        target: Application.styleHints
        function onColorSchemeChanged() { app.followTheme(); }
    }

    // A file the system asks the app to open, as a double click in the Finder
    // does once it is running: the command line has it only at launch.
    function openExternal(path) {
        if (path.length > 0) {
            openDocument(path, 0);
            return;
        }
        // A launch with no file brings the app forward.
        var last = docs.length > 0 ? docs[docs.length - 1].windows[0] : null;
        if (last) {
            last.raise();
            last.requestActivate();
        }
    }

    Component.onCompleted: {
        var args = Qt.application.arguments;
        // The first window opens at the size and place kept from last time.
        var file = fileType.createObject(app);
        file.followTheme(Application.styleHints.colorScheme !== Qt.Light);
        if (args.length > 1)
            file.openPath(args[1]);
        var doc = { file: file, windows: [] };
        docs = [doc];
        var saved = savedWindow(file);
        if (saved === null)
            open(doc, 0, 0, 0, 0, null, null);
        else
            open(doc, 0, 0, saved.width, saved.height, saved.x, saved.y, saved.screen);
    }
}
