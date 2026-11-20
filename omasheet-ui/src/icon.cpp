// Copyright (c) 2026 Stephen Roe

#include <QGuiApplication>
#include <QIcon>
#include <QPixmap>

void set_app_icon(const uint8_t *data, size_t len) {
  QPixmap pixmap;
  if (pixmap.loadFromData(data, static_cast<uint>(len))) {
    QGuiApplication::setWindowIcon(QIcon(pixmap));
  }
}
