import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons as Commons
import qs.Ui as Ui

Ui.Panel {
  id: root
  moduleName: "t3code.bar"
  ipcTarget: "t3code"
  manageIpc: false

  component PlainText: Text { textFormat: Text.PlainText }

  implicitWidth: pill.implicitWidth
  implicitHeight: pill.implicitHeight

  readonly property color foreground: bar && bar.foreground !== undefined ? bar.foreground : (Commons.Color.foreground !== undefined ? Commons.Color.foreground : "#cdd6f4")
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property color urgent: bar && bar.urgent !== undefined ? bar.urgent : (Commons.Color.urgent !== undefined ? Commons.Color.urgent : "#f7768e")
  readonly property color accent: bar && bar.accent !== undefined ? bar.accent : (Commons.Color.accent !== undefined ? Commons.Color.accent : "#7dc4e4")
  readonly property string fontFamily: bar && bar.fontFamily !== undefined ? bar.fontFamily : Commons.Style.font.family

  readonly property string home: Quickshell.env("HOME") || ""
  readonly property string stateDir: Quickshell.env("T3_CODE_BAR_STATE_DIR") || ((Quickshell.env("XDG_STATE_HOME") || home + "/.local/state") + "/t3code-bar")
  readonly property string statePath: stateDir + "/state.json"

  // Eagerly spawn watch on plugin load. No Component.onDestruction stop
  // so unplugging one monitor does not kill the bridge for other monitors.
  Process {
    id: watchProcess
    command: ["t3code-bar-bridge", "watch"]
    running: true
  }

  // Reactive state watch via FileView
  property string stateRaw: ""
  property var stateData: null

  readonly property var summary: stateData && stateData.summary ? stateData.summary : ({ status: "Settled", attentionCount: 0, runningCount: 0, badgeCount: 0 })
  readonly property var connection: stateData && stateData.connection ? stateData.connection : ({ status: "unavailable", message: "", connected: false, paired: false, hasData: false, port: 0 })
  readonly property var threads: stateData && stateData.threads ? stateData.threads : []
  readonly property var settings: stateData && stateData.settings ? stateData.settings : ({ notificationsMuted: false, showsPreviews: true })

  property double lastFreshMs: Date.now()
  property int backoffSec: 1
  property double lastRespawnMs: 0

  function applyState(content) {
    var raw = String(content || "")
    if (raw === stateRaw) return
    stateRaw = raw
    try {
      var parsed = JSON.parse(raw)
      stateData = parsed
      var parsedNow = parsed.now ? Date.parse(parsed.now) : NaN
      lastFreshMs = !isNaN(parsedNow) ? Math.max(parsedNow, Date.now()) : Date.now()
      backoffSec = 1
    } catch (e) {
      console.warn("t3code.bar: bad state.json", e)
    }
  }

  FileView {
    id: stateLoader
    path: root.statePath
    watchChanges: true
    printErrors: false
    onLoaded: root.applyState(text())
    onFileChanged: reload()
  }

  // 10s staleness crash recovery with exponential backoff (1s doubling to 30s)
  Timer {
    id: stalenessTimer
    interval: 1000
    running: true
    repeat: true
    onTriggered: {
      var staleAge = (Date.now() - root.lastFreshMs) / 1000
      if (staleAge > 10) {
        var nowMs = Date.now()
        if (nowMs - root.lastRespawnMs >= root.backoffSec * 1000) {
          root.lastRespawnMs = nowMs
          root.backoffSec = Math.min(30, root.backoffSec * 2)
          if (watchProcess.running) watchProcess.running = false
          watchProcess.running = true
        }
      }
    }
  }

  function statusColor(status) {
    switch (status) {
      case "Working":
      case "Goal": return "#60A5FA"
      case "Approval":
      case "Woke":
      case "Limited": return "#FBBF24"
      case "Input": return "#A5B4FC"
      case "Request": return "#F0ABFC"
      case "Failed": return "#F87171"
      case "Done": return "#34D399"
      case "Waiting": return "#A3A3A3"
      default: return "#A3A3A3"
    }
  }

  Component {
    id: statusDot
    Item {
      width: 16
      height: 16
      Rectangle {
        anchors.centerIn: parent
        width: 9
        height: 9
        radius: 4.5
        color: root.statusColor(root.summary.status)
        border.width: root.summary.status === "Settled" ? 1 : 0
        border.color: root.dim
      }
    }
  }

  Ui.BarIconButton {
    id: pill
    anchors.fill: parent
    bar: root.bar
    iconComponent: statusDot
    labelText: root.summary.badgeCount > 0 ? String(root.summary.badgeCount) : ""
    tooltipText: "T3 Code — " + (root.summary.status || "Settled")
      + (root.summary.attentionCount > 0 ? " · " + root.summary.attentionCount + " attention" : "")
      + (root.summary.runningCount > 0 ? " · " + root.summary.runningCount + " running" : "")
    onClicked: root.toggle()
  }

  Ui.KeyboardPanel {
    id: panel
    anchorItem: pill
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: panel.loadedContent
    contentWidth: panel.fittedContentWidth(Commons.Style.space(470))
    contentHeight: panel.fittedContentHeight(panel.loadedContentHeight, Commons.Style.space(800))

    content: Ui.PanelKeyCatcher {
      id: keyCatcher
      implicitHeight: contentCol.implicitHeight
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: contentCol
        width: parent.width
        spacing: Commons.Style.spacing.md

        Ui.PanelHero {
          title: "T3 Code"
          meta: {
            var bits = []
            if (root.summary.attentionCount > 0) bits.push(root.summary.attentionCount + " ATTENTION")
            if (root.summary.runningCount > 0) bits.push(root.summary.runningCount + " RUNNING")
            return bits.length > 0 ? bits.join(" · ") : "SETTLED"
          }
          detail: root.connection.message !== "" ? root.connection.message : root.connection.status
          foreground: root.foreground
          fontFamily: root.fontFamily
        }

        PlainText {
          anchors.horizontalCenter: parent.horizontalCenter
          text: root.summary.status + " · " + root.threads.length + " threads"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Commons.Style.font.caption
        }
      }
    }
  }
}
