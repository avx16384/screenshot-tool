/// Global hotkey listener using evdev (libinput).
///
/// Reads keyboard devices from /dev/input/eventX and watches for
/// the configured key combination. Works on both X11 and Wayland.
use std::os::fd::AsRawFd;

use evdev::{AttributeSet, Device, InputEventKind, Key};

#[derive(Debug, Clone)]
pub struct Hotkey {
    pub modifiers: Vec<Key>,
    pub key: Key,
    pub label: String,
}

impl Hotkey {
    /// Parse a hotkey string like "Ctrl+Shift+S" or "Print" or "Alt+P".
    pub fn parse(label: &str, s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            anyhow::bail!("empty hotkey");
        }

        let mut modifiers = Vec::new();
        let key_str = parts.last().unwrap();

        for &mod_str in &parts[..parts.len() - 1] {
            modifiers.push(parse_modifier(mod_str)?);
        }

        let key = parse_key(key_str)?;
        Ok(Hotkey {
            modifiers,
            key,
            label: label.to_string(),
        })
    }
}

fn parse_modifier(s: &str) -> anyhow::Result<Key> {
    match s.to_lowercase().as_str() {
        "ctrl" | "control" => Ok(Key::KEY_LEFTCTRL),
        "shift" => Ok(Key::KEY_LEFTSHIFT),
        "alt" => Ok(Key::KEY_LEFTALT),
        "super" | "meta" | "win" => Ok(Key::KEY_LEFTMETA),
        "rightctrl" => Ok(Key::KEY_RIGHTCTRL),
        "rightshift" => Ok(Key::KEY_RIGHTSHIFT),
        "rightalt" => Ok(Key::KEY_RIGHTALT),
        "rightmeta" => Ok(Key::KEY_RIGHTMETA),
        _ => anyhow::bail!("unknown modifier: {}", s),
    }
}

fn parse_key(s: &str) -> anyhow::Result<Key> {
    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            // evdev scancodes follow QWERTY keyboard layout, not alphabetical
            let key = match c.to_ascii_uppercase() {
                'Q' => Key::KEY_Q,
                'W' => Key::KEY_W,
                'E' => Key::KEY_E,
                'R' => Key::KEY_R,
                'T' => Key::KEY_T,
                'Y' => Key::KEY_Y,
                'U' => Key::KEY_U,
                'I' => Key::KEY_I,
                'O' => Key::KEY_O,
                'P' => Key::KEY_P,
                'A' => Key::KEY_A,
                'S' => Key::KEY_S,
                'D' => Key::KEY_D,
                'F' => Key::KEY_F,
                'G' => Key::KEY_G,
                'H' => Key::KEY_H,
                'J' => Key::KEY_J,
                'K' => Key::KEY_K,
                'L' => Key::KEY_L,
                'Z' => Key::KEY_Z,
                'X' => Key::KEY_X,
                'C' => Key::KEY_C,
                'V' => Key::KEY_V,
                'B' => Key::KEY_B,
                'N' => Key::KEY_N,
                'M' => Key::KEY_M,
                _ => anyhow::bail!("unknown key: {}", s),
            };
            return Ok(key);
        }
        if c.is_ascii_digit() {
            let code = (c as u16 - b'0' as u16) + KEY_0;
            return Ok(Key::new(code));
        }
    }

    match s.to_lowercase().as_str() {
        "print" | "sysrq" | "prtsc" => Ok(Key::KEY_SYSRQ),
        "print2" => Ok(Key::KEY_PRINT),
        "pause" | "break" => Ok(Key::KEY_PAUSE),
        "esc" | "escape" => Ok(Key::KEY_ESC),
        "tab" => Ok(Key::KEY_TAB),
        "enter" | "return" => Ok(Key::KEY_ENTER),
        "space" => Ok(Key::KEY_SPACE),
        "backspace" => Ok(Key::KEY_BACKSPACE),
        "insert" => Ok(Key::KEY_INSERT),
        "delete" => Ok(Key::KEY_DELETE),
        "home" => Ok(Key::KEY_HOME),
        "end" => Ok(Key::KEY_END),
        "pageup" | "page_up" => Ok(Key::KEY_PAGEUP),
        "pagedown" | "page_down" => Ok(Key::KEY_PAGEDOWN),
        "f1" => Ok(Key::KEY_F1),
        "f2" => Ok(Key::KEY_F2),
        "f3" => Ok(Key::KEY_F3),
        "f4" => Ok(Key::KEY_F4),
        "f5" => Ok(Key::KEY_F5),
        "f6" => Ok(Key::KEY_F6),
        "f7" => Ok(Key::KEY_F7),
        "f8" => Ok(Key::KEY_F8),
        "f9" => Ok(Key::KEY_F9),
        "f10" => Ok(Key::KEY_F10),
        "f11" => Ok(Key::KEY_F11),
        "f12" => Ok(Key::KEY_F12),
        "up" => Ok(Key::KEY_UP),
        "down" => Ok(Key::KEY_DOWN),
        "left" => Ok(Key::KEY_LEFT),
        "right" => Ok(Key::KEY_RIGHT),
        _ => anyhow::bail!("unknown key: {}", s),
    }
}

