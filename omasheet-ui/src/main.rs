// Copyright (c) 2026 Stephen Roe

//! Omasheet: an interactive grid for `.omx` sheets.
//!
//! The windows are Qt Quick (`qml/Main.qml`, opened by `qml/App.qml`);
//! everything they show and every edit goes through the one `Sheet` object
//! in `sheet.rs`, which wraps the engine's editable
//! [`Document`](omasheet_engine::Document).

mod json;
mod sheet;
mod theme;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    // Same look as the other Omarchy apps, unless the user chose a style.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: nothing else is running yet.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Material") };
    }

    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("omasheet"));
        app.as_mut().set_organization_name(&QString::from("Omacom"));
        app.as_mut()
            .set_organization_domain(&QString::from("omacom.io"));
    }
    QGuiApplication::set_desktop_file_name(&QString::from("omasheet"));

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/io/omacom/omasheet/qml/App.qml"));
    }
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
