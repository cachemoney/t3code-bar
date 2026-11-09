import QtQuick
import qs.Commons as Commons
import qs.Ui as Ui

Column {
  id: scr

  width: parent ? parent.width : 0

  property var usageData
  property var limitsData
  property color foreground
  property color dim
  property color accent
  property color urgent

  property string metric: "cost"   // cost | tokens | limits
  property string period: "24h"    // 24h | 7d | 30d | 90d
  property string providerFilter: ""  // empty = all providers
  property bool showAllModels: false
  property int inspectionIndex: -1
  property real inspectionX: 0
  property real inspectionY: 0
  property bool inspectionKeyboard: false

  signal viewChangeRequested(string metric, string period)
  signal linkOpenRequested(string url)

  spacing: Commons.Style.spacing.sm

  component PlainText: Text { textFormat: Text.PlainText }

  readonly property var periodData: usageData && usageData.periods ? (usageData.periods[period] || null) : null
  readonly property var timeline: periodData && periodData.timeline ? periodData.timeline : []
  readonly property var allProviders: periodData && periodData.providers ? periodData.providers : []
  readonly property var activeProviders: {
    if (providerFilter === "") return allProviders
    var rows = []
    for (var i = 0; i < allProviders.length; i++) {
      if (allProviders[i].kind === providerFilter) rows.push(allProviders[i])
    }
    return rows
  }
  readonly property var filterOptions: {
    var options = [{ kind: "", label: "All providers" }]
    for (var i = 0; i < allProviders.length; i++) {
      options.push({ kind: allProviders[i].kind, label: providerLabel(allProviders[i].kind, allProviders[i].label) })
    }
    return options
  }
  readonly property var inspectedSlot: inspectionIndex >= 0 && inspectionIndex < timeline.length ? timeline[inspectionIndex] : null
  readonly property real inspectionTotal: slotTotal(inspectedSlot)
  readonly property real filteredCost: totalFor(activeProviders, "costUsd")
  readonly property real filteredTokens: totalFor(activeProviders, "tokens")
  readonly property real filteredSessions: totalFor(activeProviders, "sessions")
  readonly property var orderedModels: {
    var rows = periodData && periodData.models ? periodData.models.slice() : []
    if (providerFilter !== "") {
      rows = rows.filter(function(row) { return row.kind === providerFilter })
    }
    if (metric === "tokens") {
      rows.sort(function(a, b) { return b.tokens - a.tokens || b.costUsd - a.costUsd })
    }
    return rows
  }
  readonly property var visibleModelRows: showAllModels ? orderedModels : orderedModels.slice(0, 5)
  readonly property real largestProviderValue: largestValue(activeProviders)
  readonly property real largestModelValue: largestValue(orderedModels)

  function providerLabel(kind, fallback) {
    var labels = {
      codex: "Codex", claude: "Claude Code", claudeAgent: "Claude Code",
      grok: "Grok Build", cursor: "Cursor", opencode: "OpenCode",
      antigravity: "Antigravity", pi: "Pi", muse: "Muse Code"
    }
    return labels[kind] || fallback || kind
  }

  function providerColor(kind) {
    var colors = {
      codex: "#f4f4f5", claude: "#d97757", claudeAgent: "#d97757",
      grok: "#b0b0b0", cursor: "#8b8b8b", opencode: "#5b9bbd",
      antigravity: "#8c7bd1", pi: "#4d9f8a", muse: "#0866ff"
    }
    return colors[kind] || "#8b8b8b"
  }

  function metricValue(row) {
    return row ? (metric === "cost" ? Number(row.costUsd) : Number(row.tokens)) : 0
  }

  function totalFor(rows, key) {
    var total = 0
    for (var i = 0; i < rows.length; i++) total += Number(rows[i][key]) || 0
    return total
  }

  function largestValue(rows) {
    var largest = 0
    for (var i = 0; i < rows.length; i++) largest = Math.max(largest, metricValue(rows[i]))
    return largest
  }

  function compact(value) {
    var scales = [[1e12, "T"], [1e9, "B"], [1e6, "M"], [1e3, "K"]]
    for (var i = 0; i < scales.length; i++) {
      var scale = scales[i][0]
      if (Math.abs(value) < scale) continue
      var n = value / scale
      var digits = Math.abs(n) >= 100 ? 0 : Math.abs(n) >= 10 ? 1 : 2
      return n.toFixed(digits).replace(/\.0+$/, "") + scales[i][1]
    }
    return Math.floor(value + 0.5).toLocaleString(Qt.locale("en_US"))
  }

  function valueText(value) {
    return metric === "cost" ? "$" + Number(value).toFixed(2) : compact(value)
  }

  function percent(value, total) {
    if (total <= 0) return "0.0%"
    var share = value / total
    return share > 0 && share < 0.001 ? "<0.1%" : (share * 100).toFixed(1) + "%"
  }

  function providerDetail(row) {
    return metric === "cost"
      ? percent(row.costUsd, filteredCost) + " of filtered cost · " + compact(row.tokens) + " tokens"
      : percent(row.tokens, filteredTokens) + " of filtered tokens · $" + row.costUsd.toFixed(2)
  }

  function modelUnpriced(row) {
    return row.records > 0 && row.unpricedRecords >= row.records
  }

  function modelPrimaryText(row) {
    if (metric === "tokens") return compact(row.tokens)
    return modelUnpriced(row) ? "Unpriced" : "$" + row.costUsd.toFixed(2)
  }

  function modelDetail(row) {
    if (metric === "tokens") {
      return percent(row.tokens, filteredTokens) + " · $" + row.costUsd.toFixed(2)
    }
    if (modelUnpriced(row)) return compact(row.tokens) + " tokens · unpriced"
    return percent(row.costUsd, filteredCost) + " · " + compact(row.tokens) + " tokens"
  }

  function modelBarShare(row) {
    if (metric === "cost" && modelUnpriced(row)) return 0
    var value = metricValue(row)
    return value > 0 && largestModelValue > 0 ? value / largestModelValue : 0
  }

  function slotProvider(slot, kind) {
    if (slot && slot.providers) {
      for (var i = 0; i < slot.providers.length; i++) {
        if (slot.providers[i].kind === kind) return slot.providers[i]
      }
    }
    return { kind: kind, costUsd: 0, tokens: 0 }
  }

  function slotTotal(slot) {
    if (!slot) return 0
    if (activeProviders.length === 0) return metricValue(slot)
    var total = 0
    for (var i = 0; i < activeProviders.length; i++) total += metricValue(slotProvider(slot, activeProviders[i].kind))
    return total
  }

  function barWidth() {
    if (timeline.length === 0) return 0
    var gap = timeline.length > 40 ? 1 : 2
    return (chart.width - (timeline.length - 1) * gap) / timeline.length
  }

  function slotCenter(index) {
    var gap = timeline.length > 40 ? 1 : 2
    var width = barWidth()
    return index * (width + gap) + width / 2
  }

  function inspectAt(x, y) {
    if (timeline.length === 0) return
    var gap = timeline.length > 40 ? 1 : 2
    var step = barWidth() + gap
    inspectionIndex = Math.max(0, Math.min(timeline.length - 1, Math.round((x - barWidth() / 2) / step)))
    inspectionX = Math.max(0, Math.min(chart.width, x))
    inspectionY = Math.max(0, Math.min(chart.height, y))
    inspectionKeyboard = false
  }

  function moveInspection(dx) {
    if (metric === "limits" || timeline.length === 0 || dx === 0) return
    inspectionIndex = inspectionIndex < 0
      ? (dx > 0 ? 0 : timeline.length - 1)
      : Math.max(0, Math.min(timeline.length - 1, inspectionIndex + dx))
    inspectionX = slotCenter(inspectionIndex)
    inspectionY = 18
    inspectionKeyboard = true
  }

  function clearInspection() {
    inspectionIndex = -1
    inspectionKeyboard = false
  }

  function cycleProvider() {
    if (metric === "limits" || filterOptions.length === 0) return
    var current = 0
    for (var i = 0; i < filterOptions.length; i++) {
      if (filterOptions[i].kind === providerFilter) current = i
    }
    selectProvider(filterOptions[(current + 1) % filterOptions.length].kind)
  }

  function selectProvider(kind) {
    providerFilter = kind
    showAllModels = false
    clearInspection()
  }

  function toggleAllModels() {
    showAllModels = !showAllModels
  }

  onMetricChanged: { showAllModels = false; clearInspection() }
  onPeriodChanged: { showAllModels = false; clearInspection() }
  onPeriodDataChanged: clearInspection()
  onVisibleChanged: if (!visible) clearInspection()

  Row {
    spacing: Commons.Style.spacing.xs
    Repeater {
      model: ["Cost", "Tokens", "Limits"]
      delegate: Ui.Button {
        required property string modelData
        text: modelData
        selected: scr.metric === modelData.toLowerCase()
        fontSize: Commons.Style.font.caption
        horizontalPadding: Commons.Style.spacing.md
        verticalPadding: Commons.Style.spacing.xxs
        onClicked: {
          scr.metric = modelData.toLowerCase()
          scr.viewChangeRequested(scr.metric, scr.period)
        }
      }
    }
  }

  Row {
    visible: scr.metric !== "limits"
    spacing: Commons.Style.spacing.xxs
    Repeater {
      model: ["24h", "7d", "30d", "90d"]
      delegate: Ui.Button {
        required property string modelData
        text: modelData
        selected: scr.period === modelData
        fontSize: Commons.Style.font.caption
        horizontalPadding: Commons.Style.spacing.sm
        verticalPadding: Commons.Style.spacing.xxs
        onClicked: {
          scr.period = modelData
          scr.viewChangeRequested(scr.metric, scr.period)
        }
      }
    }
  }

  PlainText {
    visible: scr.metric !== "limits"
    width: scr.width
    elide: Text.ElideRight
    color: scr.dim
    font.family: Commons.Style.font.family
    font.pixelSize: Commons.Style.font.caption
    text: scr.periodData
      ? (scr.providerFilter !== "" ? scr.providerLabel(scr.providerFilter, "") + " · " : "")
        + (scr.metric === "cost"
          ? "$" + scr.filteredCost.toFixed(2) + " · " + scr.filteredSessions + " sessions"
          : scr.filteredTokens.toLocaleString(Qt.locale("en_US")) + " tokens · " + scr.filteredSessions + " sessions")
      : "No usage data yet. Open T3 Code to collect usage."
  }

  Flow {
    visible: scr.metric !== "limits" && scr.allProviders.length > 0
    width: scr.width
    spacing: Commons.Style.spacing.xxs
    Repeater {
      model: scr.filterOptions
      delegate: Ui.Button {
        required property var modelData
        text: modelData.label
        selected: scr.providerFilter === modelData.kind
        fontSize: Commons.Style.font.caption
        horizontalPadding: Commons.Style.spacing.sm
        verticalPadding: Commons.Style.spacing.xxs
        onClicked: scr.selectProvider(modelData.kind)
      }
    }
  }

  Canvas {
    id: chart
    visible: scr.metric !== "limits" && scr.periodData !== null
    width: scr.width
    height: 150

    onPaint: {
      var ctx = getContext("2d")
      ctx.reset()
      var slots = scr.timeline
      if (slots.length === 0) return
      var providers = scr.activeProviders
      var maxV = 0
      var j
      for (j = 0; j < slots.length; j++) maxV = Math.max(maxV, scr.slotTotal(slots[j]))
      if (maxV <= 0) maxV = 1
      var gap = slots.length > 40 ? 1 : 2
      var barW = (width - (slots.length - 1) * gap) / slots.length
      ctx.strokeStyle = Qt.rgba(scr.foreground.r, scr.foreground.g, scr.foreground.b, 0.16)
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(0, height - 0.5)
      ctx.lineTo(width, height - 0.5)
      ctx.stroke()

      for (j = 0; j < slots.length; j++) {
        var x = j * (barW + gap)
        var selected = scr.inspectionIndex === j
        if (selected) {
          ctx.fillStyle = Qt.rgba(scr.foreground.r, scr.foreground.g, scr.foreground.b, 0.07)
          ctx.fillRect(x - gap / 2, 0, barW + gap, height)
        }
        var base = height - 1
        if (providers.length === 0) {
          var legacyValue = scr.metricValue(slots[j])
          var legacyH = legacyValue > 0 ? Math.max(1, legacyValue / maxV * (height - 8)) : 0
          ctx.fillStyle = scr.accent
          ctx.fillRect(x, base - legacyH, barW, legacyH)
        } else {
          for (var p = 0; p < providers.length; p++) {
            var value = scr.metricValue(scr.slotProvider(slots[j], providers[p].kind))
            if (value <= 0) continue
            var h = value / maxV * (height - 8)
            base -= h
            ctx.fillStyle = scr.providerColor(providers[p].kind)
            ctx.globalAlpha = selected || scr.inspectionIndex < 0 ? 0.94 : 0.58
            ctx.fillRect(x, base, barW, Math.max(0.6, h))
          }
          ctx.globalAlpha = 1
        }
        if (selected) {
          ctx.strokeStyle = scr.foreground
          ctx.lineWidth = 1
          ctx.strokeRect(x - 0.5, 0.5, barW + 1, height - 1)
          ctx.beginPath()
          ctx.moveTo(x + barW / 2, 0)
          ctx.lineTo(x + barW / 2, height)
          ctx.stroke()
        }
      }
    }

    onWidthChanged: requestPaint()
    Connections {
      target: scr
      function onMetricChanged() { chart.requestPaint() }
      function onPeriodChanged() { chart.requestPaint() }
      function onPeriodDataChanged() { chart.requestPaint() }
      function onProviderFilterChanged() { chart.requestPaint() }
      function onInspectionIndexChanged() { chart.requestPaint() }
    }

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      onEntered: scr.inspectAt(mouseX, mouseY)
      onPositionChanged: function(mouse) { scr.inspectAt(mouse.x, mouse.y) }
      onExited: scr.clearInspection()
    }

    // Chart tooltip card: edge-clamped, flipped near edge
    Rectangle {
      id: chartTooltip
      visible: scr.inspectedSlot !== null
      width: 218
      height: tooltipCol.implicitHeight + 16
      x: {
        var preferred = scr.inspectionX + 12 + width <= chart.width ? scr.inspectionX + 12 : scr.inspectionX - 12 - width
        return Math.max(0, Math.min(preferred, Math.max(0, chart.width - width)))
      }
      y: {
        var preferred = scr.inspectionY + 12 + height <= chart.height ? scr.inspectionY + 12 : scr.inspectionY - 12 - height
        return Math.max(0, Math.min(preferred, Math.max(0, chart.height - height)))
      }
      z: 10
      radius: Commons.Style.cornerRadius
      color: Commons.Color.popups !== undefined ? Commons.Color.popups.background : "#181825"
      border.width: 1
      border.color: Qt.rgba(scr.foreground.r, scr.foreground.g, scr.foreground.b, 0.28)

      Column {
        id: tooltipCol
        anchors.fill: parent
        anchors.margins: 8
        spacing: Commons.Style.spacing.xxs

        PlainText {
          width: parent.width
          elide: Text.ElideRight
          color: scr.foreground
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.caption
          font.bold: true
          text: scr.inspectedSlot ? scr.inspectedSlot.label + (scr.inspectionKeyboard ? " · ←/→" : "") : ""
        }
        Repeater {
          model: scr.activeProviders
          delegate: Item {
            required property var modelData
            width: parent.width
            height: 17
            Rectangle {
              width: 7
              height: 7
              radius: 3.5
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              color: scr.providerColor(modelData.kind)
            }
            PlainText {
              anchors.left: parent.left
              anchors.leftMargin: 12
              anchors.right: tooltipValue.left
              anchors.rightMargin: 6
              elide: Text.ElideRight
              color: scr.foreground
              font.family: Commons.Style.font.family
              font.pixelSize: Commons.Style.font.caption
              text: scr.providerLabel(modelData.kind, modelData.label)
            }
            PlainText {
              id: tooltipValue
              anchors.right: parent.right
              color: scr.foreground
              font.family: Commons.Style.font.family
              font.pixelSize: Commons.Style.font.caption
              text: scr.valueText(scr.metricValue(scr.slotProvider(scr.inspectedSlot, modelData.kind)))
            }
          }
        }
        Ui.PanelSeparator { foreground: scr.foreground; strength: 0.18 }
        Item {
          width: parent.width
          height: 17
          PlainText {
            anchors.left: parent.left
            color: scr.foreground
            font.family: Commons.Style.font.family
            font.pixelSize: Commons.Style.font.caption
            font.bold: true
            text: scr.providerFilter === "" ? "Total" : "Filtered total"
          }
          PlainText {
            anchors.right: parent.right
            color: scr.foreground
            font.family: Commons.Style.font.family
            font.pixelSize: Commons.Style.font.caption
            font.bold: true
            text: scr.valueText(scr.inspectionTotal)
          }
        }
      }
    }
  }

  Row {
    visible: scr.metric !== "limits" && scr.periodData !== null
    width: scr.width
    PlainText {
      id: firstLabel
      color: scr.dim
      font.family: Commons.Style.font.family
      font.pixelSize: Commons.Style.font.caption
      text: scr.timeline.length > 0 ? scr.timeline[0].label : ""
    }
    Item { height: 1; width: Math.max(0, scr.width - firstLabel.implicitWidth - lastLabel.implicitWidth) }
    PlainText {
      id: lastLabel
      color: scr.dim
      font.family: Commons.Style.font.family
      font.pixelSize: Commons.Style.font.caption
      text: scr.timeline.length > 0 ? scr.timeline[scr.timeline.length - 1].label : ""
    }
  }

  Ui.PanelSeparator {
    visible: scr.metric !== "limits" && scr.activeProviders.length > 0
    foreground: scr.foreground
  }
  Ui.PanelSectionHeader {
    visible: scr.metric !== "limits" && scr.activeProviders.length > 0
    text: "BY PROVIDER · FILTER APPLIES TO ALL USAGE SURFACES"
    foreground: scr.foreground
    fontFamily: Commons.Style.font.family
  }
  Repeater {
    model: scr.metric !== "limits" ? scr.activeProviders : []
    delegate: Column {
      id: providerRow
      required property var modelData
      width: scr.width
      spacing: Commons.Style.spacing.xxs
      Item {
        width: parent.width
        height: 18
        Rectangle {
          width: 8
          height: 8
          radius: 4
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          color: scr.providerColor(providerRow.modelData.kind)
        }
        PlainText {
          anchors.left: parent.left
          anchors.leftMargin: 14
          color: scr.foreground
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.bodySmall
          text: scr.providerLabel(providerRow.modelData.kind, providerRow.modelData.label) + " · " + providerRow.modelData.sessions + " sessions"
        }
        PlainText {
          anchors.right: parent.right
          color: scr.foreground
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.bodySmall
          font.bold: true
          text: scr.valueText(scr.metricValue(providerRow.modelData))
        }
      }
      PlainText {
        x: 14
        width: parent.width - 14
        elide: Text.ElideRight
        color: scr.dim
        font.family: Commons.Style.font.family
        font.pixelSize: Commons.Style.font.caption
        text: scr.providerDetail(providerRow.modelData)
      }
      Rectangle {
        x: 14
        width: parent.width - 14
        height: 4
        radius: 2
        color: Qt.rgba(scr.foreground.r, scr.foreground.g, scr.foreground.b, 0.10)
        Rectangle {
          width: parent.width * (scr.largestProviderValue > 0 ? scr.metricValue(providerRow.modelData) / scr.largestProviderValue : 0)
          height: parent.height
          radius: 2
          color: scr.providerColor(providerRow.modelData.kind)
        }
      }
    }
  }

  Ui.PanelSeparator {
    visible: scr.metric !== "limits" && scr.orderedModels.length > 0
    foreground: scr.foreground
  }
  Ui.PanelSectionHeader {
    visible: scr.metric !== "limits" && scr.orderedModels.length > 0
    text: "MODEL BREAKDOWN" + (scr.showAllModels ? " · ALL" : " · TOP 5")
    foreground: scr.foreground
    fontFamily: Commons.Style.font.family
  }
  Repeater {
    model: scr.metric !== "limits" ? scr.visibleModelRows : []
    delegate: Column {
      id: modelRow
      required property var modelData
      required property int index
      width: scr.width
      spacing: Commons.Style.spacing.xxs
      Item {
        width: parent.width
        height: 18
        PlainText {
          id: rank
          anchors.left: parent.left
          width: 20
          color: scr.dim
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.caption
          text: String(modelRow.index + 1).padStart(2, "0")
        }
        Rectangle {
          id: modelSwatch
          anchors.left: rank.right
          anchors.verticalCenter: parent.verticalCenter
          width: 7
          height: 7
          radius: 3.5
          color: scr.providerColor(modelRow.modelData.kind)
        }
        PlainText {
          anchors.left: modelSwatch.right
          anchors.leftMargin: 7
          anchors.right: modelValue.left
          anchors.rightMargin: 8
          elide: Text.ElideMiddle
          color: scr.foreground
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.bodySmall
          text: modelRow.modelData.name
        }
        PlainText {
          id: modelValue
          anchors.right: parent.right
          color: scr.modelUnpriced(modelRow.modelData) && scr.metric === "cost" ? scr.dim : scr.foreground
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.bodySmall
          font.bold: true
          text: scr.modelPrimaryText(modelRow.modelData)
        }
      }
      PlainText {
        x: 34
        width: parent.width - 34
        elide: Text.ElideRight
        color: scr.dim
        font.family: Commons.Style.font.family
        font.pixelSize: Commons.Style.font.caption
        text: scr.providerLabel(modelRow.modelData.kind, "") + " · " + scr.modelDetail(modelRow.modelData)
      }
      Rectangle {
        x: 34
        width: parent.width - 34
        height: 4
        radius: 2
        visible: scr.modelBarShare(modelRow.modelData) > 0
        color: Qt.rgba(scr.foreground.r, scr.foreground.g, scr.foreground.b, 0.10)
        Rectangle {
          width: parent.width * scr.modelBarShare(modelRow.modelData)
          height: parent.height
          radius: 2
          color: scr.providerColor(modelRow.modelData.kind)
        }
      }
    }
  }
  Ui.Button {
    visible: scr.metric !== "limits" && scr.orderedModels.length > 5
    text: scr.showAllModels ? "Show fewer" : "Show all " + scr.orderedModels.length + " models"
    fontSize: Commons.Style.font.caption
    horizontalPadding: Commons.Style.spacing.md
    verticalPadding: Commons.Style.spacing.xxs
    onClicked: scr.toggleAllModels()
  }

  PlainText {
    visible: scr.metric !== "limits" && scr.periodData !== null
    width: scr.width
    elide: Text.ElideRight
    color: scr.dim
    font.family: Commons.Style.font.family
    font.pixelSize: Commons.Style.font.caption
    text: "Hover bars or press ←/→ · f next provider · a all models · ↑/↓ scroll"
  }

  // ---- Limits ----
  Column {
    visible: scr.metric === "limits"
    width: scr.width
    spacing: Commons.Style.spacing.md

    Repeater {
      model: scr.limitsData && scr.limitsData.groups ? scr.limitsData.groups : []

      delegate: Column {
        id: group
        required property var modelData
        width: parent.width
        spacing: Commons.Style.spacing.sm

        PlainText {
          color: Qt.darker(scr.foreground, 1.4)
          font.family: Commons.Style.font.family
          font.pixelSize: Commons.Style.font.caption
          font.bold: true
          text: (group.modelData.label || group.modelData.driver).toUpperCase()
        }

        Repeater {
          model: group.modelData.rows

          delegate: Column {
            id: limitRow
            required property var modelData
            width: parent.width
            spacing: Commons.Style.spacing.xxs

            Item {
              width: parent.width
              PlainText {
                anchors.left: parent.left
                color: scr.foreground
                font.family: Commons.Style.font.family
                font.pixelSize: Commons.Style.font.bodySmall
                text: limitRow.modelData.label
              }
              PlainText {
                anchors.right: parent.right
                color: scr.dim
                font.family: Commons.Style.font.family
                font.pixelSize: Commons.Style.font.caption
                text: limitRow.modelData.remainingPercent + "% left · " + limitRow.modelData.pace + " · " + limitRow.modelData.resetAt
              }
            }
            Rectangle {
              width: parent.width
              height: 6
              radius: 3
              color: Qt.rgba(scr.foreground.r, scr.foreground.g, scr.foreground.b, 0.12)
              Rectangle {
                width: parent.width * limitRow.modelData.usedPercent / 100
                height: parent.height
                radius: 3
                color: limitRow.modelData.pace === "ahead" ? scr.urgent : scr.accent
                Behavior on width {
                  NumberAnimation { duration: Commons.Style.duration(220); easing.type: Easing.Bezier; easing.bezierCurve: Commons.Style.easeOut }
                }
              }
            }
          }
        }
      }
    }

    Repeater {
      model: scr.limitsData && scr.limitsData.links ? scr.limitsData.links : []
      delegate: Ui.Button {
        required property var modelData
        text: modelData.label + " ↗"
        fontSize: Commons.Style.font.caption
        horizontalPadding: Commons.Style.spacing.md
        verticalPadding: Commons.Style.spacing.xxs
        onClicked: scr.linkOpenRequested(modelData.url)
      }
    }
  }
}
