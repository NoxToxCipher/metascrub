/*
 * A smoke test for the Qt backend both phones share (native/scrubber.cpp),
 * running against the real Rust core over the real C ABI.
 *
 * It is not a test of the scrubbing itself — the Rust workspace has 200-odd of
 * those, plus fuzzing. It exists to catch the things that break when a front end
 * is wired up wrong and still looks fine on screen:
 *
 *   - the report comes back but is parsed into the wrong fields
 *   - a cleaned copy is written that is not actually cleaned
 *   - a format the core cannot take apart is written out anyway, which is the
 *     one failure that would make the app lie
 *
 * Runs headless, needs no device, and is wired into CI on amd64.
 */
#include "scrubber.h"

#include <QCoreApplication>
#include <QFile>
#include <QImage>
#include <QTemporaryDir>
#include <QTextStream>
#include <QVariantList>
#include <QVariantMap>

static int failures = 0;

static void check(bool ok, const QString &what)
{
    QTextStream out(stdout);
    out << (ok ? "ok   " : "FAIL ") << what << "\n";
    if (!ok) {
        ++failures;
    }
}

/* A PNG carrying text chunks, which is metadata metascrub must remove. */
static bool writeTaggedPng(const QString &path)
{
    QImage image(8, 8, QImage::Format_RGB32);
    image.fill(Qt::darkCyan);
    image.setText(QStringLiteral("Author"), QStringLiteral("Jane Q. Photographer"));
    image.setText(QStringLiteral("Comment"), QStringLiteral("taken at home"));
    return image.save(path, "PNG");
}

int main(int argc, char *argv[])
{
    QCoreApplication app(argc, argv);

    QTemporaryDir dir;
    if (!dir.isValid()) {
        return 77; // no writable temp directory; nothing to say about the code
    }

    Scrubber scrubber;

    // --- a file the core can rebuild completely -----------------------------
    const QString source = dir.filePath(QStringLiteral("tagged.png"));
    if (!writeTaggedPng(source)) {
        QTextStream(stdout) << "FAIL could not write the test PNG\n";
        return 1;
    }

    const QVariantMap report = scrubber.inspect(source, false, false);
    check(report.value(QStringLiteral("ok")).toBool(), QStringLiteral("PNG was read"));
    check(report.value(QStringLiteral("assurance")).toString() == QLatin1String("complete"),
          QStringLiteral("PNG reports assurance complete"));
    check(report.value(QStringLiteral("writable")).toBool(),
          QStringLiteral("PNG is worth saving"));
    check(report.value(QStringLiteral("removedCount")).toInt() > 0,
          QStringLiteral("PNG text chunks were found"));

    // --- cleaning it, and cleaning it again ---------------------------------
    const QString cleaned = dir.filePath(QStringLiteral("cleaned.png"));
    const QString error = scrubber.save(source, cleaned, false, false, false, 1);
    check(error.isEmpty(), QStringLiteral("cleaned copy was written"));
    check(QFile::exists(cleaned), QStringLiteral("cleaned copy exists"));

    const QVariantMap after = scrubber.inspect(cleaned, false, false);
    check(after.value(QStringLiteral("ok")).toBool(), QStringLiteral("cleaned copy was read"));
    check(after.value(QStringLiteral("removedCount")).toInt() == 0,
          QStringLiteral("cleaned copy has nothing left to remove"));
    check(after.value(QStringLiteral("retained")).toList().isEmpty(),
          QStringLiteral("cleaned copy retains nothing"));
    check(!after.value(QStringLiteral("foundLocation")).toBool(),
          QStringLiteral("cleaned copy has no location"));

    // --- the guard: a format the core cannot take apart ---------------------
    // This must never produce a file. A written copy here would be the app
    // telling someone their file was cleaned when it was not.
    const QString opaque = dir.filePath(QStringLiteral("mystery.bin"));
    {
        QFile f(opaque);
        if (!f.open(QIODevice::WriteOnly)) {
            QTextStream(stdout) << "FAIL could not write the opaque test file\n";
            return 1;
        }
        f.write(QByteArray("\x7f\x45\x4c\x46not a format metascrub knows", 34));
    }

    const QVariantMap opaqueReport = scrubber.inspect(opaque, false, false);
    const bool claimsClean = opaqueReport.value(QStringLiteral("ok")).toBool()
                             && opaqueReport.value(QStringLiteral("writable")).toBool();
    check(!claimsClean, QStringLiteral("an unknown format is never reported as cleanable"));

    const QString opaqueDest = dir.filePath(QStringLiteral("mystery-cleaned.bin"));
    const QString opaqueError = scrubber.save(opaque, opaqueDest, false, false, false, 1);
    check(!opaqueError.isEmpty(), QStringLiteral("saving an unknown format reports an error"));
    check(!QFile::exists(opaqueDest),
          QStringLiteral("saving an unknown format writes no file"));

    // --- a file that is not there -------------------------------------------
    const QVariantMap missing = scrubber.inspect(dir.filePath(QStringLiteral("nope.jpg")),
                                                 false, false);
    check(!missing.value(QStringLiteral("ok")).toBool(),
          QStringLiteral("a missing file fails cleanly"));
    check(!missing.value(QStringLiteral("error")).toString().isEmpty(),
          QStringLiteral("a missing file explains itself"));

    QTextStream(stdout) << (failures == 0 ? "\nall checks passed\n"
                                          : QStringLiteral("\n%1 check(s) failed\n").arg(failures));
    return failures == 0 ? 0 : 1;
}
