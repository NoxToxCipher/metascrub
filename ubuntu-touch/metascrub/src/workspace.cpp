#include "workspace.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QRandomGenerator>
#include <QStandardPaths>
#include <QUrl>

namespace {

/*
 * A name that says nothing. Eight bytes of the system generator, hex encoded —
 * enough that two files cleaned in the same session do not collide, and it
 * carries no date, no place and no camera.
 */
QString randomStem()
{
    const quint64 n = QRandomGenerator::system()->generate64();
    return QStringLiteral("cleaned-") + QString::number(n, 16).rightJustified(16, QLatin1Char('0'));
}

/* Nothing from a file name is allowed to steer where we write. */
QString safeStem(const QString &in)
{
    QString s = in;
    s.replace(QLatin1Char('/'), QLatin1Char('_'));
    s.replace(QLatin1Char('\\'), QLatin1Char('_'));
    while (s.startsWith(QLatin1Char('.'))) {
        s.remove(0, 1);
    }
    s = s.trimmed();
    if (s.isEmpty()) {
        s = QStringLiteral("file");
    }
    // Long names are their own leak, and some filesystems refuse them.
    return s.left(80);
}

int emptyDir(const QString &path)
{
    QDir dir(path);
    if (!dir.exists()) {
        return 0;
    }
    int removed = 0;
    const QFileInfoList entries =
        dir.entryInfoList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot | QDir::Hidden);
    for (const QFileInfo &entry : entries) {
        if (entry.isDir()) {
            // The Content Hub gives each transfer its own numbered directory.
            removed += emptyDir(entry.absoluteFilePath());
            QDir(entry.absoluteFilePath()).removeRecursively();
        } else if (QFile::remove(entry.absoluteFilePath())) {
            ++removed;
        }
    }
    return removed;
}

int countFiles(const QString &path)
{
    QDir dir(path);
    if (!dir.exists()) {
        return 0;
    }
    int n = 0;
    const QFileInfoList entries =
        dir.entryInfoList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot | QDir::Hidden);
    for (const QFileInfo &entry : entries) {
        n += entry.isDir() ? countFiles(entry.absoluteFilePath()) : 1;
    }
    return n;
}

} // namespace

Workspace::Workspace(QObject *parent) : QObject(parent) {}

QString Workspace::cleanedDir() const
{
    const QString base = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    const QString path = base + QStringLiteral("/cleaned");
    QDir().mkpath(path);
    return path;
}

QString Workspace::incomingDir() const
{
    const QString base = QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
    return base + QStringLiteral("/HubIncoming");
}

QString Workspace::pathFromUrl(const QString &url) const
{
    if (url.startsWith(QLatin1String("file:"))) {
        return QUrl(url).toLocalFile();
    }
    return url;
}

QString Workspace::urlFromPath(const QString &path) const
{
    return QUrl::fromLocalFile(path).toString();
}

QString Workspace::baseName(const QString &path) const
{
    return QFileInfo(path).fileName();
}

bool Workspace::exists(const QString &path) const
{
    return QFileInfo::exists(path);
}

QString Workspace::destinationFor(const QString &srcPath, bool randomName, bool washed) const
{
    const QDir dir(cleanedDir());
    const QFileInfo info(srcPath);

    // A washed photo is decoded and re-encoded, so it leaves as a JPEG whatever
    // it arrived as. Anything else keeps its own extension.
    const QString ext = washed ? QStringLiteral("jpg") : info.suffix().toLower();
    const QString stem = randomName ? randomStem()
                                    : safeStem(info.completeBaseName())
                                          + QStringLiteral("-cleaned");

    const QString suffix = ext.isEmpty() ? QString() : QLatin1Char('.') + ext;
    QString name = stem + suffix;
    for (int n = 2; dir.exists(name); ++n) {
        name = stem + QStringLiteral("-") + QString::number(n) + suffix;
    }
    return dir.filePath(name);
}

int Workspace::workingFileCount() const
{
    return countFiles(cleanedDir()) + countFiles(incomingDir());
}

int Workspace::clearWorkingFiles()
{
    return emptyDir(cleanedDir()) + emptyDir(incomingDir());
}
