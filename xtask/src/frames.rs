use super::{Command, Path, Stdio, fs, run};

pub(crate) struct FramesOpts {
    pub(super) video: String,
    pub(super) fps: f64,
    pub(super) cols: u32,
    pub(super) out: Option<String>,
}

pub(crate) fn parse_frames(args: &[String]) -> Result<FramesOpts, String> {
    const USAGE_LINE: &str = "usage: cargo xtask frames <video> [--fps N] [--cols N] [--out <png>]";
    let (video, flags) = match args.split_first() {
        Some((v, rest)) if !v.starts_with("--") => (v.clone(), rest),
        _ => return Err(USAGE_LINE.into()),
    };
    let mut o = FramesOpts {
        video,
        fps: 1.0,
        cols: 5,
        out: None,
    };
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--fps" => {
                o.fps = value()?
                    .parse::<f64>()
                    .ok()
                    .filter(|f| f.is_finite() && *f > 0.0)
                    .ok_or("--fps needs a positive number")?;
            }
            "--cols" => {
                o.cols = value()?
                    .parse::<u32>()
                    .ok()
                    .filter(|c| *c > 0)
                    .ok_or("--cols needs a positive integer")?;
            }
            "--out" => o.out = Some(value()?),
            _ => return Err(format!("unknown option {flag}; {USAGE_LINE}")),
        }
    }
    Ok(o)
}

/// Frames sampled at `fps` over `duration` seconds, and the rows needed to tile them into `cols` columns.
pub(crate) fn sheet_shape(duration: f64, fps: f64, cols: u32) -> (u32, u32) {
    let frames = (duration * fps).ceil().max(1.0) as u32;
    (frames, frames.div_ceil(cols))
}

pub(crate) fn check_tool(name: &str) -> Result<(), String> {
    match Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(s) if s.success() => Ok(()),
        _ => Err(format!(
            "{name} not found; install it with `brew install ffmpeg`"
        )),
    }
}

pub(crate) fn frames(args: &[String]) -> Result<(), String> {
    let o = parse_frames(args)?;
    check_tool("ffmpeg")?;
    check_tool("ffprobe")?;
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            &o.video,
        ])
        .output()
        .map_err(|e| format!("failed to start ffprobe: {e}"))?;
    if !probe.status.success() {
        return Err(format!("ffprobe cannot read {}", o.video));
    }
    let duration: f64 = String::from_utf8_lossy(&probe.stdout)
        .trim()
        .parse()
        .map_err(|_| format!("ffprobe gave no duration for {}", o.video))?;
    let stem = Path::new(&o.video)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let out = o.out.unwrap_or_else(|| format!("build/{stem}-sheet.png"));
    if let Some(dir) = Path::new(&out)
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
    {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let (_, rows) = sheet_shape(duration, o.fps, o.cols);
    let filter = format!("fps={},scale=200:-1,tile={}x{}", o.fps, o.cols, rows);
    run(Command::new("ffmpeg").args([
        "-v",
        "error",
        "-y",
        "-i",
        &o.video,
        "-vf",
        &filter,
        "-frames:v",
        "1",
        &out,
    ]))?;
    let path = Path::new(&out)
        .canonicalize()
        .map_err(|e| format!("cannot read {out}: {e}"))?;
    println!("sheet: {}", path.display());
    Ok(())
}
