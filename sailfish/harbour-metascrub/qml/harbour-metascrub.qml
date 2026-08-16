import QtQuick 2.0
import Sailfish.Silica 1.0
import "pages"

/*
 * The application window. Silica's ApplicationWindow gives the native shell:
 * the page stack, the cover shown on the app-switcher, and orientation handling.
 */
ApplicationWindow {
    id: app

    initialPage: Component { ScrubPage { } }
    cover: Qt.resolvedUrl("cover/CoverPage.qml")
    allowedOrientations: defaultAllowedOrientations
}
