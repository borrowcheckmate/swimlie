#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "pixel geometry and colour channels read best short, and drawing is a list of draw calls"
)]
//! `cargo xtask mockups`: README device mockups, drawn in pure Rust.
//!
//! Reads the screenshots in build/ and writes docs/hero.png, docs/phones.png,
//! docs/watch.png and docs/icon.png. Frames are signed-distance shapes with
//! one-pixel anti-aliasing, so the screenshot fills the screen mask exactly.

use std::{fs, path::Path};

use image::{
    Rgba, RgbaImage,
    imageops::{self, FilterType},
};

const PHONE_NAMES: [&str; 5] = ["history", "detail", "trends", "plan", "creator"];
const WORKOUT_NAMES: [&str; 4] = ["workouts", "generate", "drills", "rounds"];
const BUILDER_NAMES: [&str; 5] = [
    "creator",
    "builder-2",
    "builder-3",
    "builder-4",
    "builder-5",
];
const DETAIL_NAMES: [&str; 4] = ["detail", "detail-hr", "detail-pace", "detail-sets"];
const AI_NAMES: [&str; 3] = ["settings-ai", "ai-generate", "ai-review"];
const WATCH_NAMES: [&str; 4] = ["live-rest", "live-rep", "after-next", "summary"];
const ICON: &str = "apple/iOS/Assets.xcassets/AppIcon.appiconset/AppIcon.png";
const OUT_DIR: &str = "docs";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Phone,
    Watch,
}

pub(crate) fn run() -> Result<(), String> {
    let phones = load_named("build/phone-", &PHONE_NAMES)?;
    let watches = load_named("build/ui-", &WATCH_NAMES)?;
    let workouts = load_named("build/phone-", &WORKOUT_NAMES)?;
    let details = load_named("build/phone-", &DETAIL_NAMES)?;
    let builder = load_named("build/phone-", &BUILDER_NAMES)?;
    let ai = load_named("build/phone-", &AI_NAMES)?;
    fs::create_dir_all(OUT_DIR).map_err(|e| format!("cannot create {OUT_DIR}: {e}"))?;

    if phones.is_empty() {
        eprintln!("mockups: no build/phone-<name>.png shots; skipping hero.png and phones.png");
    } else {
        save("docs/hero.png", &hero(&phones, &watches))?;
        save("docs/phones.png", &phones_strip(&phones))?;
    }
    if !details.is_empty() {
        save("docs/details.png", &phones_strip(&details))?;
    }
    if !builder.is_empty() {
        save("docs/builder.png", &phones_strip(&builder))?;
    }
    if !ai.is_empty() {
        save("docs/ai.png", &phones_strip(&ai))?;
    }
    if !workouts.is_empty() {
        save("docs/workouts.png", &phones_strip(&workouts))?;
    }
    if watches.is_empty() {
        eprintln!("mockups: no build/ui-<name>.png watch shots; skipping watch.png");
    } else {
        save("docs/watch.png", &watch_strip(&watches))?;
    }
    match load_one(ICON)? {
        Some(src) => save("docs/icon.png", &icon(&src))?,
        None => eprintln!("mockups: {ICON} missing; skipping icon.png"),
    }
    Ok(())
}

