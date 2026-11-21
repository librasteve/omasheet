// Copyright (c) 2026 Stephen Roe

#pragma once

#include <cstddef>
#include <cstdint>

#include <QtQml/QQmlApplicationEngine>

void set_app_icon(const uint8_t *data, size_t len);

// Have a file the system asks the app to open (a double click in the Finder
// sends an open-file event, not a command line) handed to the root object's
// `openExternal(path)`.
void forward_open_events(QQmlApplicationEngine &engine);
