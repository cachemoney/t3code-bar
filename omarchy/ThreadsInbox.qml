import QtQuick
import qs.Commons as Commons
import qs.Ui as Ui

Item {
  id: root

  property var threads: []
  property color foreground: "#cdd6f4"
  property color dim: "#89b4fa"
  property color accent: "#7dc4e4"
  property color urgent: "#f7768e"
  property string fontFamily: Commons.Style.font.family

  property int cursor: 0
  property bool cursorActive: false
  property bool previewsEnabled: true

  property bool previewVisible: false
  property string previewText: ""
  property string previewUpdatedAt: ""
  property double hoverRowY: 0

  signal pointAt(int index)
  signal rowHovered(string id, double rowY)
  signal rowExited()
  signal actionRequested(string id, string action)
  signal newThreadRequested()

  implicitWidth: parent ? parent.width : 0
  implicitHeight: mainCol.implicitHeight

  component PlainText: Text { textFormat: Text.PlainText }

  function projectFill(title) {
    var h = 0
    var str = String(title || "?")
    for (var i = 0; i < str.length; i++) h = (h * 31 + str.charCodeAt(i)) % 360
    return Qt.hsla(h / 360, 0.5, 0.5, 0.55)
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

  function isAttentionStatus(status) {
    return ["Approval", "Input", "Request", "Limited", "Failed", "Woke"].indexOf(status) >= 0
  }

  function relTime(iso) {
    if (!iso) return ""
    var then = Date.parse(iso)
    if (isNaN(then)) return ""
    var s = Math.max(0, (Date.now() - then) / 1000)
    if (s < 60) return Math.floor(s) + "s"
    if (s < 3600) return Math.floor(s / 60) + "m"
    if (s < 86400) return Math.floor(s / 3600) + "h"
    return Math.floor(s / 86400) + "d"
  }

  // Elapsed time since `iso`, desktop "duration" format: 12s · 5m · 3h 12m
  function durationSince(iso) {
    if (!iso) return ""
    var s = Math.max(0, Math.floor((Date.now() - Date.parse(iso)) / 1000))
    if (isNaN(s)) return ""
    if (s < 60) return s + "s"
    if (s < 3600) return Math.floor(s / 60) + "m"
    return Math.floor(s / 3600) + "h " + (Math.floor(s / 60) % 60) + "m"
  }

  // Time remaining until `iso`, desktop "until" format: 12m · 5h · 3d
  function untilStr(iso) {
    if (!iso) return ""
    var minutes = Math.ceil((Date.parse(iso) - Date.now()) / 60000)
    if (isNaN(minutes) || minutes <= 0) return ""
    if (minutes < 60) return minutes + "m"
    var hours = Math.ceil(minutes / 60)
    return hours < 24 ? hours + "h" : Math.ceil(hours / 24) + "d"
  }

  // Desktop suffix: run duration while working, reset countdown when
  // limited, elapsed since completion while waiting.
  function statusSuffix(t) {
    if (!t) return ""
    if (t.status === "Working" || t.status === "Goal") return durationSince(t.activeRunStartedAt)
    if (t.status === "Limited") return untilStr(t.usageLimitResetAt)
    if (t.status === "Waiting") return durationSince(t.latestRunCompletedAt)
    return ""
  }

  // Desktop status line: relative time only once settled; otherwise the
  // attention state with its suffix (Limited suffix reads "· 2h").
  function statusLine(t) {
    if (!t) return ""
    if (!t.status || t.status === "Settled") return relTime(t.latestUserMessageAt || t.updatedAt)
    var suffix = statusSuffix(t)
    if (suffix === "") return t.status
    return t.status === "Limited" ? t.status + " · " + suffix : t.status + " " + suffix
  }

  function fileUrl(path) {
    if (!path) return ""
    if (Commons && Commons.Util && Commons.Util.fileUrl) {
      return Commons.Util.fileUrl(path)
    }
    return "file://" + path
  }

  Column {
    id: mainCol
    width: parent.width
    spacing: Commons.Style.spacing.xs

    // Top Header with Thread Count and New Thread action
    Item {
      width: parent.width
      height: 28

      PlainText {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Commons.Style.font.caption
        text: root.threads.length + " THREADS"
      }

      Ui.Button {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "+ New Thread"
        fontSize: Commons.Style.font.caption
        horizontalPadding: Commons.Style.spacing.sm
        verticalPadding: 2
        onClicked: root.newThreadRequested()
      }
    }

    // Threads list
    Column {
      id: listCol
      width: parent.width
      spacing: Commons.Style.spacing.xxs

      Repeater {
        model: root.threads

        delegate: Item {
          id: row
          required property var modelData
          required property int index

          readonly property bool hasCursor: mouse.containsMouse || (root.cursorActive && root.cursor === index)
          width: listCol.width
          implicitHeight: Commons.Style.spacing.popupRowHeight * 1.85

          Ui.CursorSurface {
            anchors.fill: parent
            hasCursor: row.hasCursor
          }

          MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onPositionChanged: root.pointAt(row.index)
            onEntered: {
              root.pointAt(row.index)
              var mappedY = row.mapToItem(root, 0, 0).y
              root.rowHovered(row.modelData.id, mappedY)
            }
            onExited: root.rowExited()
            onClicked: function(mouseEvent) {
              if (mouseEvent.button === Qt.RightButton) {
                root.rowExited()
              } else {
                root.actionRequested(row.modelData.id, "open")
              }
            }
          }

          Column {
            anchors.fill: parent
            anchors.leftMargin: Commons.Style.spacing.sm
            anchors.rightMargin: Commons.Style.spacing.sm
            anchors.topMargin: Commons.Style.spacing.xxs
            anchors.bottomMargin: Commons.Style.spacing.xxs
            spacing: Commons.Style.spacing.xxs

            // Line 1: Semantic status dot + title + relative timestamp + attention pill + quick actions
            Item {
              width: parent.width
              height: Commons.Style.spacing.popupRowHeight - 4

              Row {
                id: line1Left
                anchors.left: parent.left
                anchors.right: line1Right.left
                anchors.rightMargin: Commons.Style.spacing.xs
                anchors.verticalCenter: parent.verticalCenter
                spacing: Commons.Style.spacing.xs

                Rectangle {
                  anchors.verticalCenter: parent.verticalCenter
                  width: 7
                  height: 7
                  radius: 3.5
                  color: root.statusColor(row.modelData.status)
                }

                PlainText {
                  anchors.verticalCenter: parent.verticalCenter
                  width: Math.max(60, line1Left.width - 20)
                  elide: Text.ElideRight
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Commons.Style.font.bodySmall
                  font.bold: row.modelData.isPinned === true
                  text: (row.modelData.isPinned ? "» " : "") + (row.modelData.title || "Untitled")
                }
              }

              Row {
                id: line1Right
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: Commons.Style.spacing.xs

                // Quick actions toolbar visible on row cursor
                Row {
                  visible: row.hasCursor
                  anchors.verticalCenter: parent.verticalCenter
                  spacing: 4

                  Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    height: 18
                    radius: 3
                    width: appBtnText.implicitWidth + 8
                    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
                    PlainText {
                      id: appBtnText
                      anchors.centerIn: parent
                      color: root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Commons.Style.font.caption
                      text: "Open"
                    }
                    MouseArea {
                      anchors.fill: parent
                      onClicked: root.actionRequested(row.modelData.id, "open")
                    }
                  }

                  Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    height: 18
                    radius: 3
                    width: webBtnText.implicitWidth + 8
                    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
                    PlainText {
                      id: webBtnText
                      anchors.centerIn: parent
                      color: root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Commons.Style.font.caption
                      text: "Web"
                    }
                    MouseArea {
                      anchors.fill: parent
                      onClicked: root.actionRequested(row.modelData.id, "browser")
                    }
                  }

                  Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    height: 18
                    radius: 3
                    width: copyBtnText.implicitWidth + 8
                    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
                    PlainText {
                      id: copyBtnText
                      anchors.centerIn: parent
                      color: root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Commons.Style.font.caption
                      text: "Copy"
                    }
                    MouseArea {
                      anchors.fill: parent
                      onClicked: root.actionRequested(row.modelData.id, "copy")
                    }
                  }
                }

                // Attention pill
                Rectangle {
                  visible: root.isAttentionStatus(row.modelData.status)
                  anchors.verticalCenter: parent.verticalCenter
                  height: 16
                  radius: 3
                  width: attentionLabel.implicitWidth + 8
                  color: Qt.rgba(statusCol.r, statusCol.g, statusCol.b, 0.18)
                  border.width: 1
                  border.color: Qt.rgba(statusCol.r, statusCol.g, statusCol.b, 0.4)
                  readonly property color statusCol: root.statusColor(row.modelData.status)

                  PlainText {
                    id: attentionLabel
                    anchors.centerIn: parent
                    color: parent.statusCol
                    font.family: root.fontFamily
                    font.pixelSize: Commons.Style.font.caption
                    font.bold: true
                    text: String(row.modelData.status || "").toUpperCase()
                  }
                }

                // Status line: relative time once settled, otherwise the
                // attention state with run/reset/wait suffix (desktop parity).
                // Attention states already name themselves in the pill above,
                // so only their suffix lands here.
                PlainText {
                  anchors.verticalCenter: parent.verticalCenter
                  color: row.modelData.status && row.modelData.status !== "Settled" && !root.isAttentionStatus(row.modelData.status)
                    ? root.statusColor(row.modelData.status)
                    : root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Commons.Style.font.caption
                  text: root.isAttentionStatus(row.modelData.status)
                    ? root.statusSuffix(row.modelData)
                    : root.statusLine(row.modelData)
                }
              }
            }

            // Line 2: Project badge/favicon + branch chip + provider label + model name + PR number
            Row {
              width: parent.width
              spacing: Commons.Style.spacing.xs

              // Project favicon or initial badge
              Item {
                anchors.verticalCenter: parent.verticalCenter
                width: 14
                height: 14

                Image {
                  id: favIcon
                  anchors.fill: parent
                  fillMode: Image.PreserveAspectFit
                  source: root.fileUrl(row.modelData.projectFavicon)
                  visible: !!row.modelData.projectFavicon && status === Image.Ready
                }

                Rectangle {
                  visible: !favIcon.visible
                  anchors.fill: parent
                  radius: 3
                  color: root.projectFill(row.modelData.projectTitle || "?")
                  PlainText {
                    anchors.centerIn: parent
                    color: "#ffffff"
                    font.family: root.fontFamily
                    font.pixelSize: Commons.Style.font.caption - 1
                    font.bold: true
                    text: String(row.modelData.projectTitle || "?").substring(0, 1).toUpperCase()
                  }
                }
              }

              // Git branch chip
              Rectangle {
                visible: !!row.modelData.branch
                anchors.verticalCenter: parent.verticalCenter
                height: 14
                radius: 3
                width: branchText.implicitWidth + 8
                color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)

                PlainText {
                  id: branchText
                  anchors.centerIn: parent
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Commons.Style.font.caption
                  text: "± " + (row.modelData.branch || "")
                }
              }

              // Provider label
              PlainText {
                visible: !!(row.modelData.providers && row.modelData.providers.length > 0)
                anchors.verticalCenter: parent.verticalCenter
                color: root.accent
                font.family: root.fontFamily
                font.pixelSize: Commons.Style.font.caption
                text: row.modelData.providers ? row.modelData.providers.join(", ") : ""
              }

              // Model name
              PlainText {
                visible: !!row.modelData.modelName
                anchors.verticalCenter: parent.verticalCenter
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Commons.Style.font.caption
                text: row.modelData.modelName || ""
              }

              // PR number
              PlainText {
                visible: row.modelData.prNumber !== null && row.modelData.prNumber !== undefined
                anchors.verticalCenter: parent.verticalCenter
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Commons.Style.font.caption
                text: "#" + row.modelData.prNumber
              }
            }
          }
        }
      }
    }
  }

  // Anchored Hover Preview Card overlay beside hovered row
  Rectangle {
    id: previewCard
    visible: root.previewVisible && root.previewsEnabled
    width: 250
    height: Math.min(220, previewText.implicitHeight + 28)
    // Anchored beside hovered row, clamped/flipped to stay cleanly within panel bounds
    x: Math.max(Commons.Style.spacing.xs, Math.min(root.width - width - Commons.Style.spacing.xs, root.width - width - Commons.Style.spacing.xs))
    y: Math.max(Commons.Style.spacing.xs, Math.min(Math.max(Commons.Style.spacing.xs, root.height - height - Commons.Style.spacing.xs), root.hoverRowY - height / 3))
    z: 100
    radius: Commons.Style.cornerRadius
    color: "#181825"
    border.width: 1
    border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.28)

    Column {
      anchors.fill: parent
      anchors.margins: 8
      spacing: 4

      Row {
        width: parent.width
        PlainText {
          text: "PREVIEW"
          font.family: root.fontFamily
          font.pixelSize: Commons.Style.font.caption
          font.bold: true
          color: root.accent
        }
        Item {
          width: Math.max(4, parent.width - 120)
          height: 1
        }
        PlainText {
          text: root.relTime(root.previewUpdatedAt)
          font.family: root.fontFamily
          font.pixelSize: Commons.Style.font.caption
          color: root.dim
        }
      }

      PlainText {
        id: previewText
        width: parent.width
        wrapMode: Text.WordWrap
        maximumLineCount: 6
        elide: Text.ElideRight
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Commons.Style.font.caption
        text: root.previewText
      }
    }

    Behavior on opacity {
      NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
    }
  }
}