fn load_one(path: &str) -> Result<Option<RgbaImage>, String> {
    if !Path::new(path).exists() {
        return Ok(None);
    }
    let img = image::open(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    Ok(Some(img.to_rgba8()))
}

fn load_named(
    prefix: &str,
    names: &[&'static str],
) -> Result<Vec<(&'static str, RgbaImage)>, String> {
    let mut out = Vec::new();
    for name in names {
        if let Some(img) = load_one(&format!("{prefix}{name}.png"))? {
            out.push((*name, img));
        }
    }
    Ok(out)
}

fn save(path: &str, img: &RgbaImage) -> Result<(), String> {
    img.save(path)
        .map_err(|e| format!("cannot write {path}: {e}"))?;
    println!("mockups: wrote {path}");
    Ok(())
}

// ---- layout -------------------------------------------------------------

fn hero(phones: &[(&str, RgbaImage)], watches: &[(&str, RgbaImage)]) -> RgbaImage {
    let (w, h) = (2400, 1300);
    let mut canvas = background(w, h);

    let center_idx = phones.iter().position(|(n, _)| *n == "detail").unwrap_or(0);
    let mut sides: Vec<&RgbaImage> = Vec::new();
    for want in ["history", "plan"] {
        if let Some(i) = phones.iter().position(|(n, _)| *n == want)
            && i != center_idx
        {
            sides.push(&phones[i].1);
        }
    }
    for (i, (_, p)) in phones.iter().enumerate() {
        if sides.len() >= 2 {
            break;
        }
        if i != center_idx && !sides.iter().any(|s| std::ptr::eq(*s, p)) {
            sides.push(p);
        }
    }

    let cy = 650;
    if let Some(left) = sides.first() {
        canvas.device_at(&scale_h(&framed(left, Kind::Phone), 1000), 680, cy);
    }
    if let Some(right) = sides.get(1) {
        canvas.device_at(&scale_h(&framed(right, Kind::Phone), 1000), 1720, cy);
    }
    canvas.device_at(
        &scale_h(&framed(&phones[center_idx].1, Kind::Phone), 1200),
        1200,
        cy,
    );

    if let Some((_, watch)) = watches
        .iter()
        .find(|(n, _)| *n == "live-rep")
        .or_else(|| watches.first())
    {
        canvas.device_at(&scale_w(&framed(watch, Kind::Watch), 340), 1960, 1010);
    }
    canvas.into_rgba8()
}

fn phones_strip(phones: &[(&str, RgbaImage)]) -> RgbaImage {
    let frames: Vec<RgbaImage> = phones.iter().map(|(_, p)| framed(p, Kind::Phone)).collect();
    strip(&frames)
}

fn watch_strip(watches: &[(&str, RgbaImage)]) -> RgbaImage {
    let frames: Vec<RgbaImage> = watches
        .iter()
        .map(|(_, p)| framed(p, Kind::Watch))
        .collect();
    strip(&frames)
}

/// One row of equally spaced device frames on the brand background.
fn strip(frames: &[RgbaImage]) -> RgbaImage {
    let (cw, margin) = (2400u32, 120u32);
    let slot = (cw - 2 * margin) / frames.len() as u32;
    let scaled: Vec<RgbaImage> = frames.iter().map(|f| scale_w(f, slot * 9 / 10)).collect();
    let ch = scaled
        .iter()
        .map(image::ImageBuffer::height)
        .max()
        .unwrap_or(0)
        + 2 * margin;
    let mut canvas = background(cw, ch);
    for (i, dev) in scaled.iter().enumerate() {
        let x = margin + i as u32 * slot + (slot - dev.width()) / 2;
        let y = (ch - dev.height()) / 2;
        canvas.device(dev, i64::from(x), i64::from(y));
    }
    canvas.into_rgba8()
}

/// The app icon at 512 px with squircle-ish corners (radius 22.5%) and transparent corners.
fn icon(src: &RgbaImage) -> RgbaImage {
    let s = 512u32;
    let mut img = imageops::resize(src, s, s, FilterType::Lanczos3);
    let half = s as f32 / 2.0;
    let radius = 0.225 * s as f32;
    for (x, y, p) in img.enumerate_pixels_mut() {
        let cov = rrect_cov(
            x as f32 + 0.5,
            y as f32 + 0.5,
            half,
            half,
            half,
            half,
            radius,
        );
        p[3] = (f32::from(p[3]) * cov).round() as u8;
    }
    img
}

// ---- device frames ------------------------------------------------------

/// Wraps a screenshot in a device frame. The result has a transparent margin around the body
/// for the side buttons and the anti-aliased edge, so it scales and places as one image.
fn framed(shot: &RgbaImage, kind: Kind) -> RgbaImage {
    let (sw, sh) = (shot.width(), shot.height());
    let (bezel_frac, screen_r_frac, ring_frac, ring_top, ring_bot, body_rgb) = match kind {
        Kind::Phone => (
            0.032,
            0.13,
            0.006,
            hex(0x008A_8F98),
            hex(0x002A_2C30),
            hex(0x000B_0C0F),
        ),
        Kind::Watch => (
            0.08,
            0.18,
            0.012,
            hex(0x005A_5F68),
            hex(0x001C_1D21),
            hex(0x000A_0A0C),
        ),
    };
    // Bezel in whole pixels, so the screen sits on the pixel grid.
    let bezel = (sw as f32 * bezel_frac / 2.0f32.mul_add(-bezel_frac, 1.0)).round();
    let ow = 2.0f32.mul_add(bezel, sw as f32);
    let oh = 2.0f32.mul_add(bezel, sh as f32);
    let pad = (0.05 * ow).round();
    let (fw, fh) = (
        2.0f32.mul_add(pad, ow) as u32,
        2.0f32.mul_add(pad, oh) as u32,
    );
    let (cx, cy) = (pad + ow / 2.0, pad + oh / 2.0);
    let screen_r = screen_r_frac * ow;
    let body_r = match kind {
        Kind::Phone => screen_r + bezel,
        Kind::Watch => 0.24 * ow,
    };
    let ring = (ring_frac * ow).round();
    let (sx0, sy0) = ((pad + bezel) as i64, (pad + bezel) as i64);

    // Phone dynamic island: a pill at the top of the screen.
    let island_hw = 0.15 * sw as f32;
    let island_hh = 0.047 * sw as f32;
    let island_cx = sx0 as f32 + sw as f32 / 2.0;
    let island_cy = 0.02f32.mul_add(sw as f32, sy0 as f32) + island_hh;

    let mut out = Canvas::new(fw, fh);
    for y in 0..fh {
        for x in 0..fw {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut c = [0.0f32; 4];

            if kind == Kind::Watch {
                // Digital crown and side button, half hidden under the body.
                for (cy_frac, hh_frac) in [(0.34, 0.075), (0.52, 0.05)] {
                    let cov = rrect_cov(
                        px,
                        py,
                        pad + ow,
                        oh.mul_add(cy_frac, pad),
                        0.02 * ow,
                        hh_frac * ow,
                        0.02 * ow,
                    );
                    lay(&mut c, hex(0x0050_545D), cov);
                }
            }

            let outer = rrect_cov(px, py, cx, cy, ow / 2.0, oh / 2.0, body_r);
            if outer > 0.0 {
                let t = ((py - pad) / oh).clamp(0.0, 1.0);
                lay(&mut c, lerp(ring_top, ring_bot, t), outer);
            }
            let inner = rrect_cov(
                px,
                py,
                cx,
                cy,
                ow / 2.0 - ring,
                oh / 2.0 - ring,
                (body_r - ring).max(0.0),
            );
            if inner > 0.0 {
                lay(&mut c, body_rgb, inner);
            }

            let screen = rrect_cov(px, py, cx, cy, sw as f32 / 2.0, sh as f32 / 2.0, screen_r);
            if screen > 0.0 {
                let ix = (i64::from(x) - sx0).clamp(0, i64::from(sw) - 1) as u32;
                let iy = (i64::from(y) - sy0).clamp(0, i64::from(sh) - 1) as u32;
                let p = shot.get_pixel(ix, iy);
                let a = screen * f32::from(p[3]) / 255.0;
                lay(
                    &mut c,
                    [
                        f32::from(p[0]) / 255.0,
                        f32::from(p[1]) / 255.0,
                        f32::from(p[2]) / 255.0,
                    ],
                    a,
                );
            }

            if kind == Kind::Phone {
                let cov = rrect_cov(
                    px, py, island_cx, island_cy, island_hw, island_hh, island_hh,
                );
                lay(&mut c, [0.0; 3], cov);
            }

            out.px[(y * fw + x) as usize] = c;
        }
    }
    out.into_rgba8()
}

// ---- geometry and colour ------------------------------------------------

/// Coverage (0..1) of a pixel centre against a rounded rectangle, anti-aliased over one pixel.
fn rrect_cov(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let r = r.min(hw).min(hh);
    let qx = (px - cx).abs() - (hw - r);
    let qy = (py - cy).abs() - (hh - r);
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    let inside = qx.max(qy).min(0.0);
    let sd = outside + inside - r;
    (0.5 - sd).clamp(0.0, 1.0)
}

fn hex(v: u32) -> [f32; 3] {
    [
        ((v >> 16) & 255) as f32,
        ((v >> 8) & 255) as f32,
        (v & 255) as f32,
    ]
    .map(|c| c / 255.0)
}

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| (b[i] - a[i]).mul_add(t, a[i]))
}

