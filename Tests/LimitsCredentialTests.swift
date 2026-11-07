import Foundation
import Testing
@testable import T3CodeBar

struct LimitsCredentialTests {
    @Test func bearerCredentialUsesTokenAndPort() {
        let credential = LimitsCredential.bearer(port: 43110, token: "session-token")
        #expect(credential.port == 43110)
        #expect(credential.token == "session-token")
        #expect(credential.expiresAt == .distantFuture)
    }

    @Test func jwtExpiryIsReadInMilliseconds() {
        let payload = "{\"exp\":4102444800000}"
        let encoded = Data(payload.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        let expiry = LimitsCredential.bearerExpiry("\(encoded).signature")
        #expect(expiry == Date(timeIntervalSince1970: 4102444800))
    }
}
