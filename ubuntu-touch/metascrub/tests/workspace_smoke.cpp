/*
 * Tests for the app's own storage, and mostly for the one operation in it that
 * can destroy something: clearing the working files.
 *
 * The files it clears did not come from this app. They were handed over by
 * another application through the Content Hub, and their names came with them.
 * So the two things worth proving are that a name can never steer where a
 * cleaned copy is written, and that clearing deletes only what is inside the
 * app's own directories.
 *
 * QStandardPaths test mode moves those directories into a temporary tree, so
 * this never touches a real home directory.
 */
#include "workspace.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QTextStream>

static int failures = 0;

static void check(bool ok, const QString &what)
{
    QTextStream out(stdout);
    out << (ok ? "ok   " : "FAIL ") << what << "\n";
    if (!ok) {
        ++failures;
    }
}

static bool writeFile(const QString &path, const QByteArray &body)
{
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly)) {
        return false;
    }
    f.write(body);
    return true;
}

int main(int argc, char *argv[])
{
    QCoreApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("metascrub.noxtoxcipher"));
    QStandardPaths::setTestModeEnabled(true);

    Workspace workspace;
    // Start from nothing: these directories persist between runs, and a test
    // that passes only on a clean machine is not a test.
    workspace.clearWorkingFiles();
    const QString cleaned = workspace.cleanedDir();
    const QString incoming = workspace.incomingDir();
    QDir().mkpath(incoming);
    check(!cleaned.isEmpty() && QFileInfo::exists(cleaned),
          QStringLiteral("the cleaned directory is created on demand"));

    // --- a file name must never decide where we write -----------------------
    const QString traversal =
        workspace.destinationFor(QStringLiteral("/tmp/../../../etc/passwd"), false, false);
    check(QFileInfo(traversal).absolutePath() == QFileInfo(cleaned).absoluteFilePath(),
          QStringLiteral("a path in the source name cannot escape the cleaned directory"));

    const QString sneaky =
        workspace.destinationFor(QStringLiteral("/tmp/..%2f..%2fetc%2fshadow.jpg"), false, false);
    check(!QFileInfo(sneaky).fileName().contains(QLatin1Char('/')),
          QStringLiteral("no separator survives into the written name"));

    const QString dotted = workspace.destinationFor(QStringLiteral("/tmp/...hidden.png"),
                                                    false, false);
    check(!QFileInfo(dotted).fileName().startsWith(QLatin1Char('.')),
          QStringLiteral("a leading dot is not carried into the written name"));

    const QString random = workspace.destinationFor(QStringLiteral("/tmp/IMG_20240113.jpg"),
                                                    true, false);
    check(!QFileInfo(random).fileName().contains(QStringLiteral("IMG_20240113")),
          QStringLiteral("a random name keeps nothing of the original"));
    check(QFileInfo(random).suffix() == QLatin1String("jpg"),
          QStringLiteral("a random name keeps the extension"));

    const QString washed = workspace.destinationFor(QStringLiteral("/tmp/photo.png"), true, true);
    check(QFileInfo(washed).suffix() == QLatin1String("jpg"),
          QStringLiteral("a washed photo is written as JPEG"));

    // Two calls for the same source must not collide.
    check(!writeFile(random, "x") || workspace.destinationFor(
              QStringLiteral("/tmp/IMG_20240113.jpg"), false, false) != random,
          QStringLiteral("an existing name is never handed out twice"));

    // --- clearing deletes inside, and only inside ---------------------------
    QTemporaryDir elsewhere;
    if (!elsewhere.isValid()) {
        return 77;
    }
    const QString treasure = elsewhere.filePath(QStringLiteral("family-photo.jpg"));
    if (!writeFile(treasure, "not ours to delete")) {
        QTextStream(stdout) << "FAIL could not set up the outside directory\n";
        return 1;
    }

    if (!writeFile(cleaned + QStringLiteral("/inside.jpg"), "ours")) {
        QTextStream(stdout) << "FAIL could not set up the inside file\n";
        return 1;
    }

    // A link pointing out of the app's storage, of the kind that could arrive
    // among files handed over by another application.
    const QString link = incoming + QStringLiteral("/somewhere-else");
    QFile::link(elsewhere.path(), link);
    const bool linkMade = QFileInfo(link).isSymLink();

    const int before = workspace.workingFileCount();
    check(before > 0, QStringLiteral("working files are counted"));

    workspace.clearWorkingFiles();

    check(!QFileInfo::exists(cleaned + QStringLiteral("/inside.jpg")),
          QStringLiteral("clearing removes the app's own files"));
    check(QFileInfo::exists(treasure),
          linkMade ? QStringLiteral("clearing does not follow a link out of the app's storage")
                   : QStringLiteral("clearing leaves outside files alone (no link support here)"));
    check(!QFileInfo(link).isSymLink(),
          QStringLiteral("the link itself is removed"));
    check(workspace.workingFileCount() == 0,
          QStringLiteral("nothing is left afterwards"));

    QTextStream(stdout) << (failures == 0 ? "\nall checks passed\n"
                                          : QStringLiteral("\n%1 check(s) failed\n").arg(failures));
    return failures == 0 ? 0 : 1;
}
