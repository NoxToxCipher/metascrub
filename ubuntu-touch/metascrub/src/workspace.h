#ifndef WORKSPACE_H
#define WORKSPACE_H

#include <QObject>
#include <QString>

/*
 * The file plumbing an Ubuntu Touch app needs, and the parts of it a confined
 * app has to think about.
 *
 * metascrub asks for no filesystem access beyond its own two directories. It
 * never opens a path a user typed, and it cannot read the picture library. Files
 * arrive through the Content Hub, which drops a *copy* into the app's cache, and
 * cleaned copies are written into the app's data directory and handed back out
 * through the Content Hub as well.
 *
 * That leaves copies of private files sitting in the app's storage, so removing
 * them is a first-class operation here rather than an afterthought:
 * clearWorkingFiles() empties both directories, and the interface offers it.
 *
 * Honest limit, stated once here and again in the Handbook: this deletes files
 * the ordinary way. On flash storage the blocks may survive until they are
 * reused, and nothing an application can do changes that.
 */
class Workspace : public QObject
{
    Q_OBJECT
public:
    explicit Workspace(QObject *parent = nullptr);

    /* Where cleaned copies are written, created on first use. */
    Q_INVOKABLE QString cleanedDir() const;

    /* Where the Content Hub leaves files other apps hand to metascrub. */
    Q_INVOKABLE QString incomingDir() const;

    /* file:// URL to a local path, and back. Handles percent-encoded names. */
    Q_INVOKABLE QString pathFromUrl(const QString &url) const;
    Q_INVOKABLE QString urlFromPath(const QString &path) const;

    /* The file name alone, for display. */
    Q_INVOKABLE QString baseName(const QString &path) const;

    Q_INVOKABLE bool exists(const QString &path) const;

    /*
     * Where a cleaned copy of srcPath should be written.
     *
     * With randomName the copy is called something like cleaned-8f3a1c7d.jpg,
     * because the name is metadata too: IMG_20240113_Brisbane.jpg says where and
     * when before the file is even opened. A washed photo is re-encoded to JPEG,
     * so it takes that extension whatever the original was. Never returns a path
     * that already exists.
     */
    Q_INVOKABLE QString destinationFor(const QString &srcPath, bool randomName,
                                       bool washed) const;

    /* How many working copies are sitting in the app's storage right now. */
    Q_INVOKABLE int workingFileCount() const;

    /* Delete them all. Returns how many were removed. */
    Q_INVOKABLE int clearWorkingFiles();
};

#endif // WORKSPACE_H
