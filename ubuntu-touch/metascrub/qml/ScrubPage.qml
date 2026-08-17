import QtQuick 2.9
import Lomiri.Components 1.3
import "."

/*
 * The main screen: the files waiting, the options, and one honest result per
 * file. Nothing is read until Scrub is pressed, nothing is written until the
 * cleaned copies are saved, and nothing leaves the phone at any point.
 *
 * The layout is arranged around the fact that a phone screen is short. The
 * ordinary path is choose, scrub, save, and none of it should be below the
 * fold: so the options fold away until asked for, and the logo steps aside as
 * soon as there are files to look at. An early version put four full-height
 * option rows between the top of the screen and the first result, which meant
 * scrubbing something and then having to go looking for the answer.
 *
 * The result rows follow the same rule as every other metascrub interface: a
 * badge word, a colour, and a sentence saying what the verdict actually means.
 * The colour is never carrying the meaning on its own.
 */
Page {
    id: page

    property var app
    property var stack

    /* Folded away by default; the defaults are the ones most people want. */
    property bool optionsOpen: false

    header: PageHeader {
        LayoutMirroring.enabled: systemRightToLeft
        LayoutMirroring.childrenInherit: true
        title: i18n.tr("metascrub")
        trailingActionBar.actions: [
            Action {
                iconName: "help"
                text: i18n.tr("Handbook")
                onTriggered: stack.push(Qt.resolvedUrl("HandbookPage.qml"), { "stack": stack })
            },
            Action {
                iconName: "info"
                text: i18n.tr("About")
                onTriggered: stack.push(Qt.resolvedUrl("AboutPage.qml"), { "stack": stack })
            },
            Action {
                iconName: "reset"
                text: i18n.tr("Start over")
                visible: app.queue.count > 0
                onTriggered: {
                    app.queue.clear()
                    app.scrubbed = false
                    app.handedIn = false
                    app.flash = ""
                }
            },
            Action {
                iconName: "delete"
                text: i18n.tr("Clear working files")
                onTriggered: app.clearWorkingFiles()
            }
        ]
    }

    Rectangle {
        anchors.fill: parent
        color: Style.bg
        z: -1
    }

    function badgeColour(assurance) {
        if (assurance === "complete") {
            return Style.ok
        }
        if (assurance === "best_effort") {
            return Style.warn
        }
        return Style.danger
    }

    function badgeText(assurance) {
        if (assurance === "complete") {
            return i18n.tr("COMPLETE")
        }
        if (assurance === "best_effort") {
            return i18n.tr("BEST EFFORT")
        }
        return i18n.tr("NOT CLEANED")
    }

    /* How many of the scrubbed files are actually worth writing out. */
    function writableCount() {
        var n = 0
        for (var i = 0; i < app.queue.count; ++i) {
            if (app.queue.get(i).writable) {
                n += 1
            }
        }
        return n
    }

    function primaryText() {
        if (app.queue.count === 0) {
            return i18n.tr("Choose files")
        }
        if (app.saving) {
            return i18n.tr("Cleaning %1 of %2…").arg(app.saveDone + 1).arg(app.saveTotal)
        }
        if (!app.scrubbed) {
            return i18n.tr("Scrub %1 file", "Scrub %1 files", app.queue.count).arg(app.queue.count)
        }
        var n = writableCount()
        // Both forms carry %1 on purpose: the singular is still handed a number,
        // and a form without the placeholder would drop it with a warning.
        if (app.pendingExport) {
            return i18n.tr("Hand back %1 cleaned copy", "Hand back %1 cleaned copies", n).arg(n)
        }
        return i18n.tr("Save %1 cleaned copy", "Save %1 cleaned copies", n).arg(n)
    }

    function chooseFiles() {
        stack.push(Qt.resolvedUrl("ImportPage.qml"), { "app": app, "stack": stack })
    }

    function runPrimary() {
        if (app.saving) {
            return
        }
        if (app.queue.count === 0) {
            chooseFiles()
            return
        }
        if (!app.scrubbed) {
            app.scrubAll()
            return
        }
        // The writing runs on a worker thread; where the cleaned copies go next
        // is decided when it reports back.
        app.startSave()
    }

    Flickable {
        id: flickable
        anchors {
            top: page.header.bottom
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
        clip: true
        // Read the other way round in Arabic, Farsi, Sorani and Kurmanji. This
        // sits on the page content rather than on MainView: mirroring the whole
        // window makes PageStack size every page to twice the window width and
        // park it at -width, which puts the middle of the page off-screen.
        LayoutMirroring.enabled: systemRightToLeft
        LayoutMirroring.childrenInherit: true
        contentWidth: width
        contentHeight: column.height + units.gu(4)

        Column {
            id: column
            // Capped and centred, because an Ubuntu Touch app also runs on a
            // monitor. Left to fill the width, a sentence ran the whole way
            // across a desktop screen and the buttons became a metre wide.
            width: Math.min(parent.width, units.gu(50))
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: units.gu(2)
            topPadding: units.gu(2)

            // --- the mark, while there is nothing else to look at ------------
            Image {
                visible: app.queue.count === 0
                anchors.horizontalCenter: parent.horizontalCenter
                source: Qt.resolvedUrl("images/sandpiper.svg")
                sourceSize.width: units.gu(6)
                sourceSize.height: units.gu(6)
                width: units.gu(6)
                height: units.gu(6)
            }

            Label {
                visible: app.queue.count === 0
                anchors.horizontalCenter: parent.horizontalCenter
                text: i18n.tr("removes what a file says about you")
                color: Style.muted
                fontSize: "small"
            }

            // --- how we got here ---------------------------------------------
            // A handover is the moment a user most needs to be told that
            // "share" did not mean "send". Said once, at the decision point.
            Card {
                visible: app.handedIn
                accent: Style.teal
                text: i18n.tr("These files were handed to metascrub on this phone. "
                            + "Nothing was sent anywhere.")
            }

            Card {
                visible: app.pendingExport !== null
                accent: Style.teal
                text: i18n.tr("Another app is waiting for a file. Clean it here and metascrub "
                            + "will hand back the cleaned copy instead of your original.")
            }

            Card {
                visible: app.queue.count === 0 && !app.handedIn
                accent: Style.stroke
                text: i18n.tr("Add photos, PDFs or documents to clean them.\n\n"
                            + "Everything happens on this phone. Nothing is uploaded, and your "
                            + "files are only touched when you press Scrub.")
            }

            // --- the queue and its results -----------------------------------
            Label {
                visible: app.queue.count > 0
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                text: app.scrubbed ? i18n.tr("Results") : i18n.tr("Ready to scrub")
                color: Style.teal
                fontSize: "medium"
            }

            Repeater {
                model: app.queue

                delegate: Rectangle {
                    width: column.width - units.gu(4)
                    anchors.horizontalCenter: parent.horizontalCenter
                    height: rowColumn.height + units.gu(3)
                    radius: units.gu(0.5)
                    color: Style.surface
                    border.width: units.dp(1)
                    border.color: Style.stroke

                    Column {
                        id: rowColumn
                        y: units.gu(1.5)
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: parent.width - units.gu(3)
                        spacing: units.gu(0.5)

                        // The name, and a way to take it off the list that does
                        // not shout as loudly as the file itself.
                        Item {
                            width: parent.width
                            height: Math.max(nameLabel.height, removeButton.height)

                            Label {
                                id: nameLabel
                                anchors {
                                    left: parent.left
                                    right: removeButton.left
                                    rightMargin: units.gu(1)
                                    verticalCenter: parent.verticalCenter
                                }
                                elide: Text.ElideMiddle
                                text: model.name
                                color: Style.text
                            }

                            AbstractButton {
                                id: removeButton
                                anchors {
                                    right: parent.right
                                    verticalCenter: parent.verticalCenter
                                }
                                width: units.gu(4)
                                height: units.gu(4)
                                activeFocusOnPress: false
                                Accessible.name: i18n.tr("Remove")
                                onClicked: {
                                    app.queue.remove(index)
                                    app.scrubbed = false
                                }

                                Icon {
                                    anchors.centerIn: parent
                                    width: units.gu(2)
                                    height: units.gu(2)
                                    name: "close"
                                    color: removeButton.pressed ? Style.text : Style.muted
                                }
                            }
                        }

                        Row {
                            visible: model.assurance !== ""
                            spacing: units.gu(1)

                            Rectangle {
                                width: badgeLabel.width + units.gu(1.5)
                                height: badgeLabel.height + units.gu(0.75)
                                radius: units.dp(3)
                                color: page.badgeColour(model.assurance)
                                anchors.verticalCenter: parent.verticalCenter

                                Label {
                                    id: badgeLabel
                                    anchors.centerIn: parent
                                    text: page.badgeText(model.assurance)
                                    color: Style.bg
                                    fontSize: "x-small"
                                }
                            }

                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: model.foundLocation
                                text: i18n.tr("Recorded where it was taken.")
                                color: Style.danger
                                fontSize: "x-small"
                            }
                        }

                        Label {
                            visible: model.note !== ""
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: model.note
                            color: Style.muted
                            fontSize: "x-small"
                        }

                        Label {
                            visible: model.removed !== ""
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: model.removed
                            color: Style.muted
                            fontSize: "x-small"
                        }

                        Label {
                            visible: model.retained !== ""
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: i18n.tr("Still in the file") + "\n" + model.retained
                            color: Style.warn
                            fontSize: "x-small"
                        }
                    }
                }
            }

            // --- actions ------------------------------------------------------
            Button {
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: !app.saving
                color: Style.teal
                text: page.primaryText()
                onClicked: page.runPrimary()
            }

            Button {
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                visible: app.queue.count > 0
                // Left in the toolkit's own secondary style. Overriding the
                // fill to something darker made the label unreadable: Lomiri
                // derives the label colour from the fill, and both a
                // transparent and a near-black fill gave grey on grey. Teal
                // for the next step, stock grey for the alternative, is
                // hierarchy enough.
                text: i18n.tr("Add more")
                onClicked: page.chooseFiles()
            }

            Card {
                visible: app.flash !== ""
                accent: Style.teal
                text: app.flash
            }

            // --- options, folded away ------------------------------------------
            ListItem {
                divider.visible: false
                height: optionsHeader.height
                color: "transparent"
                onClicked: page.optionsOpen = !page.optionsOpen

                ListItemLayout {
                    id: optionsHeader
                    title.text: i18n.tr("Options")
                    title.color: Style.teal

                    Icon {
                        SlotsLayout.position: SlotsLayout.Trailing
                        width: units.gu(2)
                        height: units.gu(2)
                        name: page.optionsOpen ? "up" : "down"
                        color: Style.teal
                    }
                }
            }

            Column {
                id: optionsBlock
                visible: page.optionsOpen
                width: parent.width
                // Flush, like a settings list. Spacing between them left gaps
                // wide enough to look like something had failed to load.
                spacing: 0

                ListItem {
                    height: randomLayout.height
                    color: "transparent"
                    ListItemLayout {
                        id: randomLayout
                        title.text: i18n.tr("Give cleaned files random names")
                        title.color: Style.text
                        title.wrapMode: Text.WordWrap
                        title.maximumLineCount: 3
                        subtitle.text: i18n.tr("A file name is metadata too")
                        subtitle.color: Style.muted
                        subtitle.wrapMode: Text.WordWrap
                        subtitle.maximumLineCount: 3
                        Switch {
                            SlotsLayout.position: SlotsLayout.Trailing
                            checked: app.optRandomNames
                            onCheckedChanged: app.optRandomNames = checked
                        }
                    }
                }

                ListItem {
                    height: colourLayout.height
                    color: "transparent"
                    ListItemLayout {
                        id: colourLayout
                        title.text: i18n.tr("Keep colour profile")
                        title.color: Style.text
                        title.wrapMode: Text.WordWrap
                        title.maximumLineCount: 3
                        subtitle.text: i18n.tr("Keeps colours accurate, and is itself identifying")
                        subtitle.color: Style.muted
                        subtitle.wrapMode: Text.WordWrap
                        subtitle.maximumLineCount: 3
                        Switch {
                            SlotsLayout.position: SlotsLayout.Trailing
                            checked: app.optKeepColour
                            onCheckedChanged: app.optKeepColour = checked
                        }
                    }
                }

                ListItem {
                    height: orientLayout.height
                    color: "transparent"
                    ListItemLayout {
                        id: orientLayout
                        title.text: i18n.tr("Keep image orientation")
                        title.color: Style.text
                        title.wrapMode: Text.WordWrap
                        title.maximumLineCount: 3
                        subtitle.text: i18n.tr("Stops photos appearing rotated")
                        subtitle.color: Style.muted
                        subtitle.wrapMode: Text.WordWrap
                        subtitle.maximumLineCount: 3
                        Switch {
                            SlotsLayout.position: SlotsLayout.Trailing
                            checked: app.optKeepOrientation
                            onCheckedChanged: app.optKeepOrientation = checked
                        }
                    }
                }

                ListItem {
                    height: fingerLayout.height
                    color: "transparent"
                    ListItemLayout {
                        id: fingerLayout
                        title.text: i18n.tr("Reduce camera fingerprint (photos)")
                        title.color: Style.text
                        title.wrapMode: Text.WordWrap
                        title.maximumLineCount: 3
                        subtitle.text: i18n.tr("Reduces linkability. Does not remove it.")
                        subtitle.color: Style.muted
                        subtitle.wrapMode: Text.WordWrap
                        subtitle.maximumLineCount: 3
                        Switch {
                            SlotsLayout.position: SlotsLayout.Trailing
                            checked: app.optFingerprint
                            onCheckedChanged: app.optFingerprint = checked
                        }
                    }
                }
            }

            Card {
                visible: page.optionsOpen && app.optFingerprint
                accent: Style.warn
                text: i18n.tr("Softens the pixels to weaken the sensor fingerprint a "
                            + "camera leaves in every photo. This reduces how easily "
                            + "photos can be linked to one camera. It does not "
                            + "remove the fingerprint, and it makes the photo softer "
                            + "and a little smaller. Photos are saved as JPEG.")
            }

            OptionSelector {
                visible: page.optionsOpen && app.optFingerprint
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                text: i18n.tr("Strength")
                model: [i18n.tr("Gentle"), i18n.tr("Balanced"), i18n.tr("Thorough")]
                selectedIndex: app.optStrength
                onSelectedIndexChanged: app.optStrength = selectedIndex
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                fontSize: "x-small"
                color: Style.muted
                text: i18n.tr("Files handed to metascrub, and the cleaned copies it writes, sit "
                            + "in this app's own storage until you clear them. Clear working "
                            + "files is in the menu at the top.")
            }
        }
    }

    Scrollbar {
        flickableItem: flickable
        align: Qt.AlignTrailing
    }
}
