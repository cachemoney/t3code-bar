import Foundation
import Combine

enum LimitsLiveStatus: Equatable {
    case cacheOnly, connecting, live, paused, retrying, failed(String)
    var text: String {
        switch self {
        case .cacheOnly: "Connect to view usage."
        case .connecting: "Connecting…"
        case .live: "Connected"
        case .paused: "Connection paused."
        case .retrying: "Disconnected. Reconnecting…"
        case .failed(let message): message
        }
    }
}

/// A user-started, read-only subscription. T3 may run its own discovery when clients subscribe.
@MainActor
final class LimitsLiveConnection: ObservableObject {
    @Published private(set) var status: LimitsLiveStatus = .cacheOnly
    @Published private(set) var data: LimitsLiveData?
    @Published private(set) var lastUpdatedAt: Date?
    var onChange: (() -> Void)?
    private let transport: any LimitsTransport
    private let store: any LimitsCredentialStore
    private let clock: @Sendable () -> Date
    private let retryDelay: Duration
    private var credential: LimitsCredential?
    private var task: Task<Void, Never>?
    private var activeSocket: (any LimitsSocket)?
    private var generation = 0
    private var lastFrame = Date.distantPast
    private var hasSnapshot = false

    var isConnected: Bool { status == .live }

    init(transport: any LimitsTransport = LimitsURLTransport(), store: any LimitsCredentialStore = LimitsKeychainStore(),
         clock: @escaping @Sendable () -> Date = { Date() }, retryDelay: Duration = .seconds(3)) {
        self.transport = transport
        self.store = store
        self.clock = clock
        self.retryDelay = retryDelay
    }

    /// Connects with a T3 CLI session token and stores it for future launches.
    func connect(port: Int, bearerToken: String) {
        stop()
        credential = nil
        data = nil
        lastUpdatedAt = nil
        do {
            _ = try LimitsEndpoint(port: port)
            let token = bearerToken.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !token.isEmpty, token.utf8.count <= 16_384 else { throw LimitsConnectionError.invalidToken }
            let grant = LimitsCredential.bearer(port: port, token: token)
            try store.save(grant)
            credential = grant
            resume()
        } catch {
            status = .failed((error as? LimitsConnectionError)?.message ?? LimitsConnectionError.storage.message)
        }
        onChange?()
    }

    /// Runs the locally installed T3 CLI and saves the resulting session token.
    func autoGenerateToken(port: Int) {
        stop()
        status = .connecting
        onChange?()
        task = Task { [weak self] in
            do {
                let token = try await Task.detached(priority: .userInitiated) {
                    try Self.issueTokenWithCLI()
                }.value
                guard let self, !Task.isCancelled else { return }
                self.connect(port: port, bearerToken: token)
            } catch {
                guard let self, !Task.isCancelled else { return }
                self.task = nil
                self.status = .failed(error.localizedDescription)
                self.onChange?()
            }
        }
    }

    private nonisolated static func issueTokenWithCLI() throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["t3", "auth", "session", "issue", "--scope", "orchestration:read", "--ttl", "3650d", "--token-only"]
        let output = Pipe()
        let errors = Pipe()
        process.standardOutput = output
        process.standardError = errors
        try process.run()
        process.waitUntilExit()
        let token = String(data: output.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard process.terminationStatus == 0, !token.isEmpty, token.utf8.count <= 16_384 else {
            let message = String(data: errors.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw NSError(domain: "T3CLI", code: Int(process.terminationStatus), userInfo: [
                NSLocalizedDescriptionKey: message?.isEmpty == false ? message! : "Could not generate an access token with the T3 CLI."
            ])
        }
        return token
    }

    /// Explicit button action: no Keychain lookup or network request occurs just by opening Limits.
    func reconnectSaved(port: Int) {
        stop()
        credential = nil
        do {
            _ = try LimitsEndpoint(port: port)
            guard let saved = try store.load() else {
                status = .failed("No saved access token. Generate or enter one to connect.")
                return
            }
            guard saved.port == port else { throw LimitsConnectionError.destination }
            _ = try LimitsEndpoint(port: saved.port)
            guard saved.expiresAt > clock() else { throw LimitsConnectionError.expired }
            credential = saved
            resume()
        } catch {
            status = .failed((error as? LimitsConnectionError)?.message ?? LimitsConnectionError.storage.message)
        }
    }

    /// Called by the app at launch. Missing credentials keep the onboarding state instead of
    /// showing an error; an expired or revoked credential must be explained to the user.
    func reconnectSavedIfAvailable(port: Int) {
        guard status != .live, status != .connecting else { return }
        do {
            _ = try LimitsEndpoint(port: port)
            guard let saved = try store.load() else { return }
            guard saved.port == port else { throw LimitsConnectionError.destination }
            guard saved.expiresAt > clock() else { throw LimitsConnectionError.expired }
            credential = saved
            resume()
        } catch {
            status = .failed((error as? LimitsConnectionError)?.message ?? LimitsConnectionError.storage.message)
            onChange?()
        }
    }

    func resume() {
        guard task == nil, let credential else { return }
        generation += 1
        let run = generation
        status = .connecting
        task = Task { [weak self] in await self?.runConnection(credential, generation: run) }
    }

    func stop() {
        generation += 1
        task?.cancel()
        task = nil
        let socket = activeSocket
        activeSocket = nil
        if let socket {
            Task {
                // Bound the best-effort Interrupt so a broken send cannot keep the stream open.
                let deadline = Task {
                    do { try await Task.sleep(for: .seconds(1)) } catch { return }
                    await socket.close()
                }
                defer { deadline.cancel() }
                try? await socket.send(LimitsRPC.encode(LimitsRPC.interrupt))
                await socket.close()
            }
        }
        if credential != nil || status == .connecting { status = .paused }
    }

    func disconnect() {
        stop()
        credential = nil
        data = nil
        lastUpdatedAt = nil
        do { try store.delete(); status = .cacheOnly }
        catch { status = .failed(LimitsConnectionError.storage.message) }
        onChange?()
    }

    private func http(_ request: URLRequest, unauthorized: LimitsConnectionError? = nil) async throws -> [String: Any] {
        let (body, status) = try await transport.request(request)
        guard status == 200 else {
            if let unauthorized, status == 401 || status == 403 { throw unauthorized }
            if (400...499).contains(status) || (300...399).contains(status) { throw LimitsConnectionError.authorization }
            throw URLError(.badServerResponse)
        }
        guard body.count < 1_048_576,
              let object = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
            throw LimitsConnectionError.protocolMismatch
        }
        return object
    }

