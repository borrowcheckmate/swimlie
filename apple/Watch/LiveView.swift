import Combine
import SwimlieCore
import SwiftUI
import WatchKit

struct LiveView: View {
    let session: any SwimSessionModel
    let css: Double?
    let onEnd: () -> Void
    @State private var page = 1 // live; 0 = controls, to the left like the Workout app
    @State private var crown = 0.0 // how far the crown is turned, -crownTravel...crownTravel
    private let crownTravel = 2.0 // tune: bigger = more turning needed; 1.0 fired too easily in the pool
    @State private var armed = true // one action per crown turn
    @State private var fromTop = false // last crown direction: the new screen comes in from there
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.isLuminanceReduced) private var wristDown
    @Environment(\.dynamicTypeSize) private var textSize
    @State private var lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled

    /// The color fill costs battery (lit pixels): plain black in Low Power Mode. With the wrist down it stays, the wave frozen.
    private var fills: Bool { !lowPower }

    var body: some View {
        TabView(selection: $page) {
            controls.tag(0)
            live.tag(1)
        }
        .tabViewStyle(.page)
        .themed()
        .toolbar(.hidden, for: .navigationBar)
    }

    // MARK: Live page: the crown drives the workout

    private var live: some View {
        ZStack {
            if let s = session.state {
                filled(s)
                    .id(key(s))
                    .transition(screenTransition(s))
            } else {
                ProgressView()
            }
        }
        .focusable()
        .digitalCrownRotation($crown, from: -crownTravel, through: crownTravel, sensitivity: .low, isContinuous: false, isHapticFeedbackEnabled: false, onChange: { _ in turned() }, onIdle: { withAnimation(reduceMotion ? nil : .snappy) { crown = 0 }; armed = true })
        .overlay(alignment: .trailing) { CrownGauge(crown: crown, travel: crownTravel) }
        .onChange(of: session.state) { old, new in
            if let o = old, let n = new { fromTop = before(n.pos, o.pos) } else { fromTop = false } // auto advances go forward
        }
        .onReceive(NotificationCenter.default.publisher(for: .NSProcessInfoPowerStateDidChange).receive(on: RunLoop.main)) { _ in
            lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled
        }
    }

    private func turned() {
        guard armed, abs(crown) >= 0.9 * crownTravel else { return } // same point where the gauge turns white
        armed = false
        if crown > 0 { up() } else { down() }
    }

    private func up() {
        if session.state?.phase == .done { return onEnd() }
        session.manualNext()
        fromTop = false
        WKInterfaceDevice.current().play(.directionUp)
    }

    private func down() {
        let ok = session.manualBack()
        fromTop = ok
        WKInterfaceDevice.current().play(ok ? .directionDown : .failure)
    }

    /// Identity of a screen: a new rep, phase or auto-pause starts a new one.
    private func key(_ s: SwimState) -> String { "\(s.pos.block).\(s.pos.round).\(s.pos.set).\(s.pos.rep) \(s.phase) \(isPaused(s))" }

    private func isPaused(_ s: SwimState) -> Bool { s.phase == .resting && s.restRemainingS == nil }

    private func screenTransition(_ s: SwimState) -> AnyTransition {
        if reduceMotion { return .opacity }
        return isPaused(s) ? .scale(scale: 0.85).combined(with: .opacity) : .push(from: fromTop ? .top : .bottom)
    }

    /// True when `a` comes earlier in the workout than `b`.
    private func before(_ a: Position, _ b: Position) -> Bool {
        [a.block, a.round, a.set, a.rep].lexicographicallyPrecedes([b.block, b.round, b.set, b.rep])
    }

    /// `age`: seconds since `s` was computed. The app's own timer can stall with the wrist down, so clocks add it themselves; awake, a second clock would restart the ring animation twice a second.
    @ViewBuilder private func screen(_ s: SwimState, age: Double) -> some View {
        switch s.phase {
        case .swimming: swimming(s, age)
        case .resting: if let left = s.restRemainingS { resting(s, max(0, left - age)) } else { paused(s) }
        case .done: done(s)
        }
    }

    /// Progress readable from the corner of the eye: a swim fills up to its goal time (orange past it, by metres without a goal). Rest and start use the circle.
    @ViewBuilder private func filled(_ s: SwimState) -> some View {
        let content = TimelineView(.periodic(from: .now, by: 1)) { ctx in screen(s, age: wristDown ? max(0, ctx.date.timeIntervalSince(session.stateDate)) : 0) }
            .padding(.horizontal, 14).padding(.vertical, 8).frame(maxWidth: .infinity, maxHeight: .infinity)
        switch s.phase {
        case .swimming where fills:
            if let g = currentSet(s)?.goalS {
                let t = s.repElapsedS ?? 0
                Filled(frac: t / Double(g), color: t > Double(g) ? .waterOrange : .waterGreen) { content }
            } else {
                Filled(frac: Double(s.doneM) / Double(max(s.targetM, 1)), color: .waterBlue) { content }
            }
        default: content
        }
    }

    private func currentSet(_ s: SwimState) -> WorkoutSet? { session.workout?.set(at: s.pos) }

    private func swimming(_ s: SwimState, _ age: Double) -> some View {
        VStack(spacing: 0) {
            if let n = session.workout?.blocks[safe: Int(s.pos.block)]?.repeat, n > 1 {
                Label("Round \(s.pos.round + 1)/\(n)", systemImage: "repeat").font(.system(.caption2, design: .rounded, weight: .bold)).foregroundStyle(Color.accent)
            }
            if let set = currentSet(s) { setTitle(set, s); DrillCue(set: set); EquipmentRow(equipment: set.equipment) }
            Spacer(minLength: 0)
            Text(mmss((s.repElapsedS ?? 0) + age)).font(digits(70)).minimumScaleFactor(0.5).lineLimit(1)
            Spacer(minLength: 0)
            progress(s, currentSet(s))
            bottom(s)
        }
    }

    /// The rest as a circle around the countdown, the last rep inside it; the set and its equipment under it.
    /// The first rep of a set says what to do: its rest and goal, and the notes.
    private func resting(_ s: SwimState, _ left: Double) -> some View {
        return VStack(spacing: 3) {
            RestCountdown(left: left, lastRep: s.lastRep, set: lastSet(s), ageYears: session.ageYears)
            if let r = s.lastRep { LastRep(result: r, set: lastSet(s), ageYears: session.ageYears, showsTime: false) }
            else if let set = currentSet(s) { RestAndGoal(set: set) }
            if let set = currentSet(s) {
                HStack(spacing: 8) { setTitle(set, s); EquipmentRow(equipment: set.equipment) }
                DrillCue(set: set)
            }
        }
    }

    /// The set of the rep just swum: its goal and intensity show under its time.
    private func lastSet(_ s: SwimState) -> WorkoutSet? { s.lastRep.flatMap { session.workout?.set(at: $0.pos) } }

    private func paused(_ s: SwimState) -> some View {
        VStack(spacing: 8) {
            Spacer(minLength: 0)
            Image(systemName: "pause.fill").font(.system(size: 44)).foregroundStyle(.yellow).symbolEffect(.pulse)
            Text("Paused").font(.system(.title3, design: .rounded, weight: .bold)).foregroundStyle(.yellow)
            if let r = s.lastRep { LastRep(result: r, set: lastSet(s), ageYears: session.ageYears) }
            Text("swim to resume").font(.footnote).foregroundStyle(.secondary)
            Spacer(minLength: 0)
        }
    }

    private func done(_ s: SwimState) -> some View {
        VStack(spacing: 6) {
            Spacer(minLength: 0)
            Image(systemName: "checkmark.circle.fill").font(.system(size: 40)).foregroundStyle(.green).symbolEffect(.bounce)
            Text("Workout done").font(.system(.headline, design: .rounded, weight: .bold))
            Text(metres(s.swumM)).font(digits(30)).minimumScaleFactor(0.5).lineLimit(1)
                .foregroundStyle(Metric.distance.color).opacity(wristDown ? 0.6 : 1)
            Label("Crown to save", systemImage: "chevron.down").font(.footnote).foregroundStyle(.secondary)
            Button("End", action: onEnd).buttonStyle(.borderedProminent).tint(.red)
            Spacer(minLength: 0)
        }
    }

    private func setTitle(_ set: WorkoutSet, _ s: SwimState) -> some View {
        HStack(spacing: 5) {
            Circle().fill(set.intensity.color).frame(width: 8, height: 8)
            Text(set.title)
            Text("\(s.pos.rep + 1)/\(set.reps)").foregroundStyle(.secondary).accessibilityIdentifier("live.rep")
        }
        .font(.system(.footnote, design: .rounded, weight: .bold)).lineLimit(1).minimumScaleFactor(0.7)
    }

    /// Thin bar of metres done in the intensity color, with metres and goal under it.
    private func progress(_ s: SwimState, _ set: WorkoutSet?) -> some View {
        let frac = min(1, Double(s.doneM) / Double(max(s.targetM, 1)))
        return VStack(spacing: 4) {
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(.white.opacity(0.15))
                    Capsule().fill(set?.intensity.color ?? .accent).frame(width: g.size.width * frac)
                }
            }
            .frame(height: 6)
            .animation(.snappy, value: frac)
            HStack {
                Text("\(s.doneM)/\(s.targetM) m").foregroundStyle(Metric.distance.color).opacity(wristDown ? 0.6 : 1)
                Spacer()
                if let g = set?.goalS { Label(mmss(Double(g)), systemImage: "target") }
            }
            .font(.system(.footnote, design: .rounded, weight: .semibold)).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.7)
        }
    }

    /// Heart rate with its zone against the set's target zones, and the estimated time left of the workout.
    private func bottom(_ s: SwimState) -> some View {
        HStack {
            HStack(spacing: 3) {
                Image(systemName: "heart.fill").symbolEffect(.pulse)
                Text(session.heartRate.map { "\(Int($0.rounded()))" } ?? "--").fixedSize() // the number first: the time label shrinks instead
                if let set = currentSet(s) { ZoneBar(zone: zone, target: Array(targetZones(intensity: set.intensity))) }
            }
            .foregroundStyle(Metric.heart.color).opacity(wristDown ? 0.6 : 1)
            Spacer()
            if let w = session.workout, !wristDown { // the estimate ticks every second: left out with the wrist down
                HStack(spacing: 2) {
                    if !textSize.isAccessibilitySize { Image(systemName: "hourglass") } // at large text the icon squeezed the time to "…"
                    Text(mmss(estimatedRemainingS(workout: w, state: s, cssSPer100m: css))).fixedSize()
                }
                    .foregroundStyle(Metric.time.color)
            }
        }
        .font(.system(.footnote, design: .rounded, weight: .bold).monospacedDigit()).lineLimit(1).minimumScaleFactor(0.7)
        .padding(.horizontal, 2).padding(.bottom, 10) // clear of the rounded bottom corners
    }

    /// HR zone 1–5 when the heart rate and age are known.
    private var zone: UInt8? {
        guard let bpm = session.heartRate, let age = session.ageYears else { return nil }
        return hrZone(bpm: bpm, ageYears: age)
    }

    // MARK: Controls page: left of the live page, for taps when the screen is unlocked

    private var controls: some View {
        VStack(spacing: 10) {
            Spacer(minLength: 0)
            if let e = session.error { Text(e).font(.footnote).foregroundStyle(.red).multilineTextAlignment(.center) }
            Button(session.paused ? "Resume" : "Pause", systemImage: session.paused ? "play.fill" : "pause.fill") { session.togglePause() }
                .buttonStyle(.borderedProminent).tint(.orange)
            Button("End", action: onEnd).buttonStyle(.borderedProminent).tint(.red).font(.title3.bold())
            Spacer(minLength: 0)
            VStack(spacing: 3) {
                Label("Next", systemImage: "chevron.up")
                Label("Back", systemImage: "chevron.down")
                Label("Hold crown to unlock", systemImage: "lock.open")
            }
            .font(.caption).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 14).padding(.vertical, 8)
    }
}

