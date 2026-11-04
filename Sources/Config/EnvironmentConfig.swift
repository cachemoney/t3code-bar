import Foundation

public struct ServerRuntimeInfo: Codable, Sendable {
    public let version: Int?
    public let pid: Int?
    public let host: String?
    public let port: Int?
    public let origin: String?
    public let startedAt: String?
    public let serverVersion: String?
}

public enum ConfigurationError: Error, LocalizedError {
    case missingEnvironment, missingRuntime

    public var errorDescription: String? {
        switch self {
        case .missingEnvironment: return "Environment ID not found. Open T3 Code and check the data directory."
        case .missingRuntime: return "Local server address not found. Open T3 Code before opening a browser link."
        }
    }
}

public final class EnvironmentConfig: Sendable {
    public static let shared = EnvironmentConfig()
    public let baseDir: URL
    public let stateDbUrl: URL
    public let runtimeJsonUrl: URL
    public let environmentIdUrl: URL

    public init(stateDirectory: URL? = nil, environment: [String: String] = ProcessInfo.processInfo.environment,
                home: URL = FileManager.default.homeDirectoryForCurrentUser) {
        let base: URL
        if let stateDirectory {
            base = stateDirectory
        } else if let path = environment["T3_CODE_BAR_STATE_DIR"]?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty {
            base = URL(fileURLWithPath: (path as NSString).expandingTildeInPath)
        } else if let path = environment["T3CODE_HOME"]?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty {
            base = URL(fileURLWithPath: (path as NSString).expandingTildeInPath).appendingPathComponent("userdata")
        } else {
            base = home.appendingPathComponent(".t3/userdata")
        }
        baseDir = base
        stateDbUrl = base.appendingPathComponent("statev2.sqlite")
        runtimeJsonUrl = base.appendingPathComponent("server-runtime.json")
        environmentIdUrl = base.appendingPathComponent("environment-id")
    }

    public var environmentId: String? {
        guard let data = try? Data(contentsOf: environmentIdUrl),
              let id = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              !id.isEmpty else { return nil }
        return id
    }

    public var serverRuntime: ServerRuntimeInfo? {
        guard let data = try? Data(contentsOf: runtimeJsonUrl) else { return nil }
        return try? JSONDecoder().decode(ServerRuntimeInfo.self, from: data)
    }

    /// Process presence only; this does not certify HTTP health or live background tasks.
    public var isServerOnline: Bool {
        guard let pid = serverRuntime?.pid, pid > 0, pid <= Int(Int32.max) else { return false }
        return kill(pid_t(pid), 0) == 0
    }

    public func threadURL(threadId: String, browser: Bool = false) throws -> URL {
        guard let environmentId else { throw ConfigurationError.missingEnvironment }
        // Encode each identifier as a path component, including slashes, '%' and '#'.
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-._~"))
        guard let env = environmentId.addingPercentEncoding(withAllowedCharacters: allowed),
              let thread = threadId.addingPercentEncoding(withAllowedCharacters: allowed), !thread.isEmpty else {
            throw ConfigurationError.missingEnvironment
        }
        var components = URLComponents()
        if browser {
            guard let runtime = serverRuntime else { throw ConfigurationError.missingRuntime }
            let origin = runtime.origin.flatMap { URLComponents(string: $0) }
            guard let port = runtime.port ?? origin?.port, (1...65535).contains(port) else {
                throw ConfigurationError.missingRuntime
            }
            // Always stay local, even when the server binds 0.0.0.0 or advertises a LAN origin.
            components.scheme = "http"
            components.host = "127.0.0.1"
            components.port = port
            components.percentEncodedPath = "/\(env)/\(thread)"
        } else {
            components.scheme = "t3code"
            components.host = "threads"
            components.percentEncodedPath = "/\(env)/\(thread)"
        }
        guard let url = components.url else { throw ConfigurationError.missingRuntime }
        return url
    }
}
