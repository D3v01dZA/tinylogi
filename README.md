# tinylogi

A tiny alternative to Logitech Options+ for macOS: one binary, one TOML file, no UI.

tinylogi talks Logitech's HID++ 2.0 protocol directly to your mouse. It sets DPI, SmartShift,
the scroll wheel and the thumb wheel, and remaps buttons (including hold-and-swipe gestures) to
keyboard shortcuts. It runs in the foreground, reapplies settings when the device reconnects, and
rereads its config on `tinylogi --reload`.
Instead of a per-model device database, it asks each device at runtime which features it
supports, which keeps the whole thing to a few hundred lines of Rust.

Tested with the MX Master 4 over Bluetooth and through a Bolt receiver. Unifying receivers use the
same protocol but haven't been tested.

## Install

The cask puts `Tinylogi.app` in `/Applications` and starts it as a background job at login:

```sh
brew install --cask D3v01dZA/tap/tinylogi
```

tinylogi is signed with a personal certificate rather than an Apple Developer ID. The first
install on each Mac asks for your password to trust it for code signing (SHA-1
`46:09:5E:CF:A5:9F:B4:43:E0:66:93:E5:A4:5C:3C:BA:5A:EF:95:13`); without that, macOS refuses to
launch it. To trust it by hand instead:

```sh
curl -fsSLO https://raw.githubusercontent.com/D3v01dZA/tinylogi/master/macos/tinylogi-cert.pem
security add-trusted-cert -r trustRoot -p codeSign \
  -k ~/Library/Keychains/login.keychain-db tinylogi-cert.pem
```

Finally:

1. Create `~/.config/tinylogi.toml`, starting from [`tinylogi.example.toml`](tinylogi.example.toml).
2. Allow **Tinylogi** in System Settings → Privacy & Security under **Input Monitoring** and
   **Accessibility** (and **Device Control & Data Access**, if your macOS has it). It picks up
   the permissions within 30 seconds.

The log is at `/tmp/tinylogi.log`.

## Configuration

```toml
[[device]]
name = "MX Master 4"
dpi = 1200

[device.wheel]
hires = true
speed = 1.0               # tinylogi drives smooth scrolling

[device.buttons]
back = "cmd+["
thumb = { tap = "ctrl+up", left = "ctrl+left", right = "ctrl+right" }
```

Every setting is optional; anything left out stays as the device has it. Use one `[[device]]`
block per device. Run `tinylogi --list` to see each connected device's remappable buttons. The
[example config](tinylogi.example.toml) documents every option.

After editing the config, apply it with `tinylogi --reload`. tinylogi checks the new file first:
if it's valid it hands the device back and restarts in place, otherwise it logs the error and
keeps the current settings. The cask puts `tinylogi` on your PATH. Neither `--reload` nor `--list` needs
permissions for your terminal: `--list` runs itself as the app when the terminal lacks them.

## Development

You need Rust and a code-signing identity named `tinylogi` in your login keychain. A stable
signature is what lets macOS keep tinylogi's permissions across rebuilds.

| Command | Does |
|---|---|
| `make dev` | Build and run the app in the foreground with debug logging, pausing the installed one; Ctrl-C resumes it |
| `make log` | Follow the installed app's log |
| `make app` | Build and sign `target/Tinylogi.app` |
| `make dist` | Zip the app, LaunchAgent and example config for a release |
| `make release` | Tag `v<version>` from `Cargo.toml`, publish the GitHub release and update the cask in `../homebrew-tap` |

To release, bump `version` in `Cargo.toml`, commit, then run `make release`. It needs `gh`
logged in and a clone of [homebrew-tap](https://github.com/D3v01dZA/homebrew-tap) next to this
repo.

The code is in three files: `src/hidpp.rs` (the HID++ transport), `src/keys.rs` (synthetic key and
scroll events) and `src/main.rs` (config, applying settings, the event loop).

## License

MIT
