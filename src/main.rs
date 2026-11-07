static DEBUG: AtomicBool = AtomicBool::new(false);

macro_rules! debug {
    ($($arg:tt)*) => {
        if crate::DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!($($arg)*);
        }
    };
}

mod hidpp;
mod keys;

use hidapi::HidApi;
use hidpp::Device;
use keys::Combo;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Duration;

const USAGE: &str = "usage: tinylogi [--once | --list] [--debug] [config]

  (default)  apply config, then stay in the foreground handling buttons
  --once     apply dpi/smartshift/wheel/thumbwheel settings and exit
  --list     print the device's remappable buttons
  --debug    log HID++ traffic, button events and key presses to stderr

config defaults to ~/.config/tinylogi.toml";

// Control IDs shared by the MX Master line. Others can be given as hex, e.g. "0x01a0".
const BUTTONS: &[(&str, u16)] = &[
    ("middle", 0x52),
    ("back", 0x53),
    ("forward", 0x56),
    ("thumb", 0xc3),
    ("shift", 0xc4),
];

// Movement (in sensor counts) a gesture button must exceed to count as a swipe rather than a tap.
const SWIPE_THRESHOLD: i32 = 50;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Config {
    device: Option<String>,
    dpi: Option<u16>,
    smartshift: Option<SmartShift>,
    wheel: Option<Wheel>,
    thumbwheel: Option<ThumbWheel>,
    #[serde(default)]
    buttons: BTreeMap<String, Binding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SmartShift {
    mode: Option<WheelMode>,
    threshold: Option<u8>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum WheelMode {
    Freespin = 1,
    Ratchet = 2,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wheel {
    hires: Option<bool>,
    invert: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThumbWheel {
    invert: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Binding {
    Key(String),
    Gesture {
        tap: Option<String>,
        up: Option<String>,
        down: Option<String>,
        left: Option<String>,
        right: Option<String>,
    },
}

enum Action {
    Key(Combo),
    Gesture([Option<Combo>; 5]), // tap, up, down, left, right
}

/// Runtime indices of the features the event loop needs to recognise notifications.
struct Features {
    reprog: Option<u8>,
    wireless: Option<u8>,
}

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: i32) {
    STOP.store(true, Relaxed);
}

unsafe extern "C" {
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
}

fn main() {
    if let Err(e) = run() {
        eprintln!("tinylogi: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (mut once, mut list, mut path) = (false, false, None);
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--once" => once = true,
            "--list" => list = true,
            "--debug" => DEBUG.store(true, Relaxed),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ if arg.starts_with('-') => return Err(format!("unknown flag {arg}\n{USAGE}")),
            _ => path = Some(arg),
        }
    }
    let path = path.unwrap_or_else(|| {
        format!(
            "{}/.config/tinylogi.toml",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let cfg: Config = match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).map_err(|e| format!("{path}: {e}"))?,
        Err(_) if list => Config::default(),
        Err(e) => return Err(format!("{path}: {e}")),
    };
    let bindings = if once { vec![] } else { bindings(&cfg)? };
    if !bindings.is_empty() && !keys::trusted() {
        eprintln!(
            "warning: no Accessibility permission, key presses will be dropped \
             (System Settings → Privacy & Security → Accessibility)"
        );
    }

    unsafe {
        signal(2, on_signal); // SIGINT
        signal(15, on_signal); // SIGTERM
    }
    if !hidpp::request_access() {
        return Err(
            "no Input Monitoring permission; grant it to this terminal (or binary) in \
                    System Settings → Privacy & Security → Input Monitoring, then restart it"
                .into(),
        );
    }
    let mut api = HidApi::new().map_err(|e| e.to_string())?;
    let mut last_err = String::new();
    loop {
        match session(&api, &cfg, &bindings, once, list) {
            Ok(()) => return Ok(()),
            Err(e) if once || list => return Err(e),
            Err(e) if e != last_err => {
                eprintln!("{e}, waiting for device");
                last_err = e;
            }
            Err(_) => {}
        }
        for _ in 0..20 {
            if STOP.load(Relaxed) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        api.refresh_devices().map_err(|e| e.to_string())?;
    }
}

fn bindings(cfg: &Config) -> Result<Vec<(u16, Action)>, String> {
    let mut out = vec![];
    for (name, binding) in &cfg.buttons {
        let ctx = |e: String| format!("buttons.{name}: {e}");
        let cid = BUTTONS
            .iter()
            .find(|b| b.0 == name)
            .map(|b| b.1)
            .or_else(|| u16::from_str_radix(name.strip_prefix("0x")?, 16).ok())
            .ok_or(ctx(
                "unknown button, use a name or hex id from --list".into()
            ))?;
        let action = match binding {
            Binding::Key(s) => Action::Key(keys::parse(s).map_err(ctx)?),
            Binding::Gesture {
                tap,
                up,
                down,
                left,
                right,
            } => {
                let mut dirs = [None; 5];
                for (slot, s) in dirs.iter_mut().zip([tap, up, down, left, right]) {
                    if let Some(s) = s {
                        *slot = Some(keys::parse(s).map_err(ctx)?);
                    }
                }
                Action::Gesture(dirs)
            }
        };
        out.push((cid, action));
    }
    Ok(out)
}

fn session(
    api: &HidApi,
    cfg: &Config,
    bindings: &[(u16, Action)],
    once: bool,
    list: bool,
) -> Result<(), String> {
    let dev = Device::open(api, cfg.device.as_deref())?;
    if list {
        return list_controls(&dev);
    }
    let mut feats = apply(&dev, cfg, bindings)?;
    eprintln!("configured {}", dev.name);
    if once {
        return Ok(());
    }
    listen(&dev, &mut feats, cfg, bindings)?;
    // Diversion outlives us until the mouse reconnects, so hand the buttons back on exit.
    if let Some(f) = feats.reprog {
        for (cid, action) in bindings {
            let [hi, lo] = cid.to_be_bytes();
            dev.call(f, 3, &[hi, lo, reporting_flags(action, false)])?;
        }
    }
    Ok(())
}

fn need(dev: &Device, id: u16, what: &str) -> Result<u8, String> {
    dev.feature(id)?.ok_or(format!(
        "{} does not support {what} (feature {id:#06x})",
        dev.name
    ))
}

// setCidReporting flags: divert (0x01) and raw XY (0x10), each with its "valid" bit set.
fn reporting_flags(action: &Action, on: bool) -> u8 {
    let bits = match action {
        Action::Key(_) => 0x02,
        Action::Gesture(_) => 0x22,
    };
    if on { bits | bits >> 1 } else { bits }
}

fn apply(dev: &Device, cfg: &Config, bindings: &[(u16, Action)]) -> Result<Features, String> {
    if let Some(dpi) = cfg.dpi {
        let f = need(dev, hidpp::ADJUSTABLE_DPI, "dpi")?;
        let [hi, lo] = dpi.to_be_bytes();
        dev.call(f, 3, &[0, hi, lo])?;
    }
    if let Some(s) = &cfg.smartshift {
        // 0 means "leave unchanged" for both fields.
        let params = [s.mode.map_or(0, |m| m as u8), s.threshold.unwrap_or(0)];
        match dev.feature(hidpp::SMART_SHIFT_ENHANCED)? {
            Some(f) => dev.call(f, 2, &params)?,
            None => dev.call(need(dev, hidpp::SMART_SHIFT, "smartshift")?, 1, &params)?,
        };
    }
    if let Some(w) = &cfg.wheel {
        let f = need(dev, hidpp::HIRES_WHEEL, "wheel settings")?;
        let mut mode = dev.call(f, 1, &[])?[4];
        for (bit, on) in [(0x02, w.hires), (0x04, w.invert)] {
            match on {
                Some(true) => mode |= bit,
                Some(false) => mode &= !bit,
                None => {}
            }
        }
        dev.call(f, 2, &[mode])?;
    }
    if let Some(t) = &cfg.thumbwheel {
        let f = need(dev, hidpp::THUMB_WHEEL, "thumb wheel settings")?;
        // Status is [diverted, inverted]; keep the reporting mode, set only the direction.
        let diverted = dev.call(f, 1, &[])?[4];
        dev.call(f, 2, &[diverted, t.invert as u8])?;
    }
    let mut reprog = None;
    if !bindings.is_empty() {
        let f = need(dev, hidpp::REPROG_CONTROLS, "button remapping")?;
        for (cid, action) in bindings {
            let [hi, lo] = cid.to_be_bytes();
            dev.call(f, 3, &[hi, lo, reporting_flags(action, true)])
                .map_err(|e| format!("diverting button {cid:#06x}: {e}"))?;
        }
        reprog = Some(f);
    }
    let wireless = dev.feature(hidpp::WIRELESS_STATUS)?;
    Ok(Features { reprog, wireless })
}

fn listen(
    dev: &Device,
    feats: &mut Features,
    cfg: &Config,
    bindings: &[(u16, Action)],
) -> Result<(), String> {
    let lookup = |cid| bindings.iter().find(|b| b.0 == cid).map(|b| &b.1);
    let mut pressed: Vec<u16> = vec![];
    let mut gesture: Option<(u16, i32, i32)> = None;

    while !STOP.load(Relaxed) {
        let Some(r) = dev.read(500)? else { continue };
        if r[3] & 0x0f != 0 {
            continue; // a late response to one of our requests, not a notification
        }
        let event = r[3] >> 4;
        if Some(r[2]) == feats.wireless && event == 0 && r[4] == 1 {
            eprintln!("{} reconnected, reapplying", dev.name);
            *feats = apply(dev, cfg, bindings)?;
            (pressed, gesture) = (vec![], None);
        } else if Some(r[2]) == feats.reprog && event == 0 {
            // Diverted buttons event: the up-to-four buttons currently held.
            let now: Vec<u16> = r[4..12]
                .chunks(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .filter(|&c| c != 0)
                .collect();
            for &cid in now.iter().filter(|c| !pressed.contains(c)) {
                debug!("button {cid:#06x} down");
                match lookup(cid) {
                    Some(Action::Key(combo)) => keys::press(*combo),
                    Some(Action::Gesture(_)) => gesture = Some((cid, 0, 0)),
                    None => {}
                }
            }
            for &cid in pressed.iter().filter(|c| !now.contains(c)) {
                debug!("button {cid:#06x} up");
                if let (Some((g, x, y)), Some(Action::Gesture(dirs))) = (gesture, lookup(cid))
                    && g == cid
                {
                    gesture = None;
                    let dir = direction(x, y);
                    debug!(
                        "gesture moved ({x}, {y}) -> {}",
                        ["tap", "up", "down", "left", "right"][dir]
                    );
                    if let Some(combo) = dirs[dir] {
                        keys::press(combo);
                    }
                }
            }
            pressed = now;
        } else if Some(r[2]) == feats.reprog && event == 1 {
            // Raw XY event: movement while a gesture button is held.
            if let Some((_, x, y)) = &mut gesture {
                *x += i16::from_be_bytes([r[4], r[5]]) as i32;
                *y += i16::from_be_bytes([r[6], r[7]]) as i32;
            }
        }
    }
    Ok(())
}

/// Index into a gesture's [tap, up, down, left, right].
fn direction(x: i32, y: i32) -> usize {
    match (x.abs(), y.abs()) {
        (ax, ay) if ax.max(ay) < SWIPE_THRESHOLD => 0,
        (ax, ay) if ay > ax => {
            if y < 0 {
                1
            } else {
                2
            }
        }
        _ => {
            if x < 0 {
                3
            } else {
                4
            }
        }
    }
}

fn list_controls(dev: &Device) -> Result<(), String> {
    let f = need(dev, hidpp::REPROG_CONTROLS, "button remapping")?;
    println!("{}", dev.name);
    for i in 0..dev.call(f, 0, &[])?[4] {
        let r = dev.call(f, 1, &[i])?;
        let cid = u16::from_be_bytes([r[4], r[5]]);
        let name = BUTTONS.iter().find(|b| b.1 == cid).map_or("", |b| b.0);
        let divertable = if r[8] & 0x20 != 0 {
            "remappable"
        } else {
            "fixed"
        };
        let gesture = if r[12] & 0x01 != 0 { ", gestures" } else { "" };
        println!("  {cid:#06x}  {name:8} {divertable}{gesture}");
    }
    Ok(())
}
