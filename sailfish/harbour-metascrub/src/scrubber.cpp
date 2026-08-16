#include "scrubber.h"
#include "metascrub.h"

#include <QByteArray>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>
#include <cstring>

namespace {

// The whole file is held in memory to clean it, so cap it — the same bound the
// Android app uses. A phone should refuse a huge file rather than be killed.
const qint64 MAX_BYTES = 100LL * 1024 * 1024;

bool readAll(const QString &path, QByteArray &out, QString &err)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) {
        err = QStringLiteral("Could not open the file.");
        return false;
    }
    if (f.size() > MAX_BYTES) {
        err = QStringLiteral("Larger than 100 MB — not supported yet.");
        return false;
    }
    out = f.readAll();
    return true;
}

// Best-effort wipe of the in-memory copy of the file being cleaned. Qt may have
// made other copies we cannot reach, but clearing the one we hold is still worth
// doing — the file is the private thing.
void wipe(QByteArray &b)
{
    if (!b.isEmpty()) {
        std::memset(b.data(), 0, static_cast<size_t>(b.size()));
    }
}

const uint8_t *bytesOf(const QByteArray &b)
{
    return reinterpret_cast<const uint8_t *>(b.constData());
}

} // namespace

Scrubber::Scrubber(QObject *parent) : QObject(parent) {}

QVariantMap Scrubber::inspect(const QString &path, bool keepColour, bool keepOrientation)
{
    QVariantMap m;
    QByteArray bytes;
    QString err;
    if (!readAll(path, bytes, err)) {
        m[QStringLiteral("ok")] = false;
        m[QStringLiteral("error")] = err;
        return m;
    }

    char *json = ms_report_json(bytesOf(bytes), static_cast<size_t>(bytes.size()),
                                keepColour, keepOrientation);
    const QByteArray raw = json ? QByteArray(json) : QByteArray("{\"error\":\"no report\"}");
    if (json) {
        ms_string_free(json);
    }
    wipe(bytes);

    const QJsonObject o = QJsonDocument::fromJson(raw).object();
    if (o.isEmpty() || o.contains(QStringLiteral("error"))) {
        m[QStringLiteral("ok")] = false;
        m[QStringLiteral("error")] = o.value(QStringLiteral("error"))
                                         .toString(QStringLiteral("Could not read the report."));
        return m;
    }

    const QString assurance = o.value(QStringLiteral("assurance")).toString();
    m[QStringLiteral("ok")] = true;
    m[QStringLiteral("assurance")] = assurance;
    m[QStringLiteral("format")] = o.value(QStringLiteral("format")).toString();
    m[QStringLiteral("foundLocation")] = o.value(QStringLiteral("found_location")).toBool();
    m[QStringLiteral("writable")] =
        (assurance == QLatin1String("complete") || assurance == QLatin1String("best_effort"));

    QStringList kinds;
    const QJsonArray removed = o.value(QStringLiteral("removed")).toArray();
    for (const QJsonValue &v : removed) {
        const QString k = v.toObject().value(QStringLiteral("kind")).toString();
        if (!k.isEmpty() && !kinds.contains(k)) {
            kinds << k;
        }
    }
    m[QStringLiteral("removedKinds")] = kinds;
    m[QStringLiteral("removedCount")] = removed.size();

    QStringList warnings;
    const QJsonArray warr = o.value(QStringLiteral("warnings")).toArray();
    for (const QJsonValue &v : warr) {
        warnings << v.toString();
    }
    m[QStringLiteral("warnings")] = warnings;
    return m;
}

QString Scrubber::save(const QString &srcPath, const QString &destPath, bool keepColour,
                       bool keepOrientation, bool fingerprint, int strength)
{
    QByteArray bytes;
    QString err;
    if (!readAll(srcPath, bytes, err)) {
        return err;
    }

    const uint8_t *in = bytesOf(bytes);
    const size_t len = static_cast<size_t>(bytes.size());

    MsBuffer out{nullptr, 0};
    if (fingerprint && isWashable(srcPath)) {
        MsBuffer washed = ms_reduce_fingerprint(in, len, strength);
        if (!washed.data) {
            wipe(bytes);
            return QStringLiteral("Could not reduce the fingerprint.");
        }
        out = ms_sanitize(washed.data, washed.len, keepColour, keepOrientation);
        ms_buffer_free(washed);
    } else {
        // Re-inspect the bytes we are about to write (mirrors the Android fix): a
        // format the core cannot take apart comes back unchanged, and must never
        // be written out as a "cleaned copy".
        char *json = ms_report_json(in, len, keepColour, keepOrientation);
        const QByteArray raw = json ? QByteArray(json) : QByteArray();
        if (json) {
            ms_string_free(json);
        }
        const QString a =
            QJsonDocument::fromJson(raw).object().value(QStringLiteral("assurance")).toString();
        if (!(a == QLatin1String("complete") || a == QLatin1String("best_effort"))) {
            wipe(bytes);
            return QStringLiteral("This format could not be cleaned, so nothing was saved.");
        }
        out = ms_sanitize(in, len, keepColour, keepOrientation);
    }
    wipe(bytes);

    if (!out.data) {
        return QStringLiteral("Cleaning failed.");
    }
    const qint64 outLen = static_cast<qint64>(out.len);

    QFile f(destPath);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        ms_buffer_free(out);
        return QStringLiteral("Could not write the destination file.");
    }
    const qint64 written = f.write(reinterpret_cast<const char *>(out.data), outLen);
    f.close();
    ms_buffer_free(out);

    if (written != outLen) {
        return QStringLiteral("The file was not fully written.");
    }
    return QString(); // success
}

bool Scrubber::isWashable(const QString &path) const
{
    const QString s = path.toLower();
    // A cheap gate for the option; the wash itself validates the actual bytes and
    // fails gracefully if the file is not a decodable JPEG/PNG/WebP.
    return s.endsWith(QLatin1String(".jpg")) || s.endsWith(QLatin1String(".jpeg"))
           || s.endsWith(QLatin1String(".png")) || s.endsWith(QLatin1String(".webp"));
}
