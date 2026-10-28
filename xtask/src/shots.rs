//! `cargo xtask shots`: fresh README screenshots. The iPhone app runs on the simulator with
//! `-demo <arg>` (DEBUG builds only), then the watch UI tests export their screenshots.

use std::{fs, process::Command, thread, time::Duration};

use std::path::Path;

use serde_json::Value;

use crate::{IOS_SCHEME, PROJECT, env_cmd, project, run as exec, run_logged, test};

const IPHONE_NAME: &str = "iPhone 17";
const DERIVED: &str = "build/derived-iossim";
const BUILD_LOG: &str = "build/shots-build.log";
/// (output name, `-demo` argument). Each screen is written to build/phone-<name>.png.
const SHOTS: [(&str, &str); 17] = [
    ("history", "history"),
    ("detail", "detail"),
    ("trends", "trends"),
    ("plan", "plan"),
    ("creator", "creator"),
    ("workouts", "plans"),
    ("generate", "generate"),
    ("drills", "drills"),
    ("rounds", "rounds"),
    ("settings-ai", "settings-ai"),
    ("detail-hr", "detail-hr"),
    ("detail-pace", "detail-pace"),
    ("detail-sets", "detail-sets"),
    ("builder-2", "creator-2"),
    ("builder-3", "creator-3"),
    ("builder-4", "creator-4"),
    ("builder-5", "creator-5"),
];
/// (output name, `-demo` argument, reply kind). The AI replies are recorded once from a real call (`API_KEY` and
/// `AI_PROVIDER` in `.env`) into docs/shots/ai-<kind>.txt and replayed after that, so the shots cost no tokens.
/// Delete a text file (or set `REFRESH_AI=1`) to record it again.
const AI_SHOTS: [(&str, &str, &str); 2] = [
    ("ai-generate", "ai-generate", "generate"),
    ("ai-review", "detail-review", "review"),
];
const AI_DIR: &str = "docs/shots";

