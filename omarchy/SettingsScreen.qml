import QtQuick
import qs.Commons as Commons
import qs.Ui as Ui

Column {
  id: scr

  width: parent ? parent.width : 0

  property var connection
  property var settings
  property color foreground
  property color dim
  property color accent

  readonly property bool previewsOn: settings ? settings.showsPreviews !== false : true
  readonly property bool mutedOn: settings ? settings.notificationsMuted === true : false
  readonly property bool isPaired: !!(connection && connection.paired)
  readonly property bool isConnected: !!(connection && connection.connected)

  signal connectRequested(string token)
  signal reconnectRequested()
  signal disconnectRequested()
  signal settingChanged(string key, bool value)

  spacing: Commons.Style.spacing.md

  component PlainText: Text { textFormat: Text.PlainText }
  component SectionHeader: PlainText {
    color: Qt.darker(scr.foreground, 1.4)
    font.family: Commons.Style.font.family
    font.pixelSize: Commons.Style.font.caption
    font.bold: true
  }

  function formatTime(iso) {
    if (!iso) return ""
    try {
      var d = new Date(iso)
      if (isNaN(d.getTime())) return ""
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    } catch (e) {
      return ""
    }
  }

  // ---- Connection Status ----
  SectionHeader { text: "CONNECTION" }

  Item {
    width: parent.width
    height: 24

    Row {
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      spacing: Commons.Style.spacing.xs

      Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: 8
        height: 8
        radius: 4
        color: scr.isConnected ? "#34D399" : (scr.isPaired ? "#FBBF24" : "#A3A3A3")
      }

      PlainText {
        anchors.verticalCenter: parent.verticalCenter
        color: scr.foreground
        font.family: Commons.Style.font.family
        font.pixelSize: Commons.Style.font.bodySmall
        text: scr.connection && scr.connection.message !== "" ? scr.connection.message : (scr.connection ? scr.connection.status : "unavailable")
      }
    }

    PlainText {
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      color: scr.dim
      font.family: Commons.Style.font.family
      font.pixelSize: Commons.Style.font.caption
      text: scr.connection ? (scr.isPaired ? ("paired · port " + scr.connection.port + (scr.connection.updatedAt ? " · " + scr.formatTime(scr.connection.updatedAt) : "")) : "not paired") : ""
    }
  }

  // ---- Pairing Controls ----
  Row {
    visible: !scr.isPaired
    width: parent.width
    spacing: Commons.Style.spacing.sm

    Ui.TextField {
      id: tokenField
      width: parent.width - connectBtn.width - Commons.Style.spacing.sm
      password: true
      placeholderText: "Pairing token..."
      foreground: scr.foreground
      accent: scr.accent
      onAccepted: {
        if (text.trim().length > 0) {
          scr.connectRequested(text.trim())
          text = ""
        }
      }
    }

    Ui.Button {
      id: connectBtn
      text: "Connect"
      fontSize: Commons.Style.font.caption
      horizontalPadding: Commons.Style.spacing.md
      verticalPadding: Commons.Style.spacing.xxs
      enabled: tokenField.text.trim().length >= 8
      onClicked: {
        if (tokenField.text.trim().length > 0) {
          scr.connectRequested(tokenField.text.trim())
          tokenField.text = ""
        }
      }
    }
  }

  Row {
    visible: scr.isPaired
    spacing: Commons.Style.spacing.sm

    Ui.Button {
      text: "Reconnect"
      fontSize: Commons.Style.font.caption
      horizontalPadding: Commons.Style.spacing.md
      verticalPadding: Commons.Style.spacing.xxs
      onClicked: scr.reconnectRequested()
    }

    Ui.Button {
      text: "Disconnect"
      fontSize: Commons.Style.font.caption
      horizontalPadding: Commons.Style.spacing.md
      verticalPadding: Commons.Style.spacing.xxs
      onClicked: scr.disconnectRequested()
    }
  }

  // ---- Preferences ----
  SectionHeader { text: "PREFERENCES" }

  Item {
    width: parent.width
    height: previewsLabel.implicitHeight + Commons.Style.spacing.xs

    PlainText {
      id: previewsLabel
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      color: scr.foreground
      font.family: Commons.Style.font.family
      font.pixelSize: Commons.Style.font.bodySmall
      text: "Hover previews"
    }

    Ui.ToggleSwitch {
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      checked: scr.previewsOn
      onToggled: {
        scr.settingChanged("showsPreviews", !checked)
      }
    }
  }

  Item {
    width: parent.width
    height: mutedLabel.implicitHeight + Commons.Style.spacing.xs

    PlainText {
      id: mutedLabel
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      color: scr.foreground
      font.family: Commons.Style.font.family
      font.pixelSize: Commons.Style.font.bodySmall
      text: "Mute notifications"
    }

    Ui.ToggleSwitch {
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      checked: scr.mutedOn
      onToggled: {
        scr.settingChanged("notificationsMuted", !checked)
      }
    }
  }

  PlainText {
    color: scr.dim
    font.family: Commons.Style.font.family
    font.pixelSize: Commons.Style.font.caption
    text: "T3 Code Bar · Omarchy QML Plugin"
  }
}