/// The last rep: its time with the diff to goal in the verdict color and a trophy for a PR, then its goal and average heart rate, small and neutral.
/// The two rows can sit apart: the time in the rest circle, the stats under it.
private struct LastRep: View {
    let result: RepResult
    let set: WorkoutSet? // the rep's set: goal and intensity
    let ageYears: UInt32?
    var showsTime = true
    var showsStats = true
    var compact = false // inside the rest ring: the chord is narrow
    @State private var shown = false
    @Environment(\.isLuminanceReduced) private var wristDown
    @Environment(\.dynamicTypeSize) private var textSize

    var body: some View {
        VStack(spacing: 4) {
            if showsTime { timeRow }
            if showsStats { statsRow }
        }
        .scaleEffect(shown ? 1 : 0.8).opacity(shown ? 1 : 0)
        .onAppear { withAnimation(.spring(duration: 0.45, bounce: 0.3)) { shown = true } }
    }

    private var timeRow: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(mmss(result.swimS)).font(.system(compact ? .headline : .title3, design: .rounded, weight: .bold).monospacedDigit())
                .foregroundStyle(Metric.time.color).opacity(wristDown ? 0.6 : 1)
            if let d = result.goalDiffS {
                Text(signedSeconds(d)).font(.system(compact ? .caption : .subheadline, design: .rounded, weight: .bold).monospacedDigit())
                    .foregroundStyle(result.verdict?.color ?? .white)
            }
            if result.verdict == .fastest {
                Image(systemName: "trophy.fill").font(.system(size: 11, weight: .bold)).blendMode(.destinationOut) // a cutout: readable over a fill too
                    .padding(.horizontal, 7).padding(.vertical, 3).background(Color.purple, in: Capsule())
                    .compositingGroup()
                    .symbolEffect(.bounce, options: .nonRepeating)
                    .accessibilityLabel("Personal record")
            }
        }
        .lineLimit(1).minimumScaleFactor(0.4) // a small watch: the circle is smaller than the row
    }

    private var statsRow: some View {
        // At accessibility sizes the goal and the heart stack instead of squeezing into one row.
        (textSize.isAccessibilitySize ? AnyLayout(VStackLayout(spacing: 2)) : AnyLayout(HStackLayout(spacing: 8))) {
            if let g = set?.goalS { Label(mmss(Double(g)), systemImage: "target").labelStyle(Tight()) }
            if let set, let bpm = result.avgBpm {
                HStack(spacing: 3) {
                    Image(systemName: "heart.fill")
                    Text("\(Int(bpm.rounded()))").fixedSize()
                    if let age = ageYears { ZoneBar(zone: hrZone(bpm: bpm, ageYears: age), target: Array(targetZones(intensity: set.intensity))) }
                }
                .foregroundStyle(Metric.heart.color).opacity(wristDown ? 0.6 : 1)
            }
        }
        .font(.system(.footnote, design: .rounded, weight: .semibold).monospacedDigit()).foregroundStyle(.white.opacity(0.75))
        .lineLimit(1).minimumScaleFactor(0.5)
    }
}

