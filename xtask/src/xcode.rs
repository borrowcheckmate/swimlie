use super::{
    Command, NIGHTLY_DEFAULT, PROJECT, Path, SystemTime, UI_RESULT, UI_SCHEME, UNIT_SCHEME,
    WATCH_SIM_UDID, env, env_cmd, fs, remove_dir_if_exists, run, run_logged, team_id, usage_exit,
};

#[expect(
    clippy::too_many_lines,
    reason = "build steps that run in order; each step is a call, not logic"
)]
pub(crate) fn xcframework() -> Result<(), String> {
    // watchOS targets are Tier 3: they need nightly + build-std. Pinned for reproducible builds.
    let nightly = env::var("NIGHTLY").unwrap_or_else(|_| NIGHTLY_DEFAULT.to_string());
    let pkg = "apple/SwimlieCore";
    let out = "build";
    let fw = format!("{pkg}/SwimlieCoreFFI.xcframework");
    let lib = "libswimlie_core.a";
    let swift_dir = format!("{out}/swift");
    let headers = format!("{swift_dir}/headers");

    run(env_cmd("rustup").args([
        "target",
        "add",
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
    ]))?;
    run(env_cmd("rustup").args(["component", "add", "rust-src", "--toolchain", &nightly]))?;

    for t in ["aarch64-apple-ios", "aarch64-apple-ios-sim"] {
        run(env_cmd("cargo").args([
            "build",
            "-p",
            "swimlie-core",
            "--release",
            "--lib",
            "--target",
            t,
        ]))?;
    }
    for t in [
        "arm64_32-apple-watchos",
        "aarch64-apple-watchos",
        "aarch64-apple-watchos-sim",
    ] {
        run(env_cmd("cargo").arg(format!("+{nightly}")).args([
            "build",
            "-Z",
            "build-std=std,panic_abort",
            "-p",
            "swimlie-core",
            "--release",
            "--lib",
            "--target",
            t,
        ]))?;
    }
    // shortcut: Apple-silicon simulators only (no x86_64 slices), add them if someone builds on an Intel Mac.

    // Swift bindings, generated from the library's metadata.
    run(env_cmd("cargo").args(["build", "-p", "swimlie-core", "--release", "--lib"]))?;
    remove_dir_if_exists(&swift_dir)?;
    fs::create_dir_all(&headers).map_err(|e| format!("cannot create {headers}: {e}"))?;
    run(env_cmd("cargo").args([
        "run",
        "-p",
        "swimlie-core",
        "--bin",
        "uniffi-bindgen",
        "--",
        "generate",
        "--library",
        &format!("target/release/{lib}"),
        "--language",
        "swift",
        "--out-dir",
        &swift_dir,
    ]))?;
    let mv =
        |from: &str, to: &str| fs::rename(from, to).map_err(|e| format!("cannot move {from}: {e}"));
    mv(
        &format!("{swift_dir}/swimlie_coreFFI.h"),
        &format!("{headers}/swimlie_coreFFI.h"),
    )?;
    mv(
        &format!("{swift_dir}/swimlie_coreFFI.modulemap"),
        &format!("{headers}/module.modulemap"),
    )?;
    let sources = format!("{pkg}/Sources/SwimlieCore");
    fs::create_dir_all(&sources).map_err(|e| format!("cannot create {sources}: {e}"))?;
    fs::copy(
        format!("{swift_dir}/swimlie_core.swift"),
        format!("{sources}/swimlie_core.swift"),
    )
    .map_err(|e| format!("cannot copy Swift bindings: {e}"))?;

    // One watch device library holding both architectures (arm64_32 for older watches).
    fs::create_dir_all(format!("{out}/watchos"))
        .map_err(|e| format!("cannot create {out}/watchos: {e}"))?;
    let watch_lib = format!("{out}/watchos/{lib}");
    run(env_cmd("lipo").args([
        "-create",
        "-output",
        &watch_lib,
        &format!("target/arm64_32-apple-watchos/release/{lib}"),
        &format!("target/aarch64-apple-watchos/release/{lib}"),
    ]))?;

    remove_dir_if_exists(&fw)?;
    run(env_cmd("xcodebuild").args([
        "-create-xcframework",
        "-library",
        &format!("target/aarch64-apple-ios/release/{lib}"),
        "-headers",
        &headers,
        "-library",
        &format!("target/aarch64-apple-ios-sim/release/{lib}"),
        "-headers",
        &headers,
        "-library",
        &watch_lib,
        "-headers",
        &headers,
        "-library",
        &format!("target/aarch64-apple-watchos-sim/release/{lib}"),
        "-headers",
        &headers,
        "-output",
        &fw,
    ]))?;

    println!("Built {fw}");
    Ok(())
}

pub(crate) fn newest_mtime(path: &Path) -> Result<SystemTime, String> {
    let meta = fs::metadata(path).map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
    let mut newest = meta
        .modified()
        .map_err(|e| format!("cannot read mtime of {}: {e}", path.display()))?;
    if meta.is_dir() {
        let entries =
            fs::read_dir(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            newest = newest.max(newest_mtime(&entry.path())?);
        }
    }
    Ok(newest)
}

