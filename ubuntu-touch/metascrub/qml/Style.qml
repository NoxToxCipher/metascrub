pragma Singleton
import QtQuick 2.9

/*
 * The one palette, shared with the Android app and the desktop GUI
 * (android/app/src/main/res/values/colors.xml) so the family looks like one
 * tool. Every colour is stated here rather than taken from the shell theme,
 * because the assurance colours have to mean the same thing on every platform,
 * and because a theme change must never turn "NOT CLEANED" into something that
 * reads as fine.
 *
 * Colour is never the only signal: each badge carries its word, and each result
 * carries a sentence. WCAG 1.4.1.
 */
QtObject {
    // Ground and surfaces
    readonly property color bg: "#0E1417"
    readonly property color surface: "#172025"
    readonly property color stroke: "#26343B"

    // Brand
    readonly property color teal: "#5FB0BA"
    readonly property color onTeal: "#052027"

    // Text
    readonly property color text: "#EAF1F2"
    readonly property color muted: "#93A6AC"

    // Assurance
    readonly property color ok: "#57B982"       // COMPLETE
    readonly property color warn: "#E0A458"     // BEST EFFORT, and anything kept
    readonly property color danger: "#E0776A"   // NOT CLEANED, location found
}