#[expect(
    clippy::too_many_lines,
    reason = "build steps that run in order; each step is a call, not logic"
)]
pub(crate) fn run() -> Result<(), String> {
    project()?;
    let (udid, booted) = iphone()?;
    if !booted {
        exec(Command::new("xcrun").args(["simctl", "boot", &udid]))?;
        exec(Command::new("xcrun").args(["simctl", "bootstatus", &udid, "-b"]))?;
    }
    build(&udid)?;

    let app = format!("{DERIVED}/Build/Products/Debug-iphonesimulator/{IOS_SCHEME}.app");
    let bundle = bundle_id(&app)?;
    let _ = Command::new("xcrun")
        .args(["simctl", "uninstall", &udid, &bundle])
        .status(); // fresh demo data
    exec(Command::new("xcrun").args(["simctl", "install", &udid, &app]))?;
    exec(Command::new("xcrun").args([
        "simctl",
        "status_bar",
        &udid,
        "override",
        "--time",
        "9:41",
        "--batteryState",
        "charged",
        "--batteryLevel",
        "100",
        "--cellularBars",
        "4",
        "--wifiBars",
        "3",
    ]))?;
    exec(Command::new("xcrun").args(["simctl", "ui", &udid, "appearance", "dark"]))?;

    for (name, demo) in SHOTS {
        let _ = Command::new("xcrun")
            .args(["simctl", "terminate", &udid, &bundle])
            .status();
        exec(
            Command::new("xcrun")
                .args([
                    "simctl",
                    "launch",
                    "--terminate-running-process",
                    &udid,
                    &bundle,
                    "-demo",
                    demo,
                ])
                .args(if demo == "settings-ai" {
                    &[
                        "-aiEngine",
                        "api",
                        "-aiProvider",
                        "mistral",
                        "-aiEndpoint",
                        "https://api.mistral.ai/v1",
                    ][..]
                } else {
                    &[][..]
                }),
        )?;
        // The detail screens snapshot a map, which fades in late.
        let settle = if demo.starts_with("detail") { 10 } else { 3 };
        thread::sleep(Duration::from_secs(settle));
        exec(Command::new("xcrun").args([
            "simctl",
            "io",
            &udid,
            "screenshot",
            &format!("build/phone-{name}.png"),
        ]))?;
    }
    let _ = Command::new("xcrun")
        .args(["simctl", "terminate", &udid, &bundle])
        .status();

    ai_shots(&udid, &bundle)?;

    test(Some("ui"))?;

    println!("shots written:");
    let mut files: Vec<String> = SHOTS
        .iter()
        .map(|(name, _)| format!("build/phone-{name}.png"))
        .collect();
    if let Ok(dir) = fs::read_dir("build") {
        for entry in dir.flatten() {
            let path = entry.path().display().to_string();
            if path.contains("/ui-")
                && Path::new(&path)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    for f in files {
        println!("  {f}");
    }
    Ok(())
}

fn ai_shots(udid: &str, bundle: &str) -> Result<(), String> {
    let refresh = std::env::var("REFRESH_AI").is_ok_and(|v| !v.is_empty());
    let provider = crate::setting("AI_PROVIDER").unwrap_or_else(|| "mistral".into());
    fs::create_dir_all(AI_DIR).map_err(|e| format!("cannot create {AI_DIR}: {e}"))?;
    for (name, demo, kind) in AI_SHOTS {
        let text_file = format!("{AI_DIR}/ai-{kind}.txt");
        let recorded = fs::read_to_string(&text_file)
            .ok()
            .filter(|t| !t.is_empty() && !refresh);
        let _ = Command::new("xcrun")
            .args(["simctl", "terminate", udid, bundle])
            .status();
        let mut launch = Command::new("xcrun");
        launch
            .args([
                "simctl",
                "launch",
                "--terminate-running-process",
                udid,
                bundle,
                "-demo",
                demo,
            ])
            .args(["-aiEngine", "api", "-aiProvider", &provider]);
        let wait = if let Some(text) = &recorded {
            launch
                .env(
                    format!("SIMCTL_CHILD_SWIMLIE_AI_{}", kind.to_uppercase()),
                    text,
                )
                .env("SIMCTL_CHILD_SWIMLIE_API_KEY", "replay");
            6
        } else {
            let Some(key) = crate::setting("API_KEY") else {
                println!("ai shots: {name} skipped, no recorded reply and no API_KEY in .env");
                continue;
            };
            launch.env("SIMCTL_CHILD_SWIMLIE_API_KEY", key);
            60 // the model answers and streams
        };
        exec(&mut launch)?;
        thread::sleep(Duration::from_secs(wait));
        exec(Command::new("xcrun").args([
            "simctl",
            "io",
            udid,
            "screenshot",
            &format!("build/phone-{name}.png"),
        ]))?;
        if recorded.is_none() {
            let out = Command::new("xcrun")
                .args(["simctl", "get_app_container", udid, bundle, "data"])
                .output()
                .map_err(|e| e.to_string())?;
            let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            fs::copy(format!("{dir}/Documents/ai-{kind}.txt"), &text_file)
                .map_err(|e| format!("the AI reply was not recorded: {e}"))?;
        }
    }
    Ok(())
}

/// The iPhone 17 simulator: a booted one if there is one, else the first available.
fn iphone() -> Result<(String, bool), String> {
    let out = Command::new("xcrun")
        .args(["simctl", "list", "devices", "available", "-j"])
        .output()
        .map_err(|e| format!("failed to start `xcrun simctl list`: {e}"))?;
    let json: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("cannot parse `simctl list`: {e}"))?;
    let mut found: Vec<(String, bool)> = Vec::new();
    for list in json["devices"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        for d in list.as_array().into_iter().flatten() {
            if d["name"] == IPHONE_NAME {
                let udid = d["udid"].as_str().unwrap_or_default().to_string();
                found.push((udid, d["state"] == "Booted"));
            }
        }
    }
    found.sort_by_key(|(_, booted)| !booted);
    found
        .into_iter()
        .next()
        .ok_or_else(|| format!("no available simulator named {IPHONE_NAME}"))
}

fn build(udid: &str) -> Result<(), String> {
    run_logged(
        env_cmd("xcodebuild").args([
            "-project",
            PROJECT,
            "-scheme",
            IOS_SCHEME,
            "-configuration",
            "Debug",
            "-destination",
            &format!("platform=iOS Simulator,id={udid}"),
            "ARCHS=arm64",
            "-derivedDataPath",
            DERIVED,
            "build",
        ]),
        BUILD_LOG,
    )
}

fn bundle_id(app: &str) -> Result<String, String> {
    let out = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            "Print :CFBundleIdentifier",
            &format!("{app}/Info.plist"),
        ])
        .output()
        .map_err(|e| format!("cannot read {app}/Info.plist: {e}"))?;
    if !out.status.success() {
        return Err(format!("cannot read the bundle id of {app}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