/// Source-over a straight-colour layer with coverage `a` onto a premultiplied pixel.
fn lay(dst: &mut [f32; 4], rgb: [f32; 3], a: f32) {
    let k = 1.0 - a;
    for i in 0..3 {
        dst[i] = dst[i].mul_add(k, rgb[i] * a);
    }
    dst[3] = dst[3].mul_add(k, a);
}

/// Brand background: diagonal gradient, with a soft light glow.
fn background(w: u32, h: u32) -> Canvas {
    const STOPS: [(f32, [f32; 3]); 3] = [
        (0.0, [10.0, 15.0, 46.0]),
        (0.5, [10.0, 43.0, 214.0]),
        (1.0, [0.0, 184.0, 255.0]),
    ];
    let mut canvas = Canvas::new(w, h);
    let (gx, gy) = (0.35 * w as f32, 0.3 * h as f32);
    let glow_r = 0.6 * w.max(h) as f32;
    for y in 0..h {
        for x in 0..w {
            let t = (x as f32 + y as f32) / (w + h) as f32;
            let (a, b) = if t < 0.5 {
                (STOPS[0], STOPS[1])
            } else {
                (STOPS[1], STOPS[2])
            };
            let k = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
            let mut rgb = [0, 1, 2].map(|i| f32::mul_add(b.1[i] - a.1[i], k, a.1[i]) / 255.0);
            let d = (x as f32 - gx).hypot(y as f32 - gy) / glow_r;
            let glow = 0.22 * (1.0 - d).clamp(0.0, 1.0).powi(2);
            rgb = lerp(rgb, [1.0; 3], glow);
            canvas.px[(y * w + x) as usize] = [rgb[0], rgb[1], rgb[2], 1.0];
        }
    }
    canvas
}