/// Icon and title with little space between them.
private struct Tight: LabelStyle {
    func makeBody(configuration: Configuration) -> some View { HStack(spacing: 3) { configuration.icon; configuration.title } }
}

/// A bright color filling the screen from the bottom up to `frac` under a gently moving waterline.
/// Content over the color turns black: a second copy of it, masked to the water.
struct Filled<Content: View>: View {
    let frac: Double
    let color: Color
    @ViewBuilder let content: Content
    @State private var shift = 0.0 // one loop of the waves; shared, so the three layers stay in step
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GeometryReader { g in
            let water = { (c: Color) in Water(level: min(1, max(0, frac)), insets: g.safeAreaInsets, shift: shift, color: c) }
            ZStack {
                water(color)
                content.mask { Rectangle().overlay(water(.black).blendMode(.destinationOut)).compositingGroup() } // above the water only: no white edges
                content.colorMultiply(.black).mask(water(.black)).accessibilityHidden(true)
            }
        }
        .onAppear { if !reduceMotion { withAnimation(.linear(duration: 2.5).repeatForever(autoreverses: false)) { shift = 1 } } }
    }
}

/// The water with its waves slid sideways by `shift` wavelengths: an animated offset, so the render server moves it and the view is not redrawn.
private struct Water: View {
    let level: Double
    let insets: EdgeInsets
    let shift: Double
    let color: Color

