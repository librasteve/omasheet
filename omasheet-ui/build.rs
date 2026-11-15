// Copyright (c) 2026 Stephen Roe

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    // SAFETY: the closure only adds a compiler flag.
    unsafe {
        CxxQtBuilder::new_qml_module(
            QmlModule::new("io.omacom.omasheet").qml_files(["qml/App.qml", "qml/Main.qml"]),
        )
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/file.rs")
        // MSVC reads the C++ Qt makes of the QML, strings and all, in the
        // system code page unless it is told it is UTF-8, and `▾` comes out
        // as `â–¾`.
        .cc_builder(|cc| {
            if msvc {
                cc.flag("/utf-8");
            }
        })
        .build();
    }
}
