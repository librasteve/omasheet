// Copyright (c) 2026 Stephen Roe

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.omacom.omasheet").qml_files(["qml/App.qml", "qml/Main.qml"]),
    )
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .file("src/sheet.rs")
    .build();
}
