// Minimal HID++ 2.0 over a direct (Bluetooth) connection. Only long reports are used.

use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub type Report = [u8; 20];

const LOGITECH: u16 = 0x046d;
const LONG: u8 = 0x11;
const DIRECT: u8 = 0xff; // device index for Bluetooth / USB-cable connections
const SW_ID: u8 = 0x0b; // marks our responses; device notifications use 0

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

pub struct Device {
    hid: HidDevice,
    pub name: String,
}

impl Device {
    /// Opens the HID++ collection of the first Logitech device whose name contains `name`.
    pub fn open(api: &HidApi, name: Option<&str>) -> Result<Device, String> {
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == LOGITECH
                    && matches!(
                        (d.usage_page(), d.usage()),
                        (0xff43, 0x0202) | (0xff00, 0x0002)
                    )
                    && name.is_none_or(|n| d.product_string().unwrap_or("").contains(n))
            })
            .ok_or("no matching Logitech HID++ device found")?;
        let hid = info.open_device(api).map_err(|e| e.to_string())?;
        let name = info.product_string().unwrap_or("?").to_string();
        Ok(Device { hid, name })
    }

    /// Reads one HID++ long report; Ok(None) on timeout. Errors mean the device went away.
    pub fn read(&self, timeout_ms: i32) -> Result<Option<Report>, String> {
        let mut r = [0u8; 20];
        let n = self
            .hid
            .read_timeout(&mut r, timeout_ms)
            .map_err(|e| e.to_string())?;
        if n > 0 && r[0] == LONG {
            debug!("<- {:02x?}", &r[..n]);
        }
        Ok((n > 0 && r[0] == LONG && r[1] == DIRECT).then_some(r))
    }

    /// Sends a request and waits for its response, dropping notifications that arrive meanwhile.
    pub fn call(&self, feature: u8, func: u8, params: &[u8]) -> Result<Report, String> {
        let mut req = [0u8; 20];
        req[..4].copy_from_slice(&[LONG, DIRECT, feature, func << 4 | SW_ID]);
        req[4..4 + params.len()].copy_from_slice(params);
        debug!("-> {req:02x?}");
        self.hid.write(&req).map_err(|e| e.to_string())?;

        let deadline = Instant::now() + Duration::from_secs(2);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(r) = self.read(left.as_millis() as i32)? else {
                continue;
            };
            if r[2] == feature && r[3] == req[3] {
                return Ok(r);
            }
            if r[2] == 0xff && r[3] == feature && r[4] == req[3] {
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
