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
    // AllEntries plus System, which is what includes a broken symbolic link.
    // Listing only Files and Dirs would walk straight past one, leaving it in
    // place while the interface reported everything cleared. Same set Qt's own
    // QDir::removeRecursively uses.
    const QFileInfoList entries = dir.entryInfoList(
        QDir::AllEntries | QDir::Hidden | QDir::System | QDir::NoDotAndDotDot);
    for (const QFileInfo &entry : entries) {
        // A symbolic link is deleted as a link and never followed. QFileInfo
        // resolves links, so isDir() is true for a link pointing at a
        // directory, and recursing into it would delete whatever is on the
        // other end. The files this clears arrive from other applications
        // through the Content Hub; a link planted among them would otherwise
        // turn "clear working files" into a delete somewhere else entirely.
        // Qt's own QDir::removeRecursively takes the same precaution.
        if (entry.isSymLink() || !entry.isDir()) {
            if (QFile::remove(entry.absoluteFilePath())) {
                ++removed;
            }
            continue;
        }
        // The Content Hub gives each transfer its own numbered directory.
        removed += emptyDir(entry.absoluteFilePath());
        QDir(entry.absoluteFilePath()).removeRecursively();
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
    // AllEntries plus System, which is what includes a broken symbolic link.
    // Listing only Files and Dirs would walk straight past one, leaving it in
    // place while the interface reported everything cleared. Same set Qt's own
    // QDir::removeRecursively uses.
    const QFileInfoList entries = dir.entryInfoList(
        QDir::AllEntries | QDir::Hidden | QDir::System | QDir::NoDotAndDotDot);
    for (const QFileInfo &entry : entries) {
        // Counted the same way it is deleted: a link counts as the one file it
        // is, and is not followed. Following would also let a link pointing at
        // its own parent spin here forever.
        n += (entry.isDir() && !entry.isSymLink()) ? countFiles(entry.absoluteFilePath()) : 1;
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
