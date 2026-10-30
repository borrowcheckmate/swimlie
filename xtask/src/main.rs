//! Repo automation: builds, tests, devices, screenshots. Run as `cargo xtask <command>`.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command line tool talks to the terminal"
)]
mod mockups;
mod shots;

use std::{
    env,
    fs::{self, File},
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Stdio, exit},
    sync::OnceLock,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use swimlie_core::engine::Engine;
use swimlie_core::metrics::{SwimLevel, SwimmerProfile};
use swimlie_core::model::{Lap, Stroke, Workout};
use swimlie_core::review::session_review_brief;
use swimlie_core::store::StoredSession;

mod devices;
mod frames;
mod lint;
mod process;
mod recorder;
mod settings;
mod sim;
mod testflight;
mod xcode;

use devices::{install, show_devices};
use frames::frames;
use lint::lint;
use process::{describe, env_cmd, fail, remove_dir_if_exists, run, run_logged, usage_exit};
use recorder::{brief, laps};
use settings::{op_download, parse_dotenv, raw_setting, setting, team_id};
use sim::{record, sim};
use testflight::testflight;
use xcode::{project, test, xcframework};

const NIGHTLY_DEFAULT: &str = "nightly-2026-08-15";
const PROJECT: &str = "apple/Swimlie.xcodeproj";
const IOS_SCHEME: &str = "Swimlie";
const WATCH_SCHEME: &str = "SwimlieWatch";
const UNIT_SCHEME: &str = "SwimlieWatchTests"; // scheme and target of the unit tests (project.yml)
const UI_SCHEME: &str = "SwimlieWatchUITests"; // scheme and target of the UI tests (project.yml)
const UI_RESULT: &str = "build/test-ui.xcresult";
const WATCH_SIM_UDID: &str = "CF1B4B17-942E-49DC-ACE5-6FC23D196F52";
const WATCH_BUNDLE_ID: &str = "com.swimlie.swimlie.watchkitapp";
const RECORD_REPLAY: &str = "recorded-swim-b"; // default replay of `record`
const RECORD_FILE: &str = "build/sim.mp4";
const USAGE: &str = "usage: cargo xtask <command>
  xcframework                        build SwimlieCore for iOS and watchOS
  project                            rebuild the xcframework if stale, then xcodegen
  test [core|unit|ui|all]            run tests (default: all)
  sim [--replay <n>] [--speed N] [--start N] [--script <s>]   build, install and launch on the watch simulator
  record [--replay <n>] [--speed N] [--start N] [--secs N] [--script <s>]   record the watch simulator to build/sim.mp4
  devices                            list devices from devicectl (for .env)
  install [--phone <name|udid>]      build and install on a paired iPhone (default: .env, then the only paired one)
  testflight                         archive a Release build and upload it to TestFlight (needs SWIMLIE_ASC_* in .env)
  laps <file.ndjson>                 print the HealthKit laps and rests (> 8 s gaps) of a recorder log
  brief <file.ndjson>                print the AI review brief of a recorder log (the exact model input)
  frames <video> [--fps N] [--cols N] [--out <png>]   contact sheet of a video, one frame per 1/fps s (default build/<stem>-sheet.png)
  shots                              fresh README screenshots: iPhone demo screens (build/phone-*.png) and watch UI shots (build/ui-*.png)
  lint                               formatting, strict clippy and the file length limit (what CI runs)
  mockups                            render README device mockups into docs/ from build/ screenshots
settings (.env in the repo root, see .env.example): SWIMLIE_PHONE, SWIMLIE_WATCH";

/// Settings from `.env`, read once at startup. Real environment variables win over them.
static DOTENV: OnceLock<Vec<(String, String)>> = OnceLock::new();

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if let [cmd, file] = args.as_mut_slice()
        && (cmd == "laps" || cmd == "brief")
    {
        // Resolve the file before entering the workspace root, so the path is the caller's.
        *file = fs::canonicalize(&*file)
            .unwrap_or_else(|e| fail(&format!("cannot read {file}: {e}")))
            .display()
            .to_string();
    }
    if args.first().is_some_and(|c| c == "frames")
        && let Some(file) = args.get_mut(1)
        && !file.starts_with("--")
    {
        *file = fs::canonicalize(&*file)
            .unwrap_or_else(|e| fail(&format!("cannot read {file}: {e}")))
            .display()
            .to_string();
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    if let Err(e) = env::set_current_dir(&root) {
        fail(&format!("cannot enter {}: {e}", root.display()));
    }
    let dotenv = match fs::read_to_string(".env") {
        Ok(text) => parse_dotenv(&text),
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => fail(&format!("cannot read .env: {e}")),
    };
    let _ = DOTENV.set(dotenv);

    let rest = &args[args.len().min(1)..];
    let result = match args.first().map(String::as_str) {
        Some("xcframework") if rest.is_empty() => xcframework(),
        Some("project") if rest.is_empty() => project(),
        Some("test") if rest.len() <= 1 => test(rest.first().map(String::as_str)),
        Some("sim") => sim(rest),
        Some("record") => record(rest),
        Some("devices") if rest.is_empty() => show_devices(),
        Some("install") => install(rest),
        Some("testflight") if rest.is_empty() => testflight(),
        Some("laps") if rest.len() == 1 => laps(&rest[0]),
        Some("brief") if rest.len() == 1 => brief(&rest[0]),
        Some("frames") => frames(rest),
        Some("mockups") if rest.is_empty() => mockups::run(),
        Some("lint") if rest.is_empty() => lint(),
        Some("shots") if rest.is_empty() => shots::run(),
        _ => usage_exit(),
    };
    if let Err(e) = result {
        fail(&e);
    }
}

#[cfg(test)]
mod tests;
