// Copyright (c) 2026 Stephen Roe

#include "icon.h"

#include <QFileOpenEvent>
#include <QGuiApplication>
#include <QIcon>
#include <QMetaObject>
#include <QObject>
#include <QPixmap>
#include <QVariant>

void set_app_icon(const uint8_t *data, size_t len) {
  QPixmap pixmap;
  if (pixmap.loadFromData(data, static_cast<uint>(len))) {
    QGuiApplication::setWindowIcon(QIcon(pixmap));
  }
}

namespace {
class OpenForwarder : public QObject {
public:
  OpenForwarder(QQmlApplicationEngine *engine, QObject *parent)
      : QObject(parent), engine_(engine) {}

  bool eventFilter(QObject *, QEvent *event) override {
    if (event->type() != QEvent::FileOpen) {
      return false;
    }
    auto *open = static_cast<QFileOpenEvent *>(event);
    QString path = open->url().isLocalFile() ? open->url().toLocalFile()
                                             : open->file();
    const auto roots = engine_->rootObjects();
    if (!path.isEmpty() && !roots.isEmpty()) {
      QMetaObject::invokeMethod(roots.first(), "openExternal",
                                Q_ARG(QVariant, path));
    }
    return true;
  }

private:
  QQmlApplicationEngine *engine_;
};
} // namespace

void forward_open_events(QQmlApplicationEngine &engine) {
  if (auto *app = QGuiApplication::instance()) {
    app->installEventFilter(new OpenForwarder(&engine, app));
  }
}
