import QtQuick 2.0
import Sailfish.Silica 1.0

/*
 * The cover shown on the app-switcher. Kept quiet on purpose: it must never
 * reveal what the user is cleaning, so it shows only the app's identity, never a
 * file name or a result. The one cover action jumps straight to picking files.
 */
CoverBackground {
    Column {
        anchors.centerIn: parent
        width: parent.width
        spacing: Theme.paddingMedium

        Image {
            anchors.horizontalCenter: parent.horizontalCenter
            source: Qt.resolvedUrl("../images/sandpiper.svg")
            sourceSize.width: Theme.iconSizeLarge
            sourceSize.height: Theme.iconSizeLarge
            width: Theme.iconSizeLarge
            height: Theme.iconSizeLarge
        }
        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "metascrub"
            color: Theme.highlightColor
            font.pixelSize: Theme.fontSizeLarge
        }
        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width - 2 * Theme.paddingLarge
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: qsTr("Cleans metadata on this device")
            color: Theme.secondaryColor
            font.pixelSize: Theme.fontSizeExtraSmall
        }
    }

    CoverActionList {
        id: coverAction
        CoverAction {
            iconSource: "image://theme/icon-cover-new"
            onTriggered: {
                app.activate()
                // The page decides what "add files" means; the cover just wakes
                // the app to its main screen.
            }
        }
    }
}
