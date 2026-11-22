// Copyright (c) 2026 Stephen Roe

#pragma once

#include <cstddef>
#include <cstdint>

#include <QtCore/QString>
#include <QtQml/QQmlApplicationEngine>

void set_app_icon(const uint8_t *data, size_t len);

// Have a file the system asks the app to open (a double click in the Finder
// sends an open-file event, not a command line) handed to the root object's
// `openExternal(path)`.
void forward_open_events(QQmlApplicationEngine &engine);

// One app to a user, on Windows and Linux (a Mac does it itself): a launch
// that finds the app already running sends it the file to open, `path`, or
// nothing, and `hand_over` is true: the caller then exits. False if there is
// no app running.
bool hand_over(const QString &path);

// Listen for those launches, and give each one to the root object's
// `openExternal(path)`, with an empty path for a launch with no file.
void listen_for_launches(QQmlApplicationEngine &engine);
