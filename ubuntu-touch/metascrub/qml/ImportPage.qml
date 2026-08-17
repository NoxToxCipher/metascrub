import QtQuick 2.9
import Lomiri.Components 1.3
import Lomiri.Content 0.1
import "."

/*
 * Choosing files, the only way a confined app can: ask the Content Hub, let the
 * user pick the app that holds the file, and receive a copy.
 *
 * metascrub cannot read the picture library, the download folder or anything
 * else. That is not a limitation worked around here, it is the point — the app
 * can only ever see the files a person deliberately hands it.
 */
Page {
    id: page

    property var app
    property var stack
    property var activeTransfer

    header: PageHeader {
        title: i18n.tr("Choose files")
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

    ContentPeerPicker {
        id: picker
        anchors {
            top: page.header.bottom
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
        showTitle: false
        contentType: ContentType.All
        handler: ContentHandler.Source

        onPeerSelected: {
            peer.selectionType = ContentTransfer.Multiple
            page.activeTransfer = peer.request()
        }

        onCancelPressed: stack.pop()
    }

    ContentTransferHint {
        anchors.fill: parent
        activeTransfer: page.activeTransfer
    }

    Connections {
        target: page.activeTransfer
        onStateChanged: {
            if (page.activeTransfer.state === ContentTransfer.Charged) {
                app.addItems(page.activeTransfer.items)
                stack.pop()
            }
        }
    }
}
