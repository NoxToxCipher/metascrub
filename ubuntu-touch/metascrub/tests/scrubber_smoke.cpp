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
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QImage>
#include <QTemporaryDir>
#include <QTextStream>
#include <QThread>
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

    // --- destroying the backend mid-batch -----------------------------------
    // saveAll hands a worker thread a pointer to the Scrubber. Closing the app
    // while a batch runs must not leave that worker signalling into freed
    // memory, so the destructor stops the batch and waits for it. Here that is
    // observable: once the object is gone, no further file may appear.
    {
        QDir outDir(dir.filePath(QStringLiteral("batch")));
        QDir().mkpath(outDir.absolutePath());

        // Enough work that the batch is certainly still running when the object
        // is destroyed: each of these is decoded, denoised and re-encoded.
        QImage noise(700, 700, QImage::Format_RGB32);
        for (int y = 0; y < noise.height(); ++y) {
            for (int x = 0; x < noise.width(); ++x) {
                noise.setPixel(x, y, static_cast<uint>((x * 2654435761u) ^ (y * 40503u)));
            }
        }
        const QString source = dir.filePath(QStringLiteral("noise.png"));
        noise.save(source, "PNG");

        QVariantList jobs;
        for (int i = 0; i < 8; ++i) {
            QVariantMap job;
            job[QStringLiteral("src")] = source;
            job[QStringLiteral("dest")] = outDir.filePath(QStringLiteral("out-%1.jpg").arg(i));
            jobs << job;
        }

        Scrubber *doomed = new Scrubber;
        doomed->saveAll(jobs, false, false, true, 2);  // true = the slow wash
        // Let the worker get properly under way first, so this destroys the
        // object in the middle of a file rather than before it started. That
        // is the window the guard exists for.
        QThread::msleep(600);

        QElapsedTimer timer;
        timer.start();
        delete doomed;
        const qint64 waited = timer.elapsed();

        const int afterDelete = outDir.entryList(QDir::Files).count();
        QThread::msleep(400);
        const int later = outDir.entryList(QDir::Files).count();

        check(later == afterDelete,
              QStringLiteral("no file appears after the backend is destroyed"));
        check(waited < 15000,
              QStringLiteral("destroying it does not wait for the whole batch"));
        // Whether the abort actually cut the batch short depends on how fast
        // this machine is, so it is reported rather than asserted. Asserting it
        // would make the test fail on a quick machine for no good reason.
        QTextStream(stdout)
            << "     (" << afterDelete << " of " << jobs.size()
            << " written before it was abandoned, destructor waited " << waited << " ms)\n";
    }

    QTextStream(stdout) << (failures == 0 ? "\nall checks passed\n"
                                          : QStringLiteral("\n%1 check(s) failed\n").arg(failures));
    return failures == 0 ? 0 : 1;
}
