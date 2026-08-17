import QtQuick 2.9
import Lomiri.Components 1.3
import "."

/*
 * The main screen: the files waiting, the options, and one honest result per
 * file. Nothing is read until Scrub is pressed, nothing is written until the
 * cleaned copies are saved, and nothing leaves the phone at any point.
 *
 * The result rows follow the same rule as every other metascrub interface: a
 * badge word, a colour, and a sentence saying what the verdict actually means.
 * The colour is never carrying the meaning on its own.
 */
Page {
    id: page

    property var app
    property var stack

    header: PageHeader {
        id: pageHeader
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

    function runPrimary() {
        if (!app.scrubbed) {
            app.scrubAll()
            return
        }
        var paths = app.saveCleaned()
        if (paths.length === 0) {
            app.flash = i18n.tr("Nothing to save — none of these files could be cleaned.")
            return
        }
        if (app.pendingExport) {
            app.handBack(paths)
            return
        }
        stack.push(Qt.resolvedUrl("ExportPage.qml"),
                   { "app": app, "stack": stack, "paths": paths })
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
        contentHeight: column.height + units.gu(4)

        Column {
            id: column
            width: parent.width
            spacing: units.gu(2)
            topPadding: units.gu(2)

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                source: Qt.resolvedUrl("images/sandpiper.svg")
                sourceSize.width: units.gu(6)
                sourceSize.height: units.gu(6)
                width: units.gu(6)
                height: units.gu(6)
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                text: i18n.tr("removes what a file says about you")
                color: Style.muted
                fontSize: "small"
            }

            // --- how we got here -------------------------------------------
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

            // --- options ----------------------------------------------------
            ListItem {
                divider.visible: false
                height: units.gu(6)
                color: "transparent"
                ListItemLayout {
                    title.text: i18n.tr("Give cleaned files random names")
                    title.color: Style.text
                    subtitle.text: i18n.tr("A file name is metadata too")
                    subtitle.color: Style.muted
                    Switch {
                        SlotsLayout.position: SlotsLayout.Trailing
                        checked: app.optRandomNames
                        onCheckedChanged: app.optRandomNames = checked
                    }
                }
            }

            ListItem {
                divider.visible: false
                height: units.gu(6)
                color: "transparent"
                ListItemLayout {
                    title.text: i18n.tr("Keep colour profile")
                    title.color: Style.text
                    subtitle.text: i18n.tr("Keeps colours accurate, and is itself identifying")
                    subtitle.color: Style.muted
                    Switch {
                        SlotsLayout.position: SlotsLayout.Trailing
                        checked: app.optKeepColour
                        onCheckedChanged: app.optKeepColour = checked
                    }
                }
            }

            ListItem {
                divider.visible: false
                height: units.gu(6)
                color: "transparent"
                ListItemLayout {
                    title.text: i18n.tr("Keep image orientation")
                    title.color: Style.text
                    subtitle.text: i18n.tr("Stops photos appearing rotated")
                    subtitle.color: Style.muted
                    Switch {
                        SlotsLayout.position: SlotsLayout.Trailing
                        checked: app.optKeepOrientation
                        onCheckedChanged: app.optKeepOrientation = checked
                    }
                }
            }

            ListItem {
                divider.visible: false
                height: units.gu(6)
                color: "transparent"
                ListItemLayout {
                    title.text: i18n.tr("Reduce camera fingerprint (photos)")
                    title.color: Style.text
                    subtitle.text: i18n.tr("Reduces linkability. Does not remove it.")
                    subtitle.color: Style.muted
                    Switch {
                        SlotsLayout.position: SlotsLayout.Trailing
                        checked: app.optFingerprint
                        onCheckedChanged: app.optFingerprint = checked
                    }
                }
            }

            Card {
                visible: app.optFingerprint
                accent: Style.warn
                text: i18n.tr("Softens the pixels to weaken the sensor fingerprint a camera "
                            + "leaves in every photo. This reduces how easily photos can be "
                            + "linked to one camera. It does not remove the fingerprint, and it "
                            + "makes the photo softer and a little smaller. Photos are saved as "
                            + "JPEG.")
            }

            OptionSelector {
                visible: app.optFingerprint
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                text: i18n.tr("Strength")
                model: [i18n.tr("Gentle"), i18n.tr("Balanced"), i18n.tr("Thorough")]
                selectedIndex: app.optStrength
                onSelectedIndexChanged: app.optStrength = selectedIndex
            }

            // --- the queue and its results ----------------------------------
            Label {
                visible: app.queue.count > 0
                x: units.gu(2)
                text: app.scrubbed ? i18n.tr("Results") : i18n.tr("Ready to scrub")
                color: Style.teal
                fontSize: "medium"
            }

            Repeater {
                model: app.queue

                delegate: Rectangle {
                    width: column.width - units.gu(4)
                    x: units.gu(2)
                    height: rowColumn.height + units.gu(3)
                    radius: units.gu(0.5)
                    color: Style.surface
                    border.width: units.dp(1)
                    border.color: Style.stroke

                    Column {
                        id: rowColumn
                        y: units.gu(1.5)
                        x: units.gu(1.5)
                        width: parent.width - units.gu(3)
                        spacing: units.gu(0.5)

                        Label {
                            width: parent.width
                            elide: Text.ElideMiddle
                            text: model.name
                            color: Style.text
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

                        Button {
                            text: i18n.tr("Remove")
                            onClicked: {
                                app.queue.remove(index)
                                app.scrubbed = false
                            }
                        }
                    }
                }
            }

            // --- actions ----------------------------------------------------
            Button {
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                text: app.queue.count === 0 ? i18n.tr("Choose files") : i18n.tr("Add more")
                onClicked: stack.push(Qt.resolvedUrl("ImportPage.qml"),
                                      { "app": app, "stack": stack })
            }

            Button {
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                visible: app.queue.count > 0
                color: Style.teal
                text: page.primaryText()
                onClicked: page.runPrimary()
            }

            Button {
                width: parent.width - units.gu(4)
                anchors.horizontalCenter: parent.horizontalCenter
                visible: app.queue.count > 0
                text: i18n.tr("Start over")
                onClicked: {
                    app.queue.clear()
                    app.scrubbed = false
                    app.handedIn = false
                    app.flash = ""
                }
            }

            Card {
                visible: app.flash !== ""
                accent: Style.teal
                text: app.flash
            }

            Label {
                x: units.gu(2)
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
