import QtQuick 2.9
import Lomiri.Components 1.3
import "."

/*
 * The Handbook. It reads the very same handbook.json the Android app ships (the
 * build installs it from android/app/src/main/res/raw/), so the words are
 * maintained in one place and cannot drift between platforms.
 *
 * Rendered as one plain scrolling document rather than accordions: someone
 * reading this may be in a hurry and under pressure, and hidden text is text
 * that does not get read.
 */
Page {
    id: page

    property var stack
    property var chapters: []

    header: PageHeader {
        title: i18n.tr("Handbook")
        leadingActionBar.actions: [
            Action {
                iconName: "back"
                text: i18n.tr("Back")
                onTriggered: stack.pop()
            }
        ]
    }

    Rectangle {
        anchors.fill: parent
        color: Style.bg
        z: -1
    }

    Component.onCompleted: load()

    function load() {
        var request = new XMLHttpRequest()
        request.open("GET", Qt.resolvedUrl("handbook.json"))
        request.onreadystatechange = function() {
            if (request.readyState !== XMLHttpRequest.DONE) {
                return
            }
            try {
                var document = JSON.parse(request.responseText)
                page.chapters = document.chapters ? document.chapters : document
            } catch (error) {
                page.chapters = []
            }
        }
        request.send()
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
        contentHeight: content.height + units.gu(4)

        Column {
            id: content
            width: parent.width
            spacing: units.gu(2)
            topPadding: units.gu(2)

            Label {
                visible: page.chapters.length === 0
                x: units.gu(2)
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                color: Style.muted
                fontSize: "small"
                text: i18n.tr("The Handbook could not be loaded.")
            }

            Repeater {
                model: page.chapters.length

                delegate: Column {
                    width: content.width
                    spacing: units.gu(1)

                    Label {
                        x: units.gu(2)
                        width: parent.width - units.gu(4)
                        wrapMode: Text.WordWrap
                        fontSize: "large"
                        color: Style.teal
                        text: page.chapters[index].title ? page.chapters[index].title : ""
                    }

                    Label {
                        visible: text !== ""
                        x: units.gu(2)
                        width: parent.width - units.gu(4)
                        wrapMode: Text.WordWrap
                        fontSize: "small"
                        font.italic: true
                        color: Style.muted
                        text: page.chapters[index].intro ? page.chapters[index].intro : ""
                    }

                    Repeater {
                        model: page.chapters[index].entries ? page.chapters[index].entries : []

                        delegate: Column {
                            width: content.width
                            spacing: units.gu(0.5)
                            bottomPadding: units.gu(1)

                            Label {
                                visible: text !== ""
                                x: units.gu(2)
                                width: parent.width - units.gu(4)
                                wrapMode: Text.WordWrap
                                fontSize: "medium"
                                color: Style.text
                                text: modelData.heading ? modelData.heading : ""
                            }

                            Label {
                                x: units.gu(2)
                                width: parent.width - units.gu(4)
                                wrapMode: Text.WordWrap
                                fontSize: "small"
                                color: Style.muted
                                text: modelData.body ? modelData.body : ""
                            }
                        }
                    }
                }
            }
        }
    }

    Scrollbar {
        flickableItem: flickable
        align: Qt.AlignTrailing
    }
}
