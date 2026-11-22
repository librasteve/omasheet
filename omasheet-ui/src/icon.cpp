// Copyright (c) 2026 Stephen Roe

#include "icon.h"

#include <QDir>
#include <QFileInfo>
#include <QFileOpenEvent>
#include <QLocalServer>
#include <QLocalSocket>
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

namespace {
// Named for the user, and open to that user alone.
QString launch_name() {
  QString user = qEnvironmentVariable("USER", qEnvironmentVariable("USERNAME"));
  return QStringLiteral("omasheet-") + user;
}

class LaunchListener : public QObject {
public:
  LaunchListener(QQmlApplicationEngine *engine, QObject *parent)
      : QObject(parent), engine_(engine), server_(this) {
    server_.setSocketOptions(QLocalServer::UserAccessOption);
    // A socket left by an app that did not close down would refuse the name.
    QLocalServer::removeServer(launch_name());
    if (!server_.listen(launch_name())) {
      return;
    }
    connect(&server_, &QLocalServer::newConnection, this, [this] {
      while (QLocalSocket *peer = server_.nextPendingConnection()) {
        connect(peer, &QLocalSocket::disconnected, peer, &QObject::deleteLater);
        connect(peer, &QLocalSocket::readyRead, this, [this, peer] { drain(peer); });
        drain(peer);
      }
    });
  }

private:
  // One launch is one line, the path or nothing.
  void drain(QLocalSocket *peer) {
    while (peer->canReadLine()) {
      QString path = QString::fromUtf8(peer->readLine()).trimmed();
      const auto roots = engine_->rootObjects();
      if (!roots.isEmpty()) {
        QMetaObject::invokeMethod(roots.first(), "openExternal",
                                  Q_ARG(QVariant, path));
      }
    }
  }

  QQmlApplicationEngine *engine_;
  QLocalServer server_;
};
} // namespace

bool hand_over(const QString &path) {
  QLocalSocket socket;
  socket.connectToServer(launch_name());
  if (!socket.waitForConnected(300)) {
    return false;
  }
  // Whole, as this launch's folder is not the app's.
  QString whole = path.isEmpty() ? path : QFileInfo(path).absoluteFilePath();
  socket.write(whole.toUtf8() + '\n');
  socket.waitForBytesWritten(1000);
  socket.disconnectFromServer();
  return true;
}

void listen_for_launches(QQmlApplicationEngine &engine) {
  if (auto *app = QGuiApplication::instance()) {
    new LaunchListener(&engine, app);
  }
}