    private func runConnection(_ credential: LimitsCredential, generation run: Int) async {
        defer { if generation == run { task = nil } }
        while !Task.isCancelled, generation == run {
            var socket: (any LimitsSocket)?
            var heartbeat: Task<Void, Never>?
            do {
                guard credential.expiresAt > clock() else { throw LimitsConnectionError.expired }
                let endpoint = try LimitsEndpoint(port: credential.port)
                var request = URLRequest(url: endpoint.url("/api/auth/websocket-ticket/rpc"))
                request.httpMethod = "POST"
                request.setValue("Bearer \(credential.token)", forHTTPHeaderField: "Authorization")
                let issued = try await http(request, unauthorized: .expired)
                guard let ticket = issued["ticket"] as? String, !ticket.isEmpty,
                      let expires = issued["expiresAt"] as? String, let expiry = LimitsDate.parse(expires), expiry > clock() else {
                    throw LimitsConnectionError.protocolMismatch
                }
                try Task.checkCancellation()
                guard generation == run else { return }
                let connected = await transport.socket(endpoint.url("/ws", ticket: ticket))
                guard !Task.isCancelled, generation == run else { await connected.close(); return }
                socket = connected
                activeSocket = connected
                hasSnapshot = false
                lastFrame = clock()
                let connectedAt = clock()
                try await connected.send(LimitsRPC.encode(LimitsRPC.subscribe))
                heartbeat = Task { [weak self] in
                    while !Task.isCancelled {
                        do { try await Task.sleep(for: .seconds(5)) } catch { return }
                        guard let self, generation == run else { return }
                        if clock() >= credential.expiresAt || clock().timeIntervalSince(lastFrame) > 15
                            || (!hasSnapshot && clock().timeIntervalSince(connectedAt) > 15) {
                            await connected.close()
                            return
                        }
                        do { try await connected.send(LimitsRPC.encode(["_tag": "Ping"])) }
                        catch { await connected.close(); return }
                    }
                }
                var next = LimitsLiveData()
                while !Task.isCancelled, generation == run {
                    let text = try await connected.receive()
                    try Task.checkCancellation()
                    guard generation == run else { break }
                    lastFrame = clock()
                    for frame in try LimitsRPC.frames(text) {
                        guard let tag = frame["_tag"] as? String else { throw LimitsConnectionError.protocolMismatch }
                        if tag == "Pong" { continue }
                        guard tag == "Chunk", LimitsRPC.matches(frame["requestId"], integer: LimitsRPC.requestID),
                              let values = frame["values"] as? [[String: Any]] else { throw LimitsConnectionError.protocolMismatch }
                        var changed = false
                        for value in values {
                            if value["type"] as? String == "snapshot" { hasSnapshot = true }
                            guard hasSnapshot else { throw LimitsConnectionError.protocolMismatch }
                            changed = try next.apply(value) || changed
                        }
                        try await connected.send(LimitsRPC.encode(LimitsRPC.ack))
                        try Task.checkCancellation()
                        guard generation == run else { return }
                        if changed {
                            data = next
                            lastUpdatedAt = clock()
                            status = .live
                            onChange?()
                        }
                    }
                }
            } catch {
                heartbeat?.cancel()
                if let socket { await socket.close() }
                guard !Task.isCancelled, generation == run else { return }
                activeSocket = nil
                if let error = error as? LimitsConnectionError {
                    status = .failed(error.message)
                    onChange?()
                    return
                }
                status = .retrying
                onChange?()
                do { try await Task.sleep(for: retryDelay) } catch { return }
            }
            heartbeat?.cancel()
            if let socket { await socket.close() }
        }
    }
}
