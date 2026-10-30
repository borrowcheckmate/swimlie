import SwimlieCore
import SwiftUI

/// The recorder spike: the home list when idle, then the live page of the swim with Stop on the controls page.
struct RecorderView: View {
    @ObservedObject var recorder: Recorder
    @SwiftUI.State private var poolLength = 25
    @SwiftUI.State private var page = 1 // live; 0 = controls, to the left like LiveView
    @Environment(\.isLuminanceReduced) private var wristDown

    var body: some View {
        Group {
            if recorder.running { running } else { idle }
        }
        .themed()
        .toolbar(recorder.running ? .hidden : .visible, for: .navigationBar) // the swim takes the whole screen
    }

    private var idle: some View {
        List {
            Section {
                VStack(alignment: .leading) {
                    Text("Sensor log").font(.headline)
                    Text("Raw pool data for tuning").font(.footnote).foregroundStyle(.secondary)
                }
                Picker("Pool", selection: $poolLength) {
                    Text("25 m").tag(25)
                    Text("50 m").tag(50)
                }
                Button {
                    recorder.start(poolLength: poolLength)
                } label: {
                    Label("Start", systemImage: "play.fill").frame(maxWidth: .infinity)
                }.buttonStyle(.borderedProminent).tint(.green)
            }
            Text("Last: \(recorder.lastEvent)").font(.footnote).foregroundStyle(.secondary) // a start failure shows here
        }
        .navigationTitle("Recorder")
    }

    private var running: some View {
        TabView(selection: $page) {
            controls.tag(0)
            live.tag(1)
        }
        .tabViewStyle(.page)
    }

    private var controls: some View {
        VStack {
            Spacer(minLength: 0)
            Button("Stop") { recorder.stop() }.buttonStyle(.borderedProminent).tint(.red).font(.title3.bold())
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14).padding(.vertical, 8)
    }

    /// Full water like LiveView's swim, but there is no target: the fill stays up. Plain black in Low Power Mode and with the wrist down.
    @ViewBuilder private var live: some View {
        let content = swimming.padding(.horizontal, 14).padding(.vertical, 8).frame(maxWidth: .infinity, maxHeight: .infinity)
        if !ProcessInfo.processInfo.isLowPowerModeEnabled && !wristDown { Filled(frac: 1, color: .waterBlue) { content } } else { content }
    }

    private var swimming: some View {
        VStack(spacing: 0) {
            Spacer(minLength: 0)
            if recorder.pausedNow, let current = recorder.pauses.last { pausedLook(current) }
            else {
                Text(mmss(recorder.elapsed)).font(digits(70)).minimumScaleFactor(0.5).lineLimit(1).contentTransition(.numericText())
                setLine
            }
            Spacer(minLength: 0)
            progress
            bottom
        }
    }

    /// The set in progress, or the last one done while resting, from the gaps between laps. Hidden until the first lap.
    @ViewBuilder private var setLine: some View {
        let now = Date().timeIntervalSince(recorder.startedAt) // lap times include pauses, so wall time, not recorder.elapsed
        if let set = freeSets(laps: recorder.lapList, nowS: now).last {
            Group {
                if set.done {
                    Text("Last \(mmss(set.endS - set.startS)) · \(metres(set.lengths * UInt32(recorder.poolLength)))").foregroundStyle(.secondary)
                } else {
                    Label("Set \(mmss(now - set.startS))", systemImage: "timer").foregroundStyle(Metric.time.color)
                }
            }
            .font(.system(.title3, design: .rounded, weight: .bold).monospacedDigit()).lineLimit(1).minimumScaleFactor(0.6)
        }
    }

    /// LiveView's paused look with the live pause time, in place of the big time.
    private func pausedLook(_ current: Recorder.Pause) -> some View {
        VStack(spacing: 4) {
            Image(systemName: "pause.fill").font(.system(size: 44)).foregroundStyle(.yellow).symbolEffect(.pulse)
            Text("Paused").font(.system(.title3, design: .rounded, weight: .bold)).foregroundStyle(.yellow)
            TimelineView(.periodic(from: .now, by: 1)) { t in Text(mmss(t.date.timeIntervalSince(current.start))).font(digits(20)) }
        }
    }

    /// Metres and laps under the time; no target, so no bar.
    private var progress: some View {
        HStack {
            Text(metres(UInt32(recorder.distance))).foregroundStyle(Metric.distance.color)
            Spacer()
            Text("\(recorder.laps) laps")
        }
        .font(.system(.footnote, design: .rounded, weight: .semibold)).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.7)
    }

    /// Heart rate on the left, the pauses on the right: the bottom row of LiveView's swim.
    private var bottom: some View {
        HStack {
            HStack(spacing: 3) {
                Image(systemName: "heart.fill").symbolEffect(.pulse)
                Text(recorder.heartRate.map { "\(Int($0.rounded()))" } ?? "--").fixedSize()
            }
            .foregroundStyle(Metric.heart.color).opacity(wristDown ? 0.6 : 1)
            Spacer()
            pauseStatus
        }
        .font(.system(.footnote, design: .rounded, weight: .bold).monospacedDigit()).lineLimit(1).minimumScaleFactor(0.7)
        .padding(.horizontal, 2).padding(.bottom, 10) // clear of the rounded bottom corners
    }

    /// The completed pauses: count, total and the last one.
    @ViewBuilder private var pauseStatus: some View {
        if recorder.pausedNow {
            EmptyView() // the paused look above already shows the live pause time
        } else if let last = recorder.pauses.last {
            let n = recorder.pauses.count
            let total = recorder.pauses.reduce(0) { $0 + $1.duration }
            Text("\(n) \(n == 1 ? String(localized: "pause") : String(localized: "pauses")) · \(mmss(total)) · last \(mmss(last.duration))")
                .foregroundStyle(.secondary)
        } else {
            Text("No pauses").foregroundStyle(.secondary)
        }
    }
}
