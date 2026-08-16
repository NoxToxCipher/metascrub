#include <QGuiApplication>
#include <QQuickView>
#include <QScopedPointer>
#include <sailfishapp.h>

#include "scrubber.h"

int main(int argc, char *argv[])
{
    QScopedPointer<QGuiApplication> app(SailfishApp::application(argc, argv));

    // The native backend, callable from QML as `Scrubber { }`.
    qmlRegisterType<Scrubber>("org.crake.metascrub", 1, 0, "Scrubber");

    QScopedPointer<QQuickView> view(SailfishApp::createView());
    view->setSource(SailfishApp::pathTo(QStringLiteral("qml/harbour-metascrub.qml")));
    view->show();

    return app->exec();
}
