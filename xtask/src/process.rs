use super::{Command, ErrorKind, File, Path, USAGE, exit, fs};

pub(crate) fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}");
    exit(1)
}

pub(crate) fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    exit(2)
}

pub(crate) fn describe(cmd: &Command) -> String {
    let mut s = cmd.get_program().to_string_lossy().into_owned();
    for a in cmd.get_args() {
        s.push(' ');
        s.push_str(&a.to_string_lossy());
    }
    s
}

/// Runs a command with inherited output and fails loudly on a non-zero exit.
pub(crate) fn run(cmd: &mut Command) -> Result<(), String> {
    let desc = describe(cmd);
    println!("$ {desc}");
    let status = cmd
        .status()
        .map_err(|e| format!("failed to start `{desc}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{desc}` failed ({status})"))
    }
}

/// Runs a command with output going to a log file; on failure shows the log tail.
pub(crate) fn run_logged(cmd: &mut Command, log: &str) -> Result<(), String> {
    let log_path = Path::new(log);
    if let Some(dir) = log_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let file = File::create(log_path).map_err(|e| format!("cannot create {log}: {e}"))?;
    let err_file = file
        .try_clone()
        .map_err(|e| format!("cannot clone {log}: {e}"))?;
    let desc = describe(cmd);
    println!("$ {desc}  (log: {log})");
    let status = cmd
        .stdout(file)
        .stderr(err_file)
        .status()
        .map_err(|e| format!("failed to start `{desc}`: {e}"))?;
    if status.success() {
        return Ok(());
    }
    let text = fs::read_to_string(log_path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines[lines.len().saturating_sub(40)..] {
        eprintln!("{line}");
    }
    Err(format!("`{desc}` failed ({status}); full log: {log}"))
}

pub(crate) fn remove_dir_if_exists(path: &str) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove {path}: {e}")),
    }
}

/// Cargo and Xcode both read these; the Apple project's deployment targets match project.yml.
pub(crate) fn env_cmd(prog: &str) -> Command {
    let mut c = Command::new(prog);
    c.env("IPHONEOS_DEPLOYMENT_TARGET", "17.0")
        .env("WATCHOS_DEPLOYMENT_TARGET", "11.0");
    c
}
