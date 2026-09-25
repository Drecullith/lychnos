import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property var bar
  property string moduleName
  property var settings
  property bool recoverable: false

  readonly property string stateHome: {
    var configured = Quickshell.env("XDG_STATE_HOME")
    return configured ? configured : Quickshell.env("HOME") + "/.local/state"
  }
  readonly property string presencePath: stateHome + "/lychnos/shell-presence"

  visible: recoverable
  implicitWidth: recoverable ? (bar ? bar.barSize : 26) : 0
  implicitHeight: bar ? bar.barSize : 26

  function applyPresence(value) {
    var state = String(value || "").trim()
    recoverable = state === "hidden" || state === "ghosted"
  }

  FileView {
    id: presenceFile
    path: root.presencePath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.applyPresence(text())
    onLoadFailed: root.recoverable = false
  }

  Image {
    id: icon
    anchors.centerIn: parent
    width: 18
    height: 18
    fillMode: Image.PreserveAspectFit
    smooth: true
    source: Qt.resolvedUrl("lychnos-icon.png")
  }

  MouseArea {
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton
    hoverEnabled: true

    onClicked: {
      if (root.bar)
        root.bar.run("gapplication action org.lychnos.prototype.shell restore")
    }

    onEntered: {
      if (root.bar) root.bar.showTooltip(root, "Restore Lychnos / Exit Ghost Mode")
    }

    onExited: {
      if (root.bar) root.bar.hideTooltip(root)
    }
  }
}
