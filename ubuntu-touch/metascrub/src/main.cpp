#include <QCoreApplication>
#include <QDir>
#include <QGuiApplication>
#include <QLocale>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickView>
#include <QUrl>
#include <QtQml>

#include "scrubber.h"
#include "workspace.h"

/*
 * metascrub for Ubuntu Touch.
 *
 * One binary: the Rust core is linked in statically, the Qt backend is the same
 * file the Sailfish app compiles (native/scrubber.cpp), and the interface is
 * Lomiri QML loaded from beside the executable inside the click.
 *
 * The application name is the click package name on purpose. Everything the app
 * is allowed to write — the cleaned copies, the Content Hub's incoming files —
 * lives under that name in ~/.local/share and ~/.cache, which is exactly what
 * the AppArmor profile permits and nothing more.
 */
int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("metascrub.noxtoxcipher"));
    app.setApplicationVersion(QStringLiteral("0.1.0"));

    qmlRegisterType<Scrubber>("org.crake.metascrub", 1, 0, "Scrubber");
    qmlRegisterType<Workspace>("org.crake.metascrub", 1, 0, "Workspace");

    QQuickView view;

    // Where the interface finds its translations, and which way round it should
    // read. The locale comes from the system: LANGUAGE, then LC_ALL, LC_MESSAGES
    // and LANG, the ordinary gettext order.
    const QString appDir = QCoreApplication::applicationDirPath();
    view.rootContext()->setContextProperty(
        QStringLiteral("localeDir"), QDir(appDir).filePath(QStringLiteral("share/locale")));
    view.rootContext()->setContextProperty(
        QStringLiteral("systemLocale"), QLocale::system().name());
    view.rootContext()->setContextProperty(
        QStringLiteral("systemRightToLeft"),
        QLocale::system().textDirection() == Qt::RightToLeft);

    view.setResizeMode(QQuickView::SizeRootObjectToView);
    view.setSource(QUrl::fromLocalFile(
        QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("qml/Main.qml"))));
    if (view.status() == QQuickView::Error) {
        return 1;
    }
    view.show();

    return app.exec();
}
