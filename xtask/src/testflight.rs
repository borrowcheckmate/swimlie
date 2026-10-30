use super::{
    Command, IOS_SCHEME, PROJECT, Path, SystemTime, UNIX_EPOCH, fs, op_download, project,
    raw_setting, run_logged, setting, team_id,
};

/// Archives a Release iPhone build (watch app embedded) and uploads it to App Store Connect for `TestFlight`.
/// `SWIMLIE_ASC_KEY_PATH` may be a plain .p8 path or an `op://` reference, which is written to a temporary file.
pub(crate) fn testflight() -> Result<(), String> {
    setting("SWIMLIE_BUNDLE_ID").ok_or(
        "SWIMLIE_BUNDLE_ID is not set (a bundle id registered to your team, see .env.example)",
    )?;
    let key_id = setting("SWIMLIE_ASC_KEY_ID")
        .ok_or("SWIMLIE_ASC_KEY_ID is not set (App Store Connect API key id, see .env.example)")?;
    let issuer = setting("SWIMLIE_ASC_ISSUER_ID").ok_or(
        "SWIMLIE_ASC_ISSUER_ID is not set (App Store Connect issuer id, see .env.example)",
    )?;
    let key_ref = raw_setting("SWIMLIE_ASC_KEY_PATH")
        .ok_or("SWIMLIE_ASC_KEY_PATH is not set (path to the App Store Connect API .p8 key, see .env.example)")?;
    if key_ref.starts_with("op://") {
        let key_file = "build/asc-key.p8";
        op_download(&key_ref, key_file)?;
        let result = upload_testflight(&key_id, &issuer, key_file);
        let _ = fs::remove_file(key_file);
        return result;
    }
    if !Path::new(&key_ref).is_file() {
        return Err(format!("SWIMLIE_ASC_KEY_PATH does not exist: {key_ref}"));
    }
    upload_testflight(&key_id, &issuer, &key_ref)
}

/// Archives, exports and uploads the iPhone build with the given App Store Connect API key file.
#[expect(
    clippy::too_many_lines,
    reason = "build steps that run in order; each step is a call, not logic"
)]
pub(crate) fn upload_testflight(key_id: &str, issuer: &str, key_path: &str) -> Result<(), String> {
    project()?;
    // A unix timestamp is unique per upload and always higher than the last one.
    let build = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system clock is before 1970: {e}"))?
        .as_secs()
        .to_string();
    let archive = "build/Swimlie.xcarchive";
    run_logged(
        Command::new("xcodebuild")
            .args([
                "archive",
                "-project",
                PROJECT,
                "-scheme",
                IOS_SCHEME,
                "-configuration",
                "Release",
                "-destination",
                "generic/platform=iOS",
                "-archivePath",
                archive,
                "-derivedDataPath",
                "build/derived-archive",
                "-allowProvisioningUpdates",
                "-authenticationKeyPath",
                key_path,
                "-authenticationKeyID",
                key_id,
                "-authenticationKeyIssuerID",
                issuer,
            ])
            .arg(format!("CURRENT_PROJECT_VERSION={build}")),
        "build/testflight-archive.log",
    )?;

    fs::create_dir_all("build").map_err(|e| format!("cannot create build: {e}"))?;
    let plist = "build/ExportOptions.plist";
    let team = team_id()?;
    fs::write(
        plist,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store-connect</string>
    <key>destination</key>
    <string>upload</string>
    <key>teamID</key>
    <string>{team}</string>
    <key>signingStyle</key>
    <string>automatic</string>
    <key>uploadSymbols</key>
    <true/>
</dict>
</plist>
"#
        ),
    )
    .map_err(|e| format!("cannot write {plist}: {e}"))?;
    run_logged(
        Command::new("xcodebuild").args([
            "-exportArchive",
            "-archivePath",
            archive,
            "-exportOptionsPlist",
            plist,
            "-exportPath",
            "build/export",
            "-allowProvisioningUpdates",
            "-authenticationKeyPath",
            key_path,
            "-authenticationKeyID",
            key_id,
            "-authenticationKeyIssuerID",
            issuer,
        ]),
        "build/testflight-export.log",
    )?;
    println!("Uploaded build {build}; it shows in TestFlight after processing (~10 min)");
    Ok(())
}
