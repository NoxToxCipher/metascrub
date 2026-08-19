import QtQuick 2.6
import Sailfish.Silica 1.0

/*
 * About. States plainly what the app does and, just as plainly, what it refuses
 * to do — no network, no accounts — because the project's credibility comes from
 * what it will not claim.
 */
Page {
    id: page
    allowedOrientations: Orientation.All

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: column.height

        Column {
            id: column
            width: page.width
            spacing: Theme.paddingMedium
            bottomPadding: Theme.paddingLarge

            PageHeader { title: qsTr("About metascrub") }

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                source: Qt.resolvedUrl("../images/sandpiper.svg")
                sourceSize.width: Theme.iconSizeExtraLarge
                sourceSize.height: Theme.iconSizeExtraLarge
                width: Theme.iconSizeExtraLarge
                height: Theme.iconSizeExtraLarge
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.WordWrap
                color: Theme.primaryColor
                font.pixelSize: Theme.fontSizeSmall
                text: qsTr("metascrub removes the hidden data a file carries about you: where a "
                         + "photo was taken, which camera and account made a document, its editing "
                         + "history. It tells you plainly how much it could remove.")
            }

            SectionHeader { text: qsTr("What it will not do") }
            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.WordWrap
                color: Theme.secondaryColor
                font.pixelSize: Theme.fontSizeSmall
                text: qsTr("Every file is cleaned on this device. Nothing is uploaded, the app asks "
                         + "for no network access, and it keeps no accounts. It never shows a result "
                         + "it has not earned: a file it cannot fully take apart is marked so, never "
                         + "presented as clean.")
            }

            SectionHeader { text: qsTr("Source") }
            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.WordWrap
                color: Theme.secondaryColor
                font.pixelSize: Theme.fontSizeSmall
                text: qsTr("Free software, GPL-3.0. The same audited core runs on the desktop and on "
                         + "Android.\ngithub.com/NoxToxCipher/metascrub")
            }
        }

        VerticalScrollDecorator {}
    }
}
