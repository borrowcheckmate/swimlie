use super::{Command, IOS_SCHEME, PROJECT, project, run, run_logged, setting, usage_exit};

pub(crate) struct Device {
    pub(super) name: String,
    pub(super) id: String,
    pub(super) state: String,
    pub(super) paired: bool,
}

pub(crate) fn is_uuid(s: &str) -> bool {
    // Physical devices use a 25-char hardware UDID (8-16 hex), simulators a 36-char UUID.
    if s.len() == 25 {
        return s
            .bytes()
            .enumerate()
            .all(|(i, b)| (i == 8 && b == b'-') || (i != 8 && b.is_ascii_hexdigit()));
    }
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

pub(crate) fn list_devices() -> Result<Vec<Device>, String> {
    let out = Command::new("xcrun")
        .args(["devicectl", "list", "devices"])
        .output()
        .map_err(|e| format!("failed to start `xcrun devicectl list devices`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`xcrun devicectl list devices` failed ({})",
            out.status
        ));
    }
    Ok(parse_devices(&String::from_utf8_lossy(&out.stdout)))
}

pub(crate) fn parse_devices(text: &str) -> Vec<Device> {
    text.lines()
        .filter_map(|line| {
            // Columns are separated by two or more spaces; the State column follows the identifier.
            let cols: Vec<&str> = line
                .split("  ")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            let id = cols
                .iter()
                .find_map(|c| c.split_whitespace().find(|t| is_uuid(t)))?
                .to_string();
            let name = cols.first()?.to_string();
            let state = cols
                .iter()
                .position(|c| c.contains(&id))
                .and_then(|i| cols.get(i + 1))
                .unwrap_or(&"")
                .to_string();
            let paired = state.contains("paired") && !state.contains("unpaired");
            Some(Device {
                name,
                id,
                state,
                paired,
            })
        })
        .collect()
}

pub(crate) fn print_devices(devices: &[Device]) {
    let w = devices
        .iter()
        .map(|d| d.name.len())
        .max()
        .unwrap_or(0)
        .max(4);
    println!(
        "{:<w$}  {:<36}  {:<6}  STATE",
        "NAME", "IDENTIFIER", "PAIRED"
    );
    for d in devices {
        let paired = if d.paired { "yes" } else { "no" };
        println!("{:<w$}  {:<36}  {:<6}  {}", d.name, d.id, paired, d.state);
    }
}

pub(crate) fn show_devices() -> Result<(), String> {
    let devices = list_devices()?;
    print_devices(&devices);
    println!(
        "hint: copy an identifier into .env as SWIMLIE_PHONE= (iPhone) and SWIMLIE_WATCH= (watch)"
    );
    Ok(())
}

pub(crate) fn find_device<'a>(devices: &'a [Device], key: &str) -> Option<&'a Device> {
    devices
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(key) || d.name.eq_ignore_ascii_case(key))
}

/// The iPhone to install on: `--phone`, then `SWIMLIE_PHONE`, then the only paired device.
pub(crate) fn resolve_phone<'a>(
    flag: Option<&str>,
    env_phone: Option<&str>,
    devices: &'a [Device],
) -> Result<&'a Device, String> {
    if let Some(key) = flag.or(env_phone) {
        let source = if flag.is_some() {
            "--phone"
        } else {
            "SWIMLIE_PHONE"
        };
        return find_device(devices, key).ok_or_else(|| {
            format!("no device matches `{key}` from {source}; see `cargo xtask devices`")
        });
    }
    let paired: Vec<&Device> = devices.iter().filter(|d| d.paired).collect();
    match paired.as_slice() {
        [only] => Ok(*only),
        _ => Err("pick one with --phone <name|udid> or set SWIMLIE_PHONE in .env (exactly one paired device is used automatically)".into()),
    }
}

pub(crate) fn install(args: &[String]) -> Result<(), String> {
    let phone = match args {
        [] => None,
        [flag, value] if flag == "--phone" => Some(value.as_str()),
        _ => usage_exit(),
    };

    project()?;
    let devices = list_devices()?;
    let env_phone = setting("SWIMLIE_PHONE");
    let device = match resolve_phone(phone, env_phone.as_deref(), &devices) {
        Ok(d) => d,
        Err(e) => {
            print_devices(&devices);
            return Err(e);
        }
    };

    let derived = "build/derived-device";
    run_logged(
        Command::new("xcodebuild").args([
            "-project",
            PROJECT,
            "-scheme",
            IOS_SCHEME,
            "-configuration",
            "Debug",
            "-destination",
            "generic/platform=iOS",
            "-allowProvisioningUpdates",
            "-derivedDataPath",
            derived,
            "build",
        ]),
        "build/install-build.log",
    )?;
    let app = format!("{derived}/Build/Products/Debug-iphoneos/{IOS_SCHEME}.app");
    run(Command::new("xcrun").args([
        "devicectl",
        "device",
        "install",
        "app",
        "--device",
        &device.id,
        &app,
    ]))?;
    println!("installed {IOS_SCHEME} on {}", device.name);
    // The phone does not always push a new watch app across, so install it on the watch directly.
    // devicectl resolves the name or id itself, so this works even when the device list left the watch out.
    if let Some(key) = setting("SWIMLIE_WATCH") {
        let id = find_device(&devices, &key).map_or(key.as_str(), |d| d.id.as_str());
        let watch_app = format!("{app}/Watch/SwimlieWatch.app");
        match run(Command::new("xcrun").args([
            "devicectl",
            "device",
            "install",
            "app",
            "--device",
            id,
            &watch_app,
        ])) {
            Ok(()) => println!("installed the watch app on {key}"),
            Err(e) => eprintln!("warning: watch install failed: {e}"),
        }
    }
    Ok(())
}
