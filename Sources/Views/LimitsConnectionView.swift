import SwiftUI

struct LimitsConnectionView: View {
    @ObservedObject var connection: LimitsLiveConnection
    let required: Bool
    @State private var expanded: Bool
    @State private var token = ""

    init(connection: LimitsLiveConnection, required: Bool = false) {
        self.connection = connection
        self.required = required
        _expanded = State(initialValue: required)
    }

    private var port: Int { EnvironmentConfig.shared.serverRuntime?.port ?? 43110 }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !connection.isConnected || required {
                Label(connection.status.text, systemImage: connection.status == .live ? "checkmark.circle" : "network")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                if !required {
                    Button(expanded ? "Hide connection" : "Connect T3 Code…") {
                        expanded.toggle()
                        if !expanded { token = "" }
                    }
                }
                if expanded {
                    SecureField("Access Token", text: $token)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityLabel("Access Token")
                    HStack(spacing: 8) {
                        Button("Connect") {
                            let input = token
                            token = ""
                            connection.connect(port: port, bearerToken: input)
                        }
                        .disabled(token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        Button("Auto-Generate Token via T3 CLI") {
                            token = ""
                            connection.autoGenerateToken(port: port)
                        }
                        .disabled(connection.status == .connecting)
                    }
                }
            }
        }
        .font(.system(size: 11))
        .onDisappear { token = "" }
    }
}
