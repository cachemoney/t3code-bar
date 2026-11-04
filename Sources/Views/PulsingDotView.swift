import SwiftUI

public struct PulsingDotView: View {
    public let color: Color
    public let shouldPulse: Bool
    public var size: CGFloat = 8

    @State private var isPulsing: Bool = false

    public init(color: Color, shouldPulse: Bool, size: CGFloat = 8) {
        self.color = color
        self.shouldPulse = shouldPulse
        self.size = size
    }

    public var body: some View {
        ZStack {
            if shouldPulse {
                // Outer breathing halo
                Circle()
                    .fill(color.opacity(0.35))
                    .frame(width: size * 2.2, height: size * 2.2)
                    .scaleEffect(isPulsing ? 1.25 : 0.85)
                    .opacity(isPulsing ? 0.8 : 0.2)

                // Mid ring
                Circle()
                    .stroke(color.opacity(0.6), lineWidth: 1.2)
                    .frame(width: size * 1.5, height: size * 1.5)
                    .scaleEffect(isPulsing ? 1.15 : 0.95)
            }

            // Core dot
            Circle()
                .fill(color)
                .frame(width: size, height: size)
                .shadow(color: shouldPulse ? color.opacity(0.6) : .clear, radius: 3)
        }
        .frame(width: size * 2.4, height: size * 2.4)
        .onAppear {
            if shouldPulse {
                withAnimation(
                    Animation.easeInOut(duration: 1.1)
                        .repeatForever(autoreverses: true)
                ) {
                    isPulsing = true
                }
            }
        }
    }
}