    var body: some View {
        GeometryReader { g in
            let wavelength = (g.size.width + insets.leading + insets.trailing) / 1.5
            Wave(level: level, insets: insets, wavelength: wavelength).fill(color)
                .offset(x: shift * wavelength)
                .clipShape(ScreenRect(insets: insets)) // the extra wavelength must not show over a neighbouring page while paging
                .animation(.linear(duration: 1), value: level) // only the water moves: text in the same frame, no ghost copy
                .animation(.easeInOut(duration: 0.6), value: color)
        }
    }
}

/// The whole screen, safe area included, in the coordinates of the safe-area frame.
private struct ScreenRect: Shape {
    let insets: EdgeInsets
    func path(in r: CGRect) -> Path {
        Path(CGRect(x: r.minX - insets.leading, y: r.minY - insets.top,
                    width: r.width + insets.leading + insets.trailing, height: r.height + insets.top + insets.bottom))
    }
}

/// Water up to `level` (0–1) of the whole screen, safe area included, under a sine waterline; drawn one wavelength wider on the left so it can slide.
private struct Wave: Shape {
    var level: Double
    let insets: EdgeInsets
    let wavelength: CGFloat
    var animatableData: Double { get { level } set { level = newValue } }

    func path(in r: CGRect) -> Path {
        let full = CGRect(x: r.minX - insets.leading, y: r.minY - insets.top,
                          width: r.width + insets.leading + insets.trailing, height: r.height + insets.top + insets.bottom)
        let amp = 5 * min(1, min(level, 1 - level) * 8) // flat when empty or full: no gaps at the edges
        let y0 = full.maxY - full.height * level
        let x0 = full.minX - wavelength
        var p = Path()
        p.move(to: CGPoint(x: x0, y: full.maxY))
        for x in stride(from: x0, through: full.maxX + 4, by: 4) {
            p.addLine(to: CGPoint(x: x, y: y0 + amp * sin((x - x0) / wavelength * 2 * .pi)))
        }
        p.addLine(to: CGPoint(x: full.maxX + 4, y: full.maxY))
        p.closeSubpath()
        return p
    }
}