fn scale_h(img: &RgbaImage, h: u32) -> RgbaImage {
    let w = (img.width() as f32 * h as f32 / img.height() as f32).round() as u32;
    imageops::resize(img, w, h, FilterType::Lanczos3)
}

fn scale_w(img: &RgbaImage, w: u32) -> RgbaImage {
    let h = (img.height() as f32 * w as f32 / img.width() as f32).round() as u32;
    imageops::resize(img, w, h, FilterType::Lanczos3)
}

// ---- compositing --------------------------------------------------------

/// Premultiplied RGBA canvas in 0..1.
struct Canvas {
    w: u32,
    h: u32,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![[0.0; 4]; (w * h) as usize],
        }
    }

    /// Source-over a premultiplied colour at (x, y).
    fn over(&mut self, x: i64, y: i64, src: [f32; 4]) {
        if x < 0 || y < 0 || x >= i64::from(self.w) || y >= i64::from(self.h) {
            return;
        }
        let d = &mut self.px[y as usize * self.w as usize + x as usize];
        let k = 1.0 - src[3];
        for i in 0..4 {
            d[i] = d[i].mul_add(k, src[i]);
        }
    }

    fn paste(&mut self, img: &RgbaImage, x: i64, y: i64) {
        for (px, py, p) in img.enumerate_pixels() {
            let a = f32::from(p[3]) / 255.0;
            if a == 0.0 {
                continue;
            }
            let src = [
                f32::from(p[0]) / 255.0 * a,
                f32::from(p[1]) / 255.0 * a,
                f32::from(p[2]) / 255.0 * a,
                a,
            ];
            self.over(x + i64::from(px), y + i64::from(py), src);
        }
    }

    /// Places a device centred on (cx, cy), with its shadow.
    fn device_at(&mut self, dev: &RgbaImage, cx: i64, cy: i64) {
        self.device(
            dev,
            cx - i64::from(dev.width()) / 2,
            cy - i64::from(dev.height()) / 2,
        );
    }

    /// Places a device with its top-left at (x, y), with its drop shadow.
    fn device(&mut self, dev: &RgbaImage, x: i64, y: i64) {
        let blur = (dev.width() as f32 * 0.03).round().max(3.0) as usize;
        let dy = (dev.width() as f32 * 0.03).round() as i64;
        self.shadow(dev, x, y, blur, dy, 0.6);
        self.paste(dev, x, y);
    }

    /// Black drop shadow from the alpha of `dev`, blurred by `blur` px and pushed down by `dy`.
    fn shadow(&mut self, dev: &RgbaImage, x: i64, y: i64, blur: usize, dy: i64, opacity: f32) {
        let m = 3 * blur;
        let (w, h) = (dev.width() as usize + 2 * m, dev.height() as usize + 2 * m);
        let mut a = vec![0.0f32; w * h];
        for (px, py, p) in dev.enumerate_pixels() {
            a[(py as usize + m) * w + px as usize + m] = f32::from(p[3]) / 255.0;
        }
        // Three box blurs approximate a gaussian.
        for _ in 0..3 {
            a = blur_axis(&a, w, h, blur, true);
            a = blur_axis(&a, w, h, blur, false);
        }
        for yy in 0..h {
            for xx in 0..w {
                let v = a[yy * w + xx] * opacity;
                if v > 0.0 {
                    self.over(
                        x - m as i64 + xx as i64,
                        y - m as i64 + yy as i64 + dy,
                        [0.0, 0.0, 0.0, v],
                    );
                }
            }
        }
    }

    fn into_rgba8(self) -> RgbaImage {
        let mut out = RgbaImage::new(self.w, self.h);
        for (p, o) in self.px.iter().zip(out.pixels_mut()) {
            *o = unpremultiply(*p);
        }
        out
    }
}

fn unpremultiply(p: [f32; 4]) -> Rgba<u8> {
    let a = p[3];
    let c = |v: f32| {
        if a > 0.0 {
            (v / a * 255.0).round().clamp(0.0, 255.0) as u8
        } else {
            0
        }
    };
    Rgba([
        c(p[0]),
        c(p[1]),
        c(p[2]),
        (a * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

/// One box blur pass of radius `r` along rows (`horizontal`) or columns, zero outside the image.
fn blur_axis(src: &[f32], w: usize, h: usize, r: usize, horizontal: bool) -> Vec<f32> {
    let (n, lines, stride, step) = if horizontal {
        (w, h, 1, w)
    } else {
        (h, w, w, 1)
    };
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut out = vec![0.0f32; src.len()];
    for l in 0..lines {
        let at = |i: usize| l * step + i * stride;
        let mut sum: f32 = (0..=r.min(n - 1)).map(|i| src[at(i)]).sum();
        for k in 0..n {
            out[at(k)] = sum * norm;
            if k + r + 1 < n {
                sum += src[at(k + r + 1)];
            }
            if k >= r {
                sum -= src[at(k - r)];
            }
        }
    }
    out
}
