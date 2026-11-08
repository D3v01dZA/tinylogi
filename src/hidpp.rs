// Minimal HID++ 2.0, either direct (Bluetooth / USB cable) or through a Bolt/Unifying receiver.
// Requests are always long reports; short reports are only read, for receiver messages.

use hidapi::{HidApi, HidDevice};
use std::ffi::CString;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub type Report = [u8; 20];

const LOGITECH: u16 = 0x046d;
const SHORT: u8 = 0x10;
const LONG: u8 = 0x11;
const DIRECT: u8 = 0xff; // device index for Bluetooth / USB-cable connections
const SLOTS: [u8; 6] = [1, 2, 3, 4, 5, 6]; // device indices behind a receiver
const SW_ID: u8 = 0x0b; // marks our responses; device notifications use 0

pub const DEVICE_NAME: u16 = 0x0005;
pub const REPROG_CONTROLS: u16 = 0x1b04;
pub const WIRELESS_STATUS: u16 = 0x1d4b;
pub const ADJUSTABLE_DPI: u16 = 0x2201;
pub const SMART_SHIFT: u16 = 0x2110;
pub const SMART_SHIFT_ENHANCED: u16 = 0x2111;
pub const HIRES_WHEEL: u16 = 0x2121;
pub const THUMB_WHEEL: u16 = 0x2150;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDRequestAccess(request: u32) -> bool;
}

/// Opening a mouse needs Input Monitoring permission; this shows the system prompt the first time.
pub fn request_access() -> bool {
    unsafe { IOHIDRequestAccess(1) } // kIOHIDRequestTypeListenEvent
}

/// Devices currently open as (HID path, device index), so two config entries never drive the
/// same device. Two devices on one receiver share a path but not an index.
static OPEN: Mutex<Vec<(CString, u8)>> = Mutex::new(Vec::new());

struct Claim(CString, u8);

impl Drop for Claim {
    fn drop(&mut self) {
        OPEN.lock()
            .unwrap()
            .retain(|c| (&c.0, c.1) != (&self.0, self.1));
    }
}

pub struct Device {
    hid: HidDevice,
    idx: u8,
    pub name: String,
    _claim: Option<Claim>, // set once the device is handed out; dropping it frees the slot
}

impl Device {
    /// Opens the first free Logitech device whose name contains `name`, looking behind receivers.
    pub fn open(api: &HidApi, name: Option<&str>) -> Result<Device, String> {
        let mut open = OPEN.lock().unwrap();
        let mut err = String::from("no matching Logitech HID++ device found");
        for info in api.device_list().filter(|d| {
            d.vendor_id() == LOGITECH
                && matches!(
                    (d.usage_page(), d.usage()),
                    (0xff43, 0x0202) | (0xff00, 0x0002)
                )
        }) {
            let product = info.product_string().unwrap_or("?");
            let receiver = product.contains("Receiver");
            let free: Vec<u8> = (if receiver { &SLOTS[..] } else { &[DIRECT] })
                .iter()
                .copied()
                .filter(|&i| {
                    !open
                        .iter()
                        .any(|c| c.0.as_c_str() == info.path() && c.1 == i)
                })
                .collect();
            if free.is_empty() {
                continue;
            }
            let hid = match info.open_device(api) {
                Ok(hid) => hid,
                Err(e) => {
                    err = e.to_string();
                    continue;
                }
            };
            let mut dev = Device {
                hid,
                idx: DIRECT,
                name: product.to_string(),
                _claim: None,
            };
            for idx in free {
                dev.idx = idx;
                // Empty or sleeping receiver slots fail here and are skipped.
                if receiver && dev.read_name().map(|n| dev.name = n).is_err() {
                    continue;
                }
                if name.is_none_or(|n| dev.name.contains(n)) {
                    open.push((info.path().to_owned(), idx));
                    dev._claim = Some(Claim(info.path().to_owned(), idx));
                    return Ok(dev);
                }
            }
        }
        Err(err)
    }

    fn read_name(&self) -> Result<String, String> {
        let f = self.feature(DEVICE_NAME)?.ok_or("no name")?;
        let len = self.call(f, 0, &[])?[4] as usize;
        let mut name = vec![];
        while name.len() < len {
            name.extend_from_slice(&self.call(f, 1, &[name.len() as u8])?[4..]);
        }
        name.truncate(len);
        Ok(String::from_utf8_lossy(&name).into_owned())
    }

    /// Reads one HID++ report for this device (short ones zero-padded); Ok(None) on timeout or
    /// for other traffic. Errors mean the device went away.
    pub fn read(&self, timeout_ms: i32) -> Result<Option<Report>, String> {
        let mut r = [0u8; 20];
        let n = self
            .hid
            .read_timeout(&mut r, timeout_ms)
            .map_err(|e| e.to_string())?;
        let hidpp = n > 0 && matches!(r[0], SHORT | LONG);
        if hidpp {
            debug!("<- {:02x?}", &r[..n]);
        }
        Ok((hidpp && r[1] == self.idx).then_some(r))
    }

    /// Sends a request and waits for its response, dropping notifications that arrive meanwhile.
    pub fn call(&self, feature: u8, func: u8, params: &[u8]) -> Result<Report, String> {
        let mut req = [0u8; 20];
        req[..4].copy_from_slice(&[LONG, self.idx, feature, func << 4 | SW_ID]);
        req[4..4 + params.len()].copy_from_slice(params);
        debug!("-> {req:02x?}");
        self.hid.write(&req).map_err(|e| e.to_string())?;

        let deadline = Instant::now() + Duration::from_secs(1);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(r) = self.read(left.as_millis() as i32)? else {
                continue;
            };
            if r[0] == LONG && r[2] == feature && r[3] == req[3] {
                return Ok(r);
            }
            // HID++ 2.0 errors are long 0xff reports; a receiver answers for an empty or
            // unreachable slot with a short HID++ 1.0 error (0x8f).
            if matches!(r[2], 0xff | 0x8f) && r[3] == feature && r[4] == req[3] {
                return Err(format!("device returned HID++ error {}", r[5]));
            }
        }
        Err("timed out waiting for device".into())
    }

    /// Looks up the runtime index of a feature via the root feature; None if unsupported.
    pub fn feature(&self, id: u16) -> Result<Option<u8>, String> {
        let r = self.call(0, 0, &id.to_be_bytes())?;
        Ok((r[4] != 0).then_some(r[4]))
    }
}

/// True for a receiver's "device connected" message, after which the device needs reconfiguring.
pub fn is_link_up(r: &Report) -> bool {
    r[0] == SHORT && r[2] == 0x41 && r[4] & 0x40 == 0
}
