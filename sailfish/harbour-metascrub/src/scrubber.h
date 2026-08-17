#ifndef SCRUBBER_H
#define SCRUBBER_H

#include <QObject>
#include <QString>
#include <QVariantMap>

/*
 * The native backend the QML talks to.
 *
 * It reads a file, calls the metascrub Rust core over its C ABI (see
 * crates/metascrub-ffi/include/metascrub.h), and hands QML a plain QVariantMap it
 * can render — the same assurance/removed/warnings contract the CLI, the Android
 * app and the desktop GUI all use. No file bytes ever leave the device, and this
 * class logs nothing.
 */
class Scrubber : public QObject
{
    Q_OBJECT
public:
    explicit Scrubber(QObject *parent = nullptr);

    /*
     * Inspect a file and return its report as a map:
     *   ok            bool     - false when the file could not even be read
     *   error         string   - present when ok is false
     *   assurance     string   - "complete" | "best_effort" | "none"
     *   format        string
     *   foundLocation bool     - a GPS/location trace was found
     *   writable      bool     - worth saving (complete or best_effort)
     *   removedKinds  list     - de-duplicated category names
     *   removedCount  int
     *   warnings      list
     *   retained      list     - [{what, reveals}] knowingly left in the file,
     *                            with what each would tell someone examining it
     */
    Q_INVOKABLE QVariantMap inspect(const QString &path,
                                    bool keepColour, bool keepOrientation);

    /*
     * Clean srcPath and write the result to destPath. When fingerprint is true and
     * the file is a photo pixelwash can decode, the pixels are washed first, then
     * sanitized so no metadata rides the re-encoded JPEG. Returns an empty string
     * on success, or a human-readable error.
     */
    Q_INVOKABLE QString save(const QString &srcPath, const QString &destPath,
                             bool keepColour, bool keepOrientation,
                             bool fingerprint, int strength);

    /* Whether pixelwash can decode this file, so the fingerprint option applies. */
    Q_INVOKABLE bool isWashable(const QString &path) const;
};

#endif // SCRUBBER_H
