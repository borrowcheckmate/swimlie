import AVFoundation
import WatchKit

/// Pitched rest beeps: WKInterfaceDevice sounds cannot be pitched, so the countdown marks are also heard as tones.
@MainActor enum Beeper {
    private static let tickPlayer = player(hz: 880, seconds: 0.12)
    private static let goPlayer = player(hz: 1320, seconds: 0.4)

    /// Sets the audio category and readies both players, so the first beep has no latency. A failure leaves the beeps silent.
    static func prepare() {
        try? AVAudioSession.sharedInstance().setCategory(.playback, options: [.mixWithOthers])
        try? AVAudioSession.sharedInstance().setActive(true)
        tickPlayer?.prepareToPlay()
        goPlayer?.prepareToPlay()
    }

    // The haptic is the reliable cue: with the wrist down the app is not frontmost and the speaker may stay silent.
    static func tick() { WKInterfaceDevice.current().play(.click); play(tickPlayer) }
    static func go() { WKInterfaceDevice.current().play(.notification); play(goPlayer) }

    private static func play(_ p: AVAudioPlayer?) {
        p?.currentTime = 0
        p?.play()
    }

    /// 16-bit mono 22050 Hz sine WAV with a 5 ms fade in and out (no clicks), built in memory.
    private static func player(hz: Double, seconds: Double) -> AVAudioPlayer? {
        let rate = 22050
        let n = Int(Double(rate) * seconds)
        let fade = Double(rate / 200) // 5 ms
        let samples = (0..<n).map { i -> Int16 in
            let env = min(1, Double(i) / fade, Double(n - i) / fade)
            return Int16(sin(2 * .pi * hz * Double(i) / Double(rate)) * env * 0.6 * Double(Int16.max))
        }
        let pcm = samples.withUnsafeBufferPointer { Data(buffer: $0) }
        var wav = Data("RIFF".utf8)
        func put<T: FixedWidthInteger>(_ v: T) { withUnsafeBytes(of: v.littleEndian) { wav.append(contentsOf: $0) } }
        put(UInt32(36 + pcm.count)); wav.append(Data("WAVEfmt ".utf8))
        put(UInt32(16)); put(UInt16(1)); put(UInt16(1)); put(UInt32(rate)); put(UInt32(rate * 2)); put(UInt16(2)); put(UInt16(16))
        wav.append(Data("data".utf8)); put(UInt32(pcm.count)); wav.append(pcm)
        return try? AVAudioPlayer(data: wav)
    }
}
