use super::{
    Command, Duration, PROJECT, Path, RECORD_FILE, RECORD_REPLAY, Stdio, WATCH_SCHEME,
    WATCH_SIM_UDID, bundle_id, describe, fs, project, run, run_logged, thread, usage_exit,
};

/// Arguments shared by `sim` and `record`; they become launch arguments of the watch app.
#[derive(Default)]
pub(crate) struct SimOpts {
    pub(super) replay: Option<String>,
    pub(super) speed: Option<String>,
    pub(super) start: Option<String>,
    pub(super) script: Option<String>,
    pub(super) secs: Option<u64>,
}

pub(crate) fn parse_sim_args(args: &[String], record: bool) -> SimOpts {
    let mut o = SimOpts::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--replay" => o.replay = Some(value()),
            "--speed" => o.speed = Some(value()),
            "--start" => o.start = Some(value()),
            "--script" => o.script = Some(value()),
            "--secs" if record => o.secs = Some(value().parse().unwrap_or_else(|_| usage_exit())),
            _ => usage_exit(),
        }
    }
    o
}

/// Builds, installs and launches the watch app on the simulator.
pub(crate) fn build_and_launch(o: &SimOpts) -> Result<(), String> {
    project()?;
    let derived = "build/derived-sim";
    run_logged(
        Command::new("xcodebuild").args([
            "-project",
            PROJECT,
            "-scheme",
            WATCH_SCHEME,
            "-configuration",
            "Debug",
            "-destination",
            &format!("platform=watchOS Simulator,id={WATCH_SIM_UDID}"),
            "ARCHS=arm64",
            "-derivedDataPath",
            derived,
            "build",
        ]),
        "build/sim-build.log",
    )?;
    let app = format!("{derived}/Build/Products/Debug-watchsimulator/{WATCH_SCHEME}.app");
    run(Command::new("xcrun").args(["simctl", "install", WATCH_SIM_UDID, &app]))?;

    let mut launch = Command::new("xcrun");
    launch.args(["simctl", "launch", WATCH_SIM_UDID]);
    launch.arg(format!("{}.watchkitapp", bundle_id()));
    if let Some(name) = &o.replay {
        launch.args(["-replay", name]);
    }
    if let Some(n) = &o.speed {
        launch.args(["-speed", n]);
    }
    if let Some(n) = &o.start {
        launch.args(["-start", n]);
    }
    if let Some(s) = &o.script {
        launch.args(["-script", s]);
    }
    run(&mut launch)?;
    Ok(())
}

pub(crate) fn sim(args: &[String]) -> Result<(), String> {
    build_and_launch(&parse_sim_args(args, false))?;

    thread::sleep(Duration::from_secs(3)); // let the first frame render before capturing
    let shot = "build/sim.png";
    run(Command::new("xcrun").args(["simctl", "io", WATCH_SIM_UDID, "screenshot", shot]))?;
    println!(
        "screenshot: {}",
        Path::new(shot)
            .canonicalize()
            .map_err(|e| e.to_string())?
            .display()
    );
    Ok(())
}

/// Records the watch simulator for `--secs` (default 20) while the app replays (default recorded-swim-b at 5x).
pub(crate) fn record(args: &[String]) -> Result<(), String> {
    let mut o = parse_sim_args(args, true);
    o.replay.get_or_insert_with(|| RECORD_REPLAY.to_string());
    o.speed.get_or_insert_with(|| "5".to_string());
    let secs = o.secs.unwrap_or(20);
    build_and_launch(&o)?;
    let _ = fs::remove_file(RECORD_FILE); // simctl refuses to overwrite ("File exists"), leaving the old video

    let mut rec = Command::new("xcrun");
    rec.args([
        "simctl",
        "io",
        WATCH_SIM_UDID,
        "recordVideo",
        "--codec",
        "h264",
        RECORD_FILE,
    ])
    .stdout(Stdio::null());
    println!("$ {}  ({secs}s)", describe(&rec));
    let mut child = rec
        .spawn()
        .map_err(|e| format!("failed to start recordVideo: {e}"))?;
    thread::sleep(Duration::from_secs(secs));
    // SIGINT makes simctl finalise the file; a plain kill would leave it unplayable.
    run(Command::new("kill").args(["-INT", &child.id().to_string()]))?;
    child
        .wait()
        .map_err(|e| format!("recordVideo did not exit: {e}"))?;

    let len = fs::metadata(RECORD_FILE).map_or(0, |m| m.len());
    if len == 0 {
        return Err(format!("no recording written to {RECORD_FILE}"));
    }
    let path = Path::new(RECORD_FILE)
        .canonicalize()
        .map_err(|e| e.to_string())?
        .display()
        .to_string();
    println!("recording: {path} ({len} bytes)");
    println!("hint: cargo xtask frames {path}");
    Ok(())
}
