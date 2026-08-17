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
        LayoutMirroring.enabled: systemRightToLeft
        LayoutMirroring.childrenInherit: true
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

    Component.onCompleted: load(candidates())

    /*
     * The Handbook in the phone's language if it exists, English otherwise.
     * A locale like "pt_BR" is tried whole first, then as "pt", which is what
     * gettext does for the interface strings, so both follow the same rule.
     */
    function candidates() {
        var locale = systemLocale ? systemLocale : ""
        var names = []
        if (locale !== "") {
            names.push("handbook-" + locale + ".json")
            var bare = locale.split(/[_.@]/)[0]
            if (bare !== "" && bare !== locale) {
                names.push("handbook-" + bare + ".json")
            }
        }
        names.push("handbook.json")
        return names
    }

    /* Tries each name in turn; the last one is the English original. */
    function load(names) {
        if (names.length === 0) {
            page.chapters = []
            return
        }
        var request = new XMLHttpRequest()
        request.open("GET", Qt.resolvedUrl(names[0]))
        request.onreadystatechange = function() {
            if (request.readyState !== XMLHttpRequest.DONE) {
                return
            }
            var parsed = null
            try {
                parsed = JSON.parse(request.responseText)
            } catch (error) {
                parsed = null
            }
            if (parsed) {
                page.chapters = parsed.chapters ? parsed.chapters : parsed
            } else {
                load(names.slice(1))
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
        // Read the other way round in Arabic, Farsi, Sorani and Kurmanji. This
        // sits on the page content rather than on MainView: mirroring the whole
        // window makes PageStack size every page to twice the window width and
        // park it at -width, which puts the middle of the page off-screen.
        LayoutMirroring.enabled: systemRightToLeft
        LayoutMirroring.childrenInherit: true
        contentWidth: width
        contentHeight: content.height + units.gu(4)

        Column {
            id: content
            width: parent.width
            spacing: units.gu(2)
            topPadding: units.gu(2)

            Label {
                visible: page.chapters.length === 0
                anchors.horizontalCenter: parent.horizontalCenter
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
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: parent.width - units.gu(4)
                        wrapMode: Text.WordWrap
                        fontSize: "large"
                        color: Style.teal
                        text: page.chapters[index].title ? page.chapters[index].title : ""
                    }

                    Label {
                        visible: text !== ""
                        anchors.horizontalCenter: parent.horizontalCenter
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
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: parent.width - units.gu(4)
                                wrapMode: Text.WordWrap
                                fontSize: "medium"
                                color: Style.text
                                text: modelData.heading ? modelData.heading : ""
                            }

                            Label {
                                anchors.horizontalCenter: parent.horizontalCenter
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