pub(crate) fn project() -> Result<(), String> {
    let fw = Path::new("apple/SwimlieCore/SwimlieCoreFFI.xcframework");
    let fw_time = fs::metadata(fw).and_then(|m| m.modified()).ok();
    let sources =
        newest_mtime(Path::new("core/src"))?.max(newest_mtime(Path::new("core/Cargo.toml"))?);
    match fw_time {
        Some(t) if t >= sources => println!("xcframework: up to date"),
        _ => xcframework()?,
    }
    run(Command::new("xcodegen")
        .current_dir("apple")
        .env("SWIMLIE_TEAM_ID", team_id()?)
        .env("SWIMLIE_VERSION", env!("CARGO_PKG_VERSION")))?;
    println!("project: generated apple/Swimlie.xcodeproj");
    Ok(())
}

/// True when project.yml produced a target of this product type (`unit-test` or `ui-testing`).
pub(crate) fn has_test_target(product_type: &str) -> Result<bool, String> {
    let pbx = fs::read_to_string(format!("{PROJECT}/project.pbxproj")).map_err(|e| {
        format!("cannot read {PROJECT}/project.pbxproj (run `cargo xtask project`): {e}")
    })?;
    Ok(pbx.contains(&format!("com.apple.product-type.bundle.{product_type}")))
}

/// The last "Executed N tests, with M failures" line xcodebuild wrote to `log`: the whole run's total.
pub(crate) fn test_summary(log: &str) -> Option<String> {
    let text = fs::read_to_string(log).ok()?;
    text.lines()
        .map(str::trim)
        .rev()
        .find(|l| l.starts_with("Executed ") && l.contains(" tests, with "))
        .map(str::to_string)
}

/// Copies the UI test screenshots (`XCTAttachments`) out of the result bundle to build/ui-<name>.png.
pub(crate) fn export_screenshots() {
    let out = "build/ui-attachments";
    let _ = fs::remove_dir_all(out);
    let exported = Command::new("xcrun")
        .args([
            "xcresulttool",
            "export",
            "attachments",
            "--path",
            UI_RESULT,
            "--output-path",
            out,
        ])
        .status();
    if !exported.is_ok_and(|s| s.success()) {
        println!("screenshots: export failed; the attachments are in {UI_RESULT}");
        return;
    }
    // manifest.json pairs each exported file with its attachment name ("live-rest_0_<uuid>.png").
    let manifest = fs::read_to_string(format!("{out}/manifest.json")).unwrap_or_default();
    // Each key is followed by `: "value"`; the value is the text up to the next quote.
    let value = |s: &str| s.split('"').nth(1).map(str::to_string);
    for chunk in manifest.split("\"exportedFileName\"").skip(1) {
        let file = value(chunk);
        let name = chunk
            .split("\"suggestedHumanReadableName\"")
            .nth(1)
            .and_then(value);
        let name = name
            .as_deref()
            .and_then(|n| n.split("_0_").next())
            .map(str::to_string);
        if let (Some(file), Some(name)) = (
            file.filter(|f| {
                Path::new(f)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            }),
            name,
        ) {
            let to = format!("build/ui-{name}.png");
            if let Err(e) = fs::copy(format!("{out}/{file}"), &to) {
                println!("screenshots: cannot copy {file} to {to}: {e}");
            } else {
                println!("screenshot: {to}");
            }
        }
    }
}

pub(crate) fn test(part: Option<&str>) -> Result<(), String> {
    let parts: &[&str] = match part.unwrap_or("all") {
        "all" => &["core", "unit", "ui"],
        "core" => &["core"],
        "unit" => &["unit"],
        "ui" => &["ui"],
        _ => usage_exit(),
    };
    for &p in parts {
        let result = match p {
            "core" => run(env_cmd("cargo").args(["test", "-p", "swimlie-core"])),
            "unit" | "ui" => {
                let (product, label, log, scheme) = if p == "unit" {
                    (
                        "unit-test",
                        "unit tests",
                        "build/test-unit.log",
                        UNIT_SCHEME,
                    )
                } else {
                    ("ui-testing", "ui tests", "build/test-ui.log", UI_SCHEME)
                };
                if !has_test_target(product)? {
                    println!("{p}: skipped: no {label} yet");
                    continue;
                }
                let mut cmd = Command::new("xcodebuild");
                cmd.args([
                    "-project",
                    PROJECT,
                    "-scheme",
                    scheme,
                    "-destination",
                    &format!("platform=watchOS Simulator,id={WATCH_SIM_UDID}"),
                    "ARCHS=arm64",
                ]);
                cmd.arg(format!("-only-testing:{scheme}"));
                if p == "ui" {
                    let _ = fs::remove_dir_all(UI_RESULT); // xcodebuild refuses an existing bundle
                    cmd.args(["-resultBundlePath", UI_RESULT]);
                }
                let result = run_logged(cmd.arg("test"), log);
                if p == "ui" {
                    export_screenshots();
                }
                if let Some(s) = test_summary(log) {
                    println!("{p}: {s}");
                }
                result
            }
            _ => unreachable!(),
        };
        match result {
            Ok(()) => println!("{p}: passed"),
            Err(e) => {
                println!("{p}: failed");
                return Err(e);
            }
        }
    }
    Ok(())
}
