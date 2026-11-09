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
  readonly property string usagePath: stateDir + "/usage.json"
  readonly property string limitsPath: stateDir + "/limits.json"
  readonly property string viewPath: stateDir + "/view.json"

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
  property string usageRaw: ""
  property var usageData: null
  property string limitsRaw: ""
  property var limitsData: null
  property string viewRaw: ""
  property var viewData: null

  readonly property var summary: stateData && stateData.summary ? stateData.summary : ({ status: "Settled", attentionCount: 0, runningCount: 0, badgeCount: 0 })
  readonly property var connection: stateData && stateData.connection ? stateData.connection : ({ status: "unavailable", message: "", connected: false, paired: false, hasData: false, port: 0 })
  readonly property var threads: stateData && stateData.threads ? stateData.threads : []
  readonly property var settings: stateData && stateData.settings ? stateData.settings : ({ notificationsMuted: false, showsPreviews: true })

  property string currentScreen: viewData && viewData.screen ? viewData.screen : "threads"
  property string currentMetric: viewData && viewData.metric ? viewData.metric : "cost"
  property string currentPeriod: viewData && viewData.period ? viewData.period : "24h"

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

  function applyUsage(content) {
    var raw = String(content || "")
    if (raw === usageRaw) return
    usageRaw = raw
    try {
      usageData = JSON.parse(raw)
    } catch (e) {
      console.warn("t3code.bar: bad usage.json", e)
    }
  }

  function applyLimits(content) {
    var raw = String(content || "")
    if (raw === limitsRaw) return
    limitsRaw = raw
    try {
      limitsData = JSON.parse(raw)
    } catch (e) {
      console.warn("t3code.bar: bad limits.json", e)
    }
  }

  function applyView(content) {
    var raw = String(content || "")
    if (raw === viewRaw) return
    viewRaw = raw
    try {
      var parsed = JSON.parse(raw)
      viewData = parsed
      if (parsed.screen) currentScreen = parsed.screen
      if (parsed.metric) currentMetric = parsed.metric
      if (parsed.period) currentPeriod = parsed.period
    } catch (e) {
      console.warn("t3code.bar: bad view.json", e)
    }
  }

  function updateView(options) {
    var argv = ["t3code-bar-bridge", "set-view"]
    if (options.visible !== undefined) argv.push(options.visible ? "--visible" : "--hidden")
    if (options.screen !== undefined) { argv.push("--screen"); argv.push(options.screen) }
    if (options.metric !== undefined) { argv.push("--metric"); argv.push(options.metric) }
    if (options.period !== undefined) { argv.push("--period"); argv.push(options.period) }
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  function setScreen(s) {
    currentScreen = s
    updateView({ screen: s })
  }

  function openLink(url) {
    var argv = ["t3code-bar-bridge", "open-link", url]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  onOpenedChanged: {
    updateView({ visible: opened })
  }

  FileView {
    id: stateLoader
    path: root.statePath
    watchChanges: true
    printErrors: false
    onLoaded: root.applyState(text())
    onFileChanged: reload()
  }

  FileView {
    id: usageLoader
    path: root.usagePath
    watchChanges: true
    printErrors: false
    onLoaded: root.applyUsage(text())
    onFileChanged: reload()
  }

  FileView {
    id: limitsLoader
    path: root.limitsPath
    watchChanges: true
    printErrors: false
    onLoaded: root.applyLimits(text())
    onFileChanged: reload()
  }

  FileView {
    id: viewLoader
    path: root.viewPath
    watchChanges: true
    printErrors: false
    onLoaded: root.applyView(text())
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
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) {
        root.setScreen("usage")
        root.open()
      } else {
        root.toggle()
      }
    }
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
      implicitHeight: panelScroll.implicitHeight
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      onMoveRequested: function(dx, dy) {
        if (root.currentScreen === "usage") {
          if (dx !== 0) usageScreen.moveInspection(dx)
          else if (dy !== 0) panelScroll.contentY = Math.max(0, Math.min(panelScroll.contentHeight - panelScroll.height, panelScroll.contentY + dy * 44))
        }
      }

      onTextKey: function(t) {
        if (t === "1") root.setScreen("threads")
        else if (t === "2") root.setScreen("usage")
        else if ((t === "f" || t === "F") && root.currentScreen === "usage") usageScreen.cycleProvider()
        else if ((t === "a" || t === "A") && root.currentScreen === "usage") usageScreen.toggleAllModels()
      }

      Flickable {
        id: panelScroll
        anchors.fill: parent
        contentWidth: width
        contentHeight: contentCol.implicitHeight
        interactive: contentHeight > height
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        Column {
          id: contentCol
          width: panelScroll.width
          anchors.top: parent.top
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
            trailingControl: Component {
              Row {
                spacing: Commons.Style.spacing.xs
                Repeater {
                  model: ["Threads", "Usage"]
                  delegate: Ui.Button {
                    required property string modelData
                    required property int index
                    text: (index + 1) + " " + modelData
                    selected: root.currentScreen === modelData.toLowerCase()
                    fontSize: Commons.Style.font.caption
                    horizontalPadding: Commons.Style.spacing.sm
                    verticalPadding: Commons.Style.spacing.xxs
                    onClicked: root.setScreen(modelData.toLowerCase())
                  }
                }
              }
            }
          }

          PlainText {
            visible: root.currentScreen === "threads"
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.summary.status + " · " + root.threads.length + " threads"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Commons.Style.font.caption
          }

          UsageScreen {
            id: usageScreen
            visible: root.currentScreen === "usage"
            usageData: root.usageData
            limitsData: root.limitsData
            metric: root.currentMetric
            period: root.currentPeriod
            foreground: root.foreground
            dim: root.dim
            accent: root.accent
            urgent: root.urgent
            onViewChangeRequested: function(m, p) {
              root.updateView({ metric: m, period: p })
            }
            onLinkOpenRequested: function(url) {
              root.openLink(url)
            }
          }
        }
      }
    }
  }
}
