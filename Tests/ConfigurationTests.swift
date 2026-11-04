import Foundation
import Testing
@testable import T3CodeBar

struct ConfigurationTests {
    @Test func customDataDirectories() {
        let home = URL(fileURLWithPath: "/tmp/t3bar-home")
        #expect(EnvironmentConfig(environment: [:], home: home).baseDir.path == "/tmp/t3bar-home/.t3/userdata")
        #expect(EnvironmentConfig(environment: ["T3CODE_HOME": "/tmp/custom"]).baseDir.path == "/tmp/custom/userdata")
        #expect(EnvironmentConfig(environment: ["T3CODE_HOME": "/tmp/custom", "T3_CODE_BAR_STATE_DIR": "/tmp/dev"]).baseDir.path == "/tmp/dev")
    }

    @Test func runtimePortAndIdentifierEncoding() throws {
        let fixture = try DatabaseFixture()
        try "env /#%?".write(to: fixture.config.environmentIdUrl, atomically: true, encoding: .utf8)
        try #"{"port":4123,"origin":"http://192.168.1.5:4123","pid":0}"#.write(to: fixture.config.runtimeJsonUrl, atomically: true, encoding: .utf8)
        let deepLink = try fixture.config.threadURL(threadId: "thread/#%?")
        #expect(deepLink.absoluteString == "t3code://threads/env%20%2F%23%25%3F/thread%2F%23%25%3F")
        let browserLink = try fixture.config.threadURL(threadId: "thread/#%?", browser: true)
        #expect(browserLink.host == "127.0.0.1")
        #expect(browserLink.port == 4123)
        #expect(browserLink.query == nil)
        #expect(browserLink.fragment == nil)
        #expect(!fixture.config.isServerOnline)
    }

    @Test func missingConfigurationDoesNotInventEnvironmentOrPort() throws {
        let fixture = try DatabaseFixture()
        #expect(throws: ConfigurationError.missingEnvironment) { try fixture.config.threadURL(threadId: "t") }
        try "env".write(to: fixture.config.environmentIdUrl, atomically: true, encoding: .utf8)
        #expect(throws: ConfigurationError.missingRuntime) { try fixture.config.threadURL(threadId: "t", browser: true) }
    }

    @Test(arguments: [0, -1, 65536])
    func invalidPortIsRejected(_ port: Int) throws {
        let fixture = try DatabaseFixture()
        try "env".write(to: fixture.config.environmentIdUrl, atomically: true, encoding: .utf8)
        try "{\"port\":\(port)}".write(to: fixture.config.runtimeJsonUrl, atomically: true, encoding: .utf8)
        #expect(throws: ConfigurationError.missingRuntime) { try fixture.config.threadURL(threadId: "t", browser: true) }
    }

    @Test func missingPortCanBeReadFromRuntimeOrigin() throws {
        let fixture = try DatabaseFixture()
        try "env".write(to: fixture.config.environmentIdUrl, atomically: true, encoding: .utf8)
        try #"{"origin":"http://0.0.0.0:4888"}"#.write(to: fixture.config.runtimeJsonUrl, atomically: true, encoding: .utf8)
        #expect(try fixture.config.threadURL(threadId: "t", browser: true).absoluteString == "http://127.0.0.1:4888/env/t")
    }
}
