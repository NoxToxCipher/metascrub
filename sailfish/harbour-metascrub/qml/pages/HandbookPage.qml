import QtQuick 2.6
import Sailfish.Silica 1.0

/*
 * The Handbook. It loads the very same handbook.json the Android app ships (the
 * build copies it in from android/app/src/main/res/raw/handbook.json), so the
 * words are maintained in one place across platforms, and renders each chapter as
 * a native Silica expanding section.
 */
Page {
    id: page
    allowedOrientations: Orientation.All

    property var chapters: []

    Component.onCompleted: loadHandbook()

    function loadHandbook() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", Qt.resolvedUrl("../handbook.json"))
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                try {
                    var doc = JSON.parse(xhr.responseText)
                    chapters = doc.chapters ? doc.chapters : doc
                } catch (e) {
                    chapters = []
                }
            }
        }
        xhr.send()
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        Column {
            id: content
            width: page.width

            PageHeader { title: qsTr("Handbook") }

            ExpandingSectionGroup {
                width: parent.width

                Repeater {
                    model: page.chapters.length
                    delegate: ExpandingSection {
                        title: page.chapters[index].title ? page.chapters[index].title : ""
                        content.sourceComponent: Column {
                            width: parent.width
                            spacing: Theme.paddingMedium

                            Label {
                                visible: !!page.chapters[index].intro
                                x: Theme.horizontalPageMargin
                                width: parent.width - 2 * Theme.horizontalPageMargin
                                wrapMode: Text.WordWrap
                                color: Theme.secondaryColor
                                font.pixelSize: Theme.fontSizeSmall
                                font.italic: true
                                text: page.chapters[index].intro ? page.chapters[index].intro : ""
                            }

                            Repeater {
                                model: page.chapters[index].entries
                                    ? page.chapters[index].entries : []
                                delegate: Column {
                                    x: Theme.horizontalPageMargin
                                    width: parent.width - 2 * Theme.horizontalPageMargin
                                    spacing: Theme.paddingSmall
                                    bottomPadding: Theme.paddingMedium

                                    Label {
                                        visible: !!modelData.heading
                                        width: parent.width
                                        wrapMode: Text.WordWrap
                                        color: Theme.highlightColor
                                        font.pixelSize: Theme.fontSizeSmall
                                        text: modelData.heading ? modelData.heading : ""
                                    }
                                    Label {
                                        width: parent.width
                                        wrapMode: Text.WordWrap
                                        color: Theme.secondaryHighlightColor
                                        font.pixelSize: Theme.fontSizeSmall
                                        text: modelData.body ? modelData.body : ""
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        VerticalScrollDecorator {}
    }
}