const KEY_0: u16 = 11;
#[allow(dead_code)]
const KEY_A: u16 = 30;
const KEY_PRINT: u16 = 210;

fn find_keyboard_devices() -> Vec<Device> {
    let mut devices = Vec::new();
    for (path, dev) in evdev::enumerate() {
        if let Some(keys) = dev.supported_keys() {
            if keys.contains(Key::KEY_A) || keys.contains(Key::KEY_SPACE) {
                log::info!(
                    "found keyboard: {} ({})",
                    path.display(),
                    dev.name().unwrap_or("unnamed")
                );
                devices.push(dev);
            }
        }
    }
    devices
}

fn key_matches(target: &Key, pressed: Key) -> bool {
    if *target == pressed {
        return true;
    }
    let sysrq = Key::KEY_SYSRQ;
    let print = Key::new(KEY_PRINT);
    if (*target == sysrq && pressed == print) || (*target == print && pressed == sysrq) {
        return true;
    }
    false
}

/// Which hotkey was triggered
#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyAction {
    Fullscreen,
    Region,
    Record,
}

/// Listen for multiple hotkeys. Sends action via watch channel.
/// Blocks the calling thread (run in a std::thread).
pub fn listen(
    hotkeys: &[Hotkey],
    tx: &tokio::sync::mpsc::Sender<HotkeyAction>,
) -> anyhow::Result<()> {
    let mut keyboards = find_keyboard_devices();
    if keyboards.is_empty() {
        anyhow::bail!("no keyboard devices found in /dev/input/ — are you in the 'input' group?");
    }

    let mut pressed: AttributeSet<Key> = AttributeSet::new();
    // Debounce: track which keys have already triggered (prevent multi-device + repeat)
    let mut triggered_keys: AttributeSet<Key> = AttributeSet::new();

    let mut poll_fds: Vec<libc::pollfd> = keyboards
        .iter()
        .map(|dev| libc::pollfd {
            fd: dev.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();

    loop {
        for pfd in &mut poll_fds {
            pfd.revents = 0;
        }

        let ret = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as _, -1) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            anyhow::bail!("poll error: {err}");
        }

        // Collect all events from all devices, then process
        let mut pending_actions: Vec<HotkeyAction> = Vec::new();

        for dev in &mut keyboards {
            if let Ok(events) = dev.fetch_events() {
                for ev in events {
                    let kind = ev.kind();
                    let key = match kind {
                        InputEventKind::Key(k) => k,
                        _ => continue,
                    };
                    let value = ev.value();

                    match value {
                        1 | 2 => pressed.insert(key),
                        0 => {
                            pressed.remove(key);
                            triggered_keys.remove(key);
                        }
                        _ => continue,
                    }

                    // Check on first key-down only (not repeat, not already triggered)
                    if value == 1 && !triggered_keys.contains(key) {
                        for hk in hotkeys {
                            if key_matches(&hk.key, key)
                                && hk.modifiers.iter().all(|m| pressed.contains(*m))
                            {
                                log::info!("hotkey triggered: {}", hk.label);
                                triggered_keys.insert(key);
                                let action = match hk.label.as_str() {
                                    "region" => HotkeyAction::Region,
                                    "record" => HotkeyAction::Record,
                                    _ => HotkeyAction::Fullscreen,
                                };
                                pending_actions.push(action);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Send all pending actions (deduplicated by triggered_keys)
        for action in pending_actions {
            let _ = tx.blocking_send(action);
        }
    }
}
