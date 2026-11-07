// Parses "cmd+shift+4" style combos and posts them as synthetic key events via CoreGraphics.

use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut c_void);
    fn CFRelease(cf: *const c_void);
    fn AXIsProcessTrusted() -> bool;
}

const SHIFT: u64 = 0x20000;
const CONTROL: u64 = 0x40000;
const ALTERNATE: u64 = 0x80000;
const COMMAND: u64 = 0x100000;
const NUMERIC_PAD: u64 = 0x200000;
const SECONDARY_FN: u64 = 0x800000;

#[derive(Clone, Copy, Debug)]
pub struct Combo {
    key: u16,
    flags: u64,
}

pub fn parse(s: &str) -> Result<Combo, String> {
    let (mods, key) = s.rsplit_once('+').unwrap_or(("", s));
    let mut flags = 0;
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        flags |= match m {
            "cmd" | "command" => COMMAND,
            "ctrl" | "control" => CONTROL,
            "alt" | "opt" | "option" => ALTERNATE,
            "shift" => SHIFT,
            "fn" => SECONDARY_FN,
            _ => return Err(format!("unknown modifier '{m}' in '{s}'")),
        };
    }
    let key = keycode(key).ok_or(format!("unknown key '{key}' in '{s}'"))?;
    // Real keyboards flag these keys this way; some system shortcuts (e.g. ctrl+arrow) need it.
    if key >= 0x60 {
        flags |= SECONDARY_FN;
    }
    if (0x7b..=0x7e).contains(&key) {
        flags |= NUMERIC_PAD;
    }
    Ok(Combo { key, flags })
}

pub fn press(c: Combo) {
    debug!("key {:#04x} flags {:#x}", c.key, c.flags);
    for down in [true, false] {
        unsafe {
            let ev = CGEventCreateKeyboardEvent(std::ptr::null(), c.key, down);
            if ev.is_null() {
                return;
            }
            CGEventSetFlags(ev, c.flags);
            CGEventPost(0, ev); // kCGHIDEventTap
            CFRelease(ev);
        }
    }
}

/// Synthetic key events are silently dropped unless the process has Accessibility permission.
pub fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[rustfmt::skip]
// macOS virtual keycodes (ANSI layout), from HIToolbox/Events.h.
fn keycode(name: &str) -> Option<u16> {
    Some(match name {
        "a" => 0x00, "s" => 0x01, "d" => 0x02, "f" => 0x03, "h" => 0x04, "g" => 0x05,
        "z" => 0x06, "x" => 0x07, "c" => 0x08, "v" => 0x09, "b" => 0x0b, "q" => 0x0c,
        "w" => 0x0d, "e" => 0x0e, "r" => 0x0f, "y" => 0x10, "t" => 0x11,
        "1" => 0x12, "2" => 0x13, "3" => 0x14, "4" => 0x15, "6" => 0x16, "5" => 0x17,
        "=" => 0x18, "9" => 0x19, "7" => 0x1a, "-" => 0x1b, "8" => 0x1c, "0" => 0x1d,
        "]" => 0x1e, "o" => 0x1f, "u" => 0x20, "[" => 0x21, "i" => 0x22, "p" => 0x23,
        "l" => 0x25, "j" => 0x26, "'" => 0x27, "k" => 0x28, ";" => 0x29, "\\" => 0x2a,
        "," => 0x2b, "/" => 0x2c, "n" => 0x2d, "m" => 0x2e, "." => 0x2f, "`" => 0x32,
        "return" | "enter" => 0x24, "tab" => 0x30, "space" => 0x31, "delete" | "backspace" => 0x33,
        "escape" | "esc" => 0x35,
        "f1" => 0x7a, "f2" => 0x78, "f3" => 0x63, "f4" => 0x76, "f5" => 0x60, "f6" => 0x61,
        "f7" => 0x62, "f8" => 0x64, "f9" => 0x65, "f10" => 0x6d, "f11" => 0x67, "f12" => 0x6f,
        "home" => 0x73, "pageup" => 0x74, "forwarddelete" => 0x75, "end" => 0x77, "pagedown" => 0x79,
        "left" => 0x7b, "right" => 0x7c, "down" => 0x7d, "up" => 0x7e,
        _ => return None,
    })
}