/// The rest as a circle draining from its length, the countdown in it with the last rep; the last 5 s turn blue.
private struct RestCountdown: View {
    let left: Double
    let lastRep: RepResult?
    let set: WorkoutSet? // the last rep's set: goal and intensity
    let ageYears: UInt32?
    @State private var total: Double // the rest length when this screen appeared
    @Environment(\.isLuminanceReduced) private var wristDown

    init(left: Double, lastRep: RepResult?, set: WorkoutSet?, ageYears: UInt32?) {
        self.left = left
        self.lastRep = lastRep
        self.set = set
        self.ageYears = ageYears
        _total = State(initialValue: max(left, 1))
    }

    private let d = WKInterfaceDevice.current().screenBounds.height * 0.62 // about 60% of the screen height, on every watch size

    var body: some View {
        let frac = min(1, max(0, left / total))
        let blue = left <= 5
        ZStack {
            Circle().stroke(.white.opacity(0.12), lineWidth: 12).padding(6) // the stroke is centred on the circle: inset by half of it to keep the diameter d
            Circle().trim(from: 0, to: frac)
                .stroke(blue ? Color.accent : (lastRep?.verdict?.color ?? .accent), style: StrokeStyle(lineWidth: 12, lineCap: .round))
                .padding(6)
                .rotationEffect(.degrees(-90))
                .animation(.linear(duration: 1), value: frac)
                .animation(.easeInOut(duration: 0.6), value: blue)
            VStack(spacing: 2) {
                if lastRep == nil { Text("GET READY").font(.system(.caption, design: .rounded, weight: .bold)).foregroundStyle(Color.accent).multilineTextAlignment(.center).minimumScaleFactor(0.6) }
                Text(mmss(left)).font(.system(size: 50, weight: .bold, design: .rounded).monospacedDigit())
                    .minimumScaleFactor(0.4).lineLimit(1).contentTransition(.numericText(countsDown: true))
                    .foregroundStyle(blue ? Color.accent : Metric.time.color).opacity(wristDown ? 0.6 : 1)
                    .animation(.easeInOut(duration: 0.6), value: blue)
                    .layoutPriority(1).accessibilityIdentifier("live.rest")
                if let r = lastRep { LastRep(result: r, set: set, ageYears: ageYears, showsStats: false, compact: true).frame(maxWidth: d * 0.6) } // inside the ring's chord
            }
            .padding(.horizontal, 20)
        }
        .frame(width: d, height: d) // a fixed share of the screen: the text under it must not shrink it
    }
}

/// How far the crown has turned towards a skip: fills from the centre (up = next, down = back), white when the skip is about to fire.
private struct CrownGauge: View {
    let crown: Double
    let travel: Double
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let frac = min(1, abs(crown) / travel)
        let next = crown > 0
        let shown = abs(crown) > 0.02
        let color: Color = frac >= 0.9 ? .white : (next ? Color.accent : .orange)
        VStack(spacing: 4) {
            Image(systemName: "chevron.up").opacity(next ? 1 : 0)
            ZStack {
                Capsule().fill(.white.opacity(0.15))
                Capsule().fill(color).frame(height: 45 * frac).offset(y: (next ? -1 : 1) * 22.5 * frac)
            }
            .frame(width: 6, height: 90)
            Image(systemName: "chevron.down").opacity(next ? 0 : 1)
        }
        .font(.system(size: 11, weight: .bold))
        .padding(.vertical, 8).padding(.horizontal, 5)
        .background(Color.black.opacity(0.5), in: Capsule()) // readable over the water
        .opacity(shown ? 1 : 0)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.15), value: shown)
        .allowsHitTesting(false)
    }
}

/// Rest (prefixed "@" for an interval) and goal of a set, each only when it exists.
private struct RestAndGoal: View {
    let set: WorkoutSet

    var body: some View {
        HStack(spacing: 8) {
            if let r = set.rest { Label((r.kind == .interval ? "@" : "") + mmss(Double(r.seconds)), systemImage: "timer") }
            if let g = set.goalS { Label(mmss(Double(g)), systemImage: "target") }
        }
        .font(.system(.footnote, design: .rounded, weight: .semibold).monospacedDigit()).foregroundStyle(.secondary)
        .lineLimit(1).minimumScaleFactor(0.7)
    }
}

private extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}
