import QtQuick 2.9
import Lomiri.Components 1.3
import Lomiri.Content 1.3
import "."

/*
 * Handing the cleaned copies to wherever the user wants them — the gallery, the
 * file manager, a messenger. The originals are never offered: only the files
 * metascrub wrote itself, in its own storage, are put on the transfer.
 */
Page {
    id: page

    property var app
    property var stack
    property var paths: []
    property var activeTransfer

    header: PageHeader {
        LayoutMirroring.enabled: systemRightToLeft
        LayoutMirroring.childrenInherit: true
        title: i18n.tr("Save cleaned copies")
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

    Component { id: itemComponent; ContentItem {} }

    ContentPeerPicker {
        anchors {
            top: page.header.bottom
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
        showTitle: false
        contentType: ContentType.All
        handler: ContentHandler.Destination

        onPeerSelected: {
            peer.selectionType = ContentTransfer.Multiple
            var transfer = peer.request()
            var items = []
            for (var i = 0; i < page.paths.length; ++i) {
                items.push(itemComponent.createObject(page,
                    { "url": app.workspace.urlFromPath(page.paths[i]) }))
            }
            transfer.items = items
            transfer.state = ContentTransfer.Charged
            page.activeTransfer = transfer
            // Held by the root object, not by this page: the page is about to be
            // popped and destroyed, and the transfer has to outlive it.
            app.lastExport = transfer

            app.flash = app.savedMessage(page.paths.length)
            stack.pop()
        }

        onCancelPressed: stack.pop()
    }
}
