import QtQuick 2.9
import Lomiri.Components 1.3
import "."

/*
 * A plain block of text in a bordered panel. The accent colour carries tone —
 * teal for how you got here, amber for a claim that needs qualifying — but the
 * sentence inside always says the thing outright, so the colour is decoration
 * rather than information.
 */
Rectangle {
    id: card

    property alias text: cardLabel.text
    property color accent: Style.stroke

    x: units.gu(2)
    width: parent ? parent.width - units.gu(4) : 0
    height: cardLabel.implicitHeight + units.gu(3)

    radius: units.gu(0.5)
    color: Style.surface
    border.width: units.dp(1)
    border.color: accent

    Label {
        id: cardLabel
        anchors {
            fill: parent
            margins: units.gu(1.5)
        }
        wrapMode: Text.WordWrap
        fontSize: "small"
        color: Style.text
    }
}
