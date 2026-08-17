import QtQuick 2.9
import Lomiri.Components 1.3
import Lomiri.Content 1.3
import org.crake.metascrub 1.0
import "."

/*
 * The application window, and the one place that holds state.
 *
 * There are three ways in, and the app has to behave sensibly in all of them:
 *
 *   1. Opened from the launcher. The user picks files through the Content Hub.
 *   2. Another app hands files over ("share to metascrub"). They arrive already
 *      copied into this app's own storage, and the interface says plainly that
 *      nothing left the phone.
 *   3. Another app asks metascrub *for* a file — a messenger picking a photo to
 *      attach. Then metascrub cleans first and hands back the clean copy, which
 *      is the flow this whole app exists for.
 *
 * The queue, the options and the results live here so the pages stay
 * presentational, and so a Content Hub request that arrives while a page is open
 * still lands somewhere sensible.
 */
MainView {
    id: root
    objectName: "mainView"
    applicationName: "metascrub.noxtoxcipher"
    automaticOrientation: true
    anchorToKeyboard: true

    // The dark ground the whole family uses. If a future image renames the
    // theme, the app still works: every colour that carries meaning is set
    // explicitly in Style.qml, and only the stock controls would look wrong.
    theme.name: "Lomiri.Components.Themes.SuruDark"

    width: units.gu(45)
    height: units.gu(75)

    // The native backends. Neither logs anything, and neither opens a socket.
    Scrubber { id: scrubberBackend }
    Workspace { id: workspaceBackend }
    property alias scrubber: scrubberBackend
    property alias workspace: workspaceBackend

    // One row per file. `assurance` stays empty until it has been scrubbed, so
    // nothing claims a result before the work was done.
    ListModel { id: queueModel }
    property alias queue: queueModel

    property bool scrubbed: false
    property bool handedIn: false          // files arrived from another app
    property var pendingExport: null       // another app is waiting for a file
    property var lastExport: null          // keeps a charged transfer alive
    property string flash: ""              // one honest line under the buttons
    property int lastFailed: 0

    // Options. Random names default on: a file name is metadata too.
    property bool optRandomNames: true
    property bool optKeepColour: false
    property bool optKeepOrientation: false
    property bool optFingerprint: false
    property int optStrength: 1            // 0 gentle, 1 balanced, 2 thorough

    PageStack { id: pageStack }

    ScrubPage {
        id: scrubPage
        visible: false
        app: root
        stack: pageStack
    }

    // Pushed from here rather than from the PageStack's own onCompleted: the
    // stack is declared first, so at that point the page does not exist yet.
    Component.onCompleted: pageStack.push(scrubPage)

    Component { id: contentItemComponent; ContentItem {} }

    /*
     * Requests from other apps. An import or a share arrives already charged, so
     * the items are readable copies sitting in this app's cache. An export
     * request is the opposite: someone is waiting for a file from us, and we
     * hold the transfer until there is a cleaned one to give.
     */
    Connections {
        target: ContentHub
        onImportRequested: root.acceptHandover(transfer)
        onShareRequested: root.acceptHandover(transfer)
        onExportRequested: {
            root.pendingExport = transfer
            root.flash = ""
        }
    }

    function acceptHandover(transfer) {
        if (!transfer || !transfer.items) {
            return
        }
        root.handedIn = true
        addItems(transfer.items)
    }

    function addItems(items) {
        var urls = []
        for (var i = 0; i < items.length; ++i) {
            urls.push(items[i].url.toString())
        }
        addUrls(urls)
    }

    function addUrls(urls) {
        for (var i = 0; i < urls.length; ++i) {
            var path = workspaceBackend.pathFromUrl(urls[i])
            if (path === "") {
                continue
            }
            var duplicate = false
            for (var j = 0; j < queueModel.count; ++j) {
                if (queueModel.get(j).path === path) {
                    duplicate = true
                    break
                }
            }
            if (!duplicate) {
                queueModel.append({ "path": path,
                                    "name": workspaceBackend.baseName(path),
                                    "assurance": "",
                                    "note": "",
                                    "writable": false,
                                    "foundLocation": false,
                                    "removed": "",
                                    "retained": "" })
            }
        }
        // Anything new invalidates the previous verdicts, so nothing on screen
        // is ever a result for a different set of files.
        scrubbed = false
        flash = ""
    }

    /* The honest one-liner for each verdict. Same words as the Android app. */
    function noteFor(assurance) {
        if (assurance === "complete") {
            return i18n.tr("Rebuilt from known-safe parts. Nothing unknown was kept.")
        }
        if (assurance === "best_effort") {
            return i18n.tr("Cleaned in place. Some structure may remain.")
        }
        return i18n.tr("Could not take this format apart, so assume it still carries everything.")
    }

    function scrubAll() {
        for (var i = 0; i < queueModel.count; ++i) {
            var item = queueModel.get(i)
            var report = scrubberBackend.inspect(item.path, optKeepColour, optKeepOrientation)

            if (!report.ok) {
                queueModel.setProperty(i, "assurance", "none")
                queueModel.setProperty(i, "note", report.error)
                queueModel.setProperty(i, "writable", false)
                queueModel.setProperty(i, "foundLocation", false)
                queueModel.setProperty(i, "removed", "")
                queueModel.setProperty(i, "retained", "")
                continue
            }

            queueModel.setProperty(i, "assurance", report.assurance)
            queueModel.setProperty(i, "note", noteFor(report.assurance))
            queueModel.setProperty(i, "writable", report.writable)
            queueModel.setProperty(i, "foundLocation", report.foundLocation)
            // "Already clean" is only true of a file the core could actually take
            // apart. Saying it about a format it could not read would be the
            // reassuring lie this whole tool exists to avoid: nothing was found
            // there because nothing could be looked for. Same guard the Android
            // app uses.
            queueModel.setProperty(i, "removed",
                report.removedCount > 0
                    ? i18n.tr("Removed: %1").arg(report.removedKinds.join(", "))
                    : (report.writable ? i18n.tr("Already clean. Nothing to remove.") : ""))

            // What was knowingly kept, and what each thing would tell someone
            // looking. A clean that leaves something identifying but says
            // nothing is worse than one that spells it out.
            var kept = []
            for (var k = 0; k < report.retained.length; ++k) {
                var line = "• " + report.retained[k].what
                if (report.retained[k].reveals !== "") {
                    line += ": " + report.retained[k].reveals
                }
                kept.push(line)
            }
            queueModel.setProperty(i, "retained", kept.join("\n"))
        }
        scrubbed = true
        flash = ""
    }

    /*
     * Write the cleaned copies into the app's own storage and return their
     * paths. Nothing is handed anywhere yet — that is the user's next choice.
     * save() re-checks the bytes before writing, so a file that cannot be
     * cleaned is never written out as if it had been.
     */
    function saveCleaned() {
        var written = []
        var failed = 0
        for (var i = 0; i < queueModel.count; ++i) {
            var item = queueModel.get(i)
            if (!item.writable) {
                continue
            }
            var willWash = optFingerprint && scrubberBackend.isWashable(item.path)
            var dest = workspaceBackend.destinationFor(item.path, optRandomNames, willWash)
            var error = scrubberBackend.save(item.path, dest, optKeepColour, optKeepOrientation,
                                             optFingerprint, optStrength)
            if (error === "") {
                written.push(dest)
            } else {
                failed += 1
            }
        }
        lastFailed = failed
        return written
    }

    /*
     * A file that could not be written is never allowed to pass in silence. It
     * would otherwise look exactly like success from the outside, which is the
     * one thing this tool must not do.
     */
    function failureTail() {
        if (lastFailed === 0) {
            return ""
        }
        return " " + i18n.tr("%1 file could not be saved.",
                             "%1 files could not be saved.", lastFailed).arg(lastFailed)
    }

    function savedMessage(count) {
        return i18n.tr("Saved %1 cleaned copy. Your original was not changed.",
                       "Saved %1 cleaned copies. Your originals were not changed.",
                       count).arg(count) + failureTail()
    }

    /* Give the waiting app the cleaned copies instead of the originals. */
    function handBack(paths) {
        if (!pendingExport) {
            return
        }
        var items = []
        for (var i = 0; i < paths.length; ++i) {
            items.push(contentItemComponent.createObject(root,
                { "url": workspaceBackend.urlFromPath(paths[i]) }))
        }
        pendingExport.items = items
        pendingExport.state = ContentTransfer.Charged
        pendingExport = null
        flash = i18n.tr("Handed back the cleaned copy. Your original was not sent.")
                + failureTail()
    }

    function clearWorkingFiles() {
        var removed = workspaceBackend.clearWorkingFiles()
        queueModel.clear()
        scrubbed = false
        handedIn = false
        flash = removed === 0
            ? i18n.tr("Nothing was left to clear.")
            : i18n.tr("Cleared %1 working file from this app's storage.",
                      "Cleared %1 working files from this app's storage.",
                      removed).arg(removed)
    }
}
