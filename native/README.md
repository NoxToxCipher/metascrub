# native/ — the Qt backend both Qt phones share

`Scrubber` is the class QML talks to on **Sailfish OS** and on **Ubuntu Touch**.
It reads a file, calls the Rust core over its C ABI
(`crates/metascrub-ffi/include/metascrub.h`), and hands QML a plain
`QVariantMap` with the same `assurance` / `removed` / `retained` contract the
CLI, the Android app and the desktop GUI all use.

It lives here rather than inside either app because of one method. Before
`save()` writes anything it re-inspects the bytes it is about to write, and
refuses when the assurance is `none`, so a file the core could not take apart is
never written out as a "cleaned copy". That guard is the difference between an
honest tool and a reassuring one, and two copies of it would eventually be one
guard and one bug.

It depends on Qt Core only (`QObject`, `QFile`, `QJsonDocument`), never on
Silica, Lomiri or any shell, so a third Qt platform costs a `main.cpp` and some
QML.

Who compiles it:

| Front end | How |
|---|---|
| Sailfish | `sailfish/harbour-metascrub/harbour-metascrub.pro` lists `../../native/scrubber.cpp` |
| Ubuntu Touch | `ubuntu-touch/metascrub/CMakeLists.txt` adds `${METASCRUB_NATIVE_DIR}/scrubber.cpp` |

`ubuntu-touch/metascrub/tests/scrubber_smoke.cpp` exercises this file end to end
through the real Rust library, and runs in CI on amd64.
