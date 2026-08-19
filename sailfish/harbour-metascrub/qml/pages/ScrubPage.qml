import QtQuick 2.0
import Sailfish.Silica 1.0
import Sailfish.Pickers 1.0
import org.crake.metascrub 1.0

/*
 * The main screen: choose files, set the options, scrub, then save cleaned
 * copies. Mirrors the Android app's flow — files are only touched when the user
 * asks, and the assurance of each result is shown honestly, never a green tick
 * that was not earned. Every badge pairs colour with a word, so colour is never
 * the only signal.
 */
Page {
    id: page
    allowedOrientations: Orientation.All

    Scrubber { id: scrubber }

    // One row per chosen file. `assurance` is empty until scrubbed.
    ListModel { id: queue }

    property bool scrubbed: false
    property string saveMessage: ""

    // Native pickers, pushed on demand. Multi-file selection is a Dialog
    // (MultiFilePickerDialog): its chosen files arrive in the selectedContent
    // model when the dialog is accepted, each row carrying filePath/fileName.
    Component {
        id: filePickerPage
        MultiFilePickerDialog {
            title: qsTr("Choose files")
            onAccepted: {
                var urls = []
                for (var i = 0; i < selectedContent.count; ++i)
                    urls.push(selectedContent.get(i).filePath)
                addFiles(urls)
            }
        }
    }
    Component {
        id: folderPickerPage
        FolderPickerPage {
            title: qsTr("Save cleaned copies to")
            onSelectedPathChanged: saveAll(selectedPath.toString().replace("file://", ""))
        }
    }

    function strengthIndex() { return strengthCombo.currentIndex } // 0/1/2 = gentle/balanced/thorough

    function addFiles(urls) {
        for (var i = 0; i < urls.length; ++i) {
            var path = urls[i].toString().replace("file://", "")
            var dup = false
            for (var j = 0; j < queue.count; ++j) {
                if (queue.get(j).path === path) { dup = true; break }
            }
            if (!dup) {
                queue.append({ "path": path,
                               "name": path.substring(path.lastIndexOf('/') + 1),
                               "assurance": "", "note": "", "writable": false,
                               "foundLocation": false, "removed": "", "retained": "" })
            }
        }
        scrubbed = false
    }

    function scrubAll() {
        for (var i = 0; i < queue.count; ++i) {
            var it = queue.get(i)
            var r = scrubber.inspect(it.path, optKeepColour.checked, optKeepOrientation.checked)
            if (!r.ok) {
                queue.setProperty(i, "assurance", "none")
                queue.setProperty(i, "note", r.error)
                queue.setProperty(i, "writable", false)
                queue.setProperty(i, "retained", "")
                continue
            }
            queue.setProperty(i, "assurance", r.assurance)
            queue.setProperty(i, "writable", r.writable)
            queue.setProperty(i, "foundLocation", r.foundLocation)
            queue.setProperty(i, "removed", r.removedKinds.join(", "))
            // What was knowingly kept, and what it reveals. Each item on its own
            // line so a kept colour profile (or raw residue) is never silent.
            var keptLines = []
            var kept = r.retained
            for (var k = 0; k < kept.length; ++k) {
                var line = "• " + kept[k].what
                if (kept[k].reveals !== "")
                    line += ": " + kept[k].reveals
                keptLines.push(line)
            }
            queue.setProperty(i, "retained", keptLines.join("\n"))
        }
        scrubbed = true
    }

    function badgeColour(a) {
        if (a === "complete") return Theme.highlightColor          // earned green/teal
        if (a === "best_effort") return "#d08a1e"                  // amber: partial
        return Theme.errorColor                                    // none: not cleaned
    }
    function badgeText(a) {
        if (a === "complete") return qsTr("COMPLETE")
        if (a === "best_effort") return qsTr("BEST EFFORT")
        return qsTr("NOT CLEANED")
    }

    function randomStem() {
        return "cleaned-" + Math.floor(Math.random() * 1e9).toString(36)
                          + Math.floor(Math.random() * 1e9).toString(36)
    }
    function extOf(path, washed) {
        if (washed) return ".jpg"            // a washed photo is re-encoded to JPEG
        var dot = path.lastIndexOf('.')
        return dot >= 0 ? path.substring(dot) : ""
    }
    function saveAll(folder) {
        var saved = 0, failed = 0
        for (var i = 0; i < queue.count; ++i) {
            var it = queue.get(i)
            if (!it.writable) continue
            var willWash = optFingerprint.checked && scrubber.isWashable(it.path)
            var base = optRandom.checked
                     ? randomStem()
                     : it.name.replace(/\.[^/.]+$/, "") + "-cleaned"
            var dest = folder + "/" + base + extOf(it.path, willWash)
            var err = scrubber.save(it.path, dest, optKeepColour.checked,
                                    optKeepOrientation.checked, optFingerprint.checked,
                                    strengthIndex())
            if (err === "") saved++
            else failed++
        }
        saveMessage = failed === 0
            ? qsTr("Saved %n cleaned copy(s). Your originals were not changed.", "", saved)
            : qsTr("Saved %1, could not save %2.").arg(saved).arg(failed)
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: column.height

        PullDownMenu {
            MenuItem {
                text: qsTr("About")
                onClicked: pageStack.push(Qt.resolvedUrl("AboutPage.qml"))
            }
            MenuItem {
                text: qsTr("Handbook")
                onClicked: pageStack.push(Qt.resolvedUrl("HandbookPage.qml"))
            }
        }

        Column {
            id: column
            width: page.width
            spacing: Theme.paddingMedium

            PageHeader { title: qsTr("metascrub") }

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                source: Qt.resolvedUrl("../images/sandpiper.svg")
                sourceSize.width: Theme.iconSizeLarge
                sourceSize.height: Theme.iconSizeLarge
                width: Theme.iconSizeLarge
                height: Theme.iconSizeLarge
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.WordWrap
                color: Theme.secondaryColor
                font.pixelSize: Theme.fontSizeSmall
                text: qsTr("Removes what a file says about you. Everything happens on this device. "
                         + "Nothing is uploaded, and your files are only touched when you press Scrub.")
            }

            TextSwitch { id: optRandom; text: qsTr("Give cleaned files random names"); checked: true }
            TextSwitch { id: optKeepColour; text: qsTr("Keep colour profile") }
            TextSwitch { id: optKeepOrientation; text: qsTr("Keep image orientation") }
            TextSwitch { id: optFingerprint; text: qsTr("Reduce camera fingerprint (photos)") }

            ComboBox {
                id: strengthCombo
                visible: optFingerprint.checked
                label: qsTr("Strength")
                currentIndex: 1
                menu: ContextMenu {
                    MenuItem { text: qsTr("Gentle") }
                    MenuItem { text: qsTr("Balanced") }
                    MenuItem { text: qsTr("Thorough") }
                }
            }

            SectionHeader { text: scrubbed ? qsTr("Results") : qsTr("Ready to clean") }

            Repeater {
                model: queue
                delegate: ListItem {
                    id: row
                    // Grows when there is retained data to disclose, so the amber
                    // line is never clipped; a plain row keeps its compact height.
                    contentHeight: Math.max(Theme.itemSizeMedium, rowCol.height + Theme.paddingMedium)
                    menu: ContextMenu {
                        MenuItem { text: qsTr("Remove"); onClicked: queue.remove(index) }
                    }

                    Column {
                        id: rowCol
                        anchors.top: parent.top
                        anchors.topMargin: Theme.paddingSmall
                        x: Theme.horizontalPageMargin
                        width: parent.width - 2 * Theme.horizontalPageMargin
                        spacing: Theme.paddingSmall

                        Label {
                            width: parent.width
                            truncationMode: TruncationMode.Fade
                            text: model.name
                            color: row.highlighted ? Theme.highlightColor : Theme.primaryColor
                        }

                        Row {
                            spacing: Theme.paddingSmall
                            visible: model.assurance !== ""

                            // The badge: colour AND word, never colour alone.
                            Rectangle {
                                width: badge.width + 2 * Theme.paddingSmall
                                height: badge.height + Theme.paddingSmall
                                radius: Theme.paddingSmall / 2
                                color: badgeColour(model.assurance)
                                Label {
                                    id: badge
                                    anchors.centerIn: parent
                                    text: badgeText(model.assurance)
                                    color: "black"
                                    font.pixelSize: Theme.fontSizeExtraSmall
                                }
                            }
                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                text: model.foundLocation ? qsTr("location found") : model.removed
                                color: Theme.secondaryColor
                                font.pixelSize: Theme.fontSizeExtraSmall
                                truncationMode: TruncationMode.Fade
                                width: page.width / 2
                            }
                        }

                        // Knowingly kept, and what it reveals. Amber, like the
                        // best-effort badge: a clean that leaves something
                        // identifying but says nothing is worse than one that
                        // spells it out.
                        Label {
                            visible: model.retained !== ""
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("Still in the file:") + "\n" + model.retained
                            color: "#d08a1e"
                            font.pixelSize: Theme.fontSizeExtraSmall
                        }
                    }
                }
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                text: queue.count === 0 ? qsTr("Choose files") : qsTr("Add more")
                onClicked: pageStack.push(filePickerPage)
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: queue.count > 0
                text: scrubbed ? qsTr("Save cleaned copies") : qsTr("Scrub")
                onClicked: {
                    if (!scrubbed) scrubAll()
                    else pageStack.push(folderPickerPage)
                }
            }

            Label {
                visible: saveMessage !== ""
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.WordWrap
                color: Theme.highlightColor
                font.pixelSize: Theme.fontSizeSmall
                text: saveMessage
            }

            Item { width: 1; height: Theme.paddingLarge }
        }

        VerticalScrollDecorator {}
    }
}
