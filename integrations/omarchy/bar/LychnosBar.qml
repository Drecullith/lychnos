import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property var bar
  property string moduleName
  property var settings
  property bool minimized: false

  readonly property string stateHome: {
    var configured = Quickshell.env("XDG_STATE_HOME")
    return configured ? configured : Quickshell.env("HOME") + "/.local/state"
  }
  readonly property string presencePath: stateHome + "/lychnos/shell-presence"

  visible: minimized
  implicitWidth: minimized ? (bar ? bar.barSize : 26) : 0
  implicitHeight: bar ? bar.barSize : 26

  function applyPresence(value) {
    minimized = String(value || "").trim() === "hidden"
  }

  FileView {
    id: presenceFile
    path: root.presencePath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.applyPresence(text())
    onLoadFailed: root.minimized = false
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
      if (root.bar) root.bar.showTooltip(root, "Restore Lychnos")
    }

    onExited: {
      if (root.bar) root.bar.hideTooltip(root)
    }
  }
}
