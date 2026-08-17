import QtQuick 2.9
import Lomiri.Components 1.3
import "."

/*
 * About. What the app does, and just as plainly what it refuses to do and what
 * it cannot promise. The project's credibility comes from what it will not
 * claim, so this page states the limits as clearly as the features.
 */
Page {
    id: page

    property var stack

    header: PageHeader {
        LayoutMirroring.enabled: systemRightToLeft
        LayoutMirroring.childrenInherit: true
        title: i18n.tr("About metascrub")
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
            width: parent.width
            spacing: units.gu(2)
            topPadding: units.gu(2)

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                source: Qt.resolvedUrl("images/sandpiper.svg")
                sourceSize.width: units.gu(8)
                sourceSize.height: units.gu(8)
                width: units.gu(8)
                height: units.gu(8)
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                fontSize: "small"
                color: Style.text
                text: i18n.tr("metascrub removes the hidden data a file carries about you — where "
                            + "a photo was taken, which camera and account made a document, its "
                            + "editing history — and tells you honestly how much it could remove.")
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                text: i18n.tr("What it will not do")
                fontSize: "medium"
                color: Style.teal
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                fontSize: "small"
                color: Style.muted
                text: i18n.tr("Every file is cleaned on this phone. Nothing is uploaded, and the "
                            + "app keeps no accounts. It never shows a result it has not earned: "
                            + "a file it cannot fully take apart is marked so, never presented as "
                            + "clean.")
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                text: i18n.tr("You do not have to take our word for it")
                fontSize: "medium"
                color: Style.teal
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                fontSize: "small"
                color: Style.muted
                text: i18n.tr("On Ubuntu Touch every app runs under an AppArmor profile that lists "
                            + "what it is allowed to touch. metascrub asks for two things: to "
                            + "receive files from other apps, and to give files back. It does not "
                            + "ask for network access, so the system itself refuses to let it "
                            + "reach one. The profile is a short file inside the app package "
                            + "(metascrub.apparmor) and anyone can read it.")
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                text: i18n.tr("What it cannot promise")
                fontSize: "medium"
                color: Style.warn
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                fontSize: "small"
                color: Style.muted
                text: i18n.tr("Files handed to metascrub, and the cleaned copies it writes, sit in "
                            + "this app's own storage until you clear them. Clearing deletes them "
                            + "the ordinary way: on flash storage the data may survive until those "
                            + "blocks are reused, and no application can change that. Nothing here "
                            + "protects you from a phone that has already been tampered with.")
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                text: i18n.tr("Source")
                fontSize: "medium"
                color: Style.teal
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width - units.gu(4)
                wrapMode: Text.WordWrap
                fontSize: "small"
                color: Style.muted
                text: i18n.tr("Free software, GPL-3.0. The same audited core runs on the desktop, "
                            + "on Android and on Sailfish OS.\ngithub.com/NoxToxCipher/metascrub")
            }
        }
    }

    Scrollbar {
        flickableItem: flickable
        align: Qt.AlignTrailing
    }
}
