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
    // Keep stdin open: the bridge treats stdin EOF as "parent shell died"
    // and exits. Without this, Quickshell's Process leaves stdin closed and
    // the bridge dies immediately.
    stdinEnabled: true
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
  // App settings projected from state.json. Named `appSettings` so it never
  // shadows the `settings` property Omarchy's bar injects from shell.json.
  readonly property var appSettings: stateData && stateData.settings ? stateData.settings : ({ notificationsMuted: false, showsPreviews: true })

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
      if (!isNaN(parsedNow)) {
        lastFreshMs = parsedNow
        if (Date.now() - parsedNow > 10000) {
          triggerRespawn()
        } else {
          backoffSec = 1
        }
      } else {
        lastFreshMs = Date.now()
        backoffSec = 1
      }
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

  // ---- Thread cursor and action state --------------------------------------
  property int threadCursor: 0
  property bool threadCursorActive: false

  function moveThreadCursor(dy) {
    if (!root.threads || root.threads.length === 0) return
    cancelPreview()
    root.threadCursor = Math.max(0, Math.min(root.threads.length - 1, root.threadCursor + dy))
    root.threadCursorActive = true
  }

  function activeThread() {
    if (!root.threads || root.threads.length === 0) return null
    var idx = Math.max(0, Math.min(root.threads.length - 1, root.threadCursor))
    return root.threads[idx]
  }

  function executeThreadAction(id, action) {
    if (!id || !action) return
    var argv = ["t3code-bar-bridge", "thread", id, action]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  function startNewThread() {
    var argv = ["t3code-bar-bridge", "new-thread"]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  function setSetting(key, val) {
    var argv = ["t3code-bar-bridge", "set-setting", key, val ? "true" : "false"]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  function connectPairing(token) {
    var argv = ["t3code-bar-bridge", "connect", token]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  function reconnectPairing() {
    var argv = ["t3code-bar-bridge", "reconnect"]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  function disconnectPairing() {
    var argv = ["t3code-bar-bridge", "disconnect"]
    if (Commons.Util && Commons.Util.execArgv) {
      Commons.Util.execArgv(argv)
    } else {
      Quickshell.execDetached(argv)
    }
  }

  // ---- 500ms Anchored Hover Preview with generation tracking ---------------
  property string hoveredId: ""
  property int previewGen: 0
  property bool previewVisible: false
  property string previewText: ""
  property string previewUpdatedAt: ""
  property double hoverRowY: 0

  Timer {
    id: previewTimer
    interval: 500
    repeat: false
    onTriggered: {
      if (!root.appSettings.showsPreviews || root.hoveredId === "") return
      var reqGen = root.previewGen
      var reqId = root.hoveredId
      previewProc.targetGen = reqGen
      previewProc.targetId = reqId
      previewProc.command = ["t3code-bar-bridge", "preview", reqId]
      previewProc.running = true
    }
  }

  Process {
    id: previewProc
    running: false
    property int targetGen: 0
    property string targetId: ""
    stdout: SplitParser {
      onRead: function(line) {
        if (previewProc.targetGen !== root.previewGen || root.hoveredId !== previewProc.targetId) return
        if (!root.opened || root.currentScreen !== "threads" || !root.appSettings.showsPreviews) return
        var raw = String(line || "").trim()
        if (raw === "" || raw === "null") {
          root.previewVisible = false
          return
        }
        try {
          var parsed = JSON.parse(raw)
          if (parsed && parsed.text) {
            root.previewText = parsed.text
            root.previewUpdatedAt = parsed.updatedAt || ""
            root.previewVisible = true
          } else {
            root.previewVisible = false
          }
        } catch (e) {
          root.previewVisible = false
        }
      }
    }
  }

  function requestPreview(id, rowY) {
    if (!root.appSettings.showsPreviews) return
    root.previewGen++
    root.hoveredId = id
    root.hoverRowY = rowY
    root.previewVisible = false
    previewTimer.restart()
  }

  function cancelPreview() {
    root.previewGen++
    root.hoveredId = ""
    root.previewVisible = false
    previewTimer.stop()
    if (previewProc.running) previewProc.running = false
  }

  onCurrentScreenChanged: cancelPreview()
  onAppSettingsChanged: if (!appSettings || !appSettings.showsPreviews) cancelPreview()
  onOpenedChanged: {
    if (!opened) cancelPreview()
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

  function triggerRespawn() {
    var nowMs = Date.now()
    if (nowMs - root.lastRespawnMs >= root.backoffSec * 1000) {
      root.lastRespawnMs = nowMs
      root.backoffSec = Math.min(30, root.backoffSec * 2)
      if (watchProcess.running) watchProcess.running = false
      watchProcess.running = true
    }
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
        root.triggerRespawn()
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
    tooltipText: root.summary.status ? ("T3 Code — " + root.summary.status) : "T3 Code Bar"
    // WidgetButton has no `clicked` signal — use `pressed(int button)`, which
    // below already handles left (toggle) and right (usage screen) buttons.
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
      implicitHeight: contentCol.implicitHeight
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      onMoveRequested: function(dx, dy) {
        if (root.currentScreen === "usage") {
          if (dx !== 0) usageScreen.moveInspection(dx)
          else if (dy !== 0) {
            usageScreen.scroll(dy)
            panelScroll.contentY = Math.max(0, Math.min(panelScroll.contentHeight - panelScroll.height, panelScroll.contentY + dy * 44))
          }
        } else if (root.currentScreen === "threads") {
          if (dy !== 0) root.moveThreadCursor(dy)
        }
      }

      onActivateRequested: {
        if (root.currentScreen === "threads") {
          var t = root.activeThread()
          if (t) root.executeThreadAction(t.id, "open")
        }
      }

      onTextKey: function(t) {
        if (t === "1") root.setScreen("threads")
        else if (t === "2") root.setScreen("usage")
        else if (t === "3") root.setScreen("settings")
        else if ((t === "f" || t === "F") && root.currentScreen === "usage") usageScreen.cycleProvider()
        else if ((t === "a" || t === "A") && root.currentScreen === "usage") usageScreen.toggleAllModels()
        else if (root.currentScreen === "threads") {
          if (t === "j") root.moveThreadCursor(1)
          else if (t === "k") root.moveThreadCursor(-1)
          else if (t === "b" || t === "B") {
            var tb = root.activeThread()
            if (tb) root.executeThreadAction(tb.id, "browser")
          } else if (t === "c" || t === "C") {
            var tc = root.activeThread()
            if (tc) root.executeThreadAction(tc.id, "copy")
          } else if (t === "n" || t === "N") {
            root.startNewThread()
          }
        }
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
                  model: ["Threads", "Usage", "Settings"]
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

          ThreadsInbox {
            id: threadsInbox
            visible: root.currentScreen === "threads"
            threads: root.threads
            foreground: root.foreground
            dim: root.dim
            accent: root.accent
            urgent: root.urgent
            fontFamily: root.fontFamily
            cursor: root.threadCursor
            cursorActive: root.threadCursorActive
            previewsEnabled: root.appSettings.showsPreviews !== false
            previewVisible: root.previewVisible
            previewText: root.previewText
            previewUpdatedAt: root.previewUpdatedAt
            hoverRowY: root.hoverRowY
            onPointAt: function(index) {
              root.threadCursor = index
              root.threadCursorActive = true
            }
            onRowHovered: function(id, rowY) {
              root.requestPreview(id, rowY)
            }
            onRowExited: root.cancelPreview()
            onActionRequested: function(id, action) {
              root.executeThreadAction(id, action)
            }
            onNewThreadRequested: root.startNewThread()
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

          SettingsScreen {
            id: settingsScreen
            visible: root.currentScreen === "settings"
            connection: root.connection
            settings: root.appSettings
            foreground: root.foreground
            dim: root.dim
            accent: root.accent
            onConnectRequested: function(token) {
              root.connectPairing(token)
            }
            onReconnectRequested: function() {
              root.reconnectPairing()
            }
            onDisconnectRequested: function() {
              root.disconnectPairing()
            }
            onSettingChanged: function(key, val) {
              root.setSetting(key, val)
            }
          }
        }
      }
    }
  }
}
