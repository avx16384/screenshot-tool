//! Session D-Bus service exposing screenshot actions so compositor-level
//! hotkeys (e.g. Sway `bindsym`) can trigger captures without raw evdev,
//! which is starved by Sway's exclusive EVIOCGRAB on keyboards.

use crate::hotkey::HotkeyAction;
use zbus::fdo;

/// D-Bus object that dispatches capture actions into the daemon's main loop.
///
/// Each method hands a `HotkeyAction` to the same `mpsc::Sender` that the
/// tray menu and the evdev listener use, so all trigger paths converge on
/// the single main loop in [`crate::main`].
pub struct ScreenshotService {
    tx: tokio::sync::mpsc::Sender<HotkeyAction>,
}

impl ScreenshotService {
    pub fn new(tx: tokio::sync::mpsc::Sender<HotkeyAction>) -> Self {
        Self { tx }
    }

    /// Non-blocking dispatch. Returns `fdo::Error::Failed` if the action
    /// channel is full (capacity 4) — the correct backpressure signal to
    /// the D-Bus caller, who can retry.
    fn dispatch(&self, action: HotkeyAction) -> fdo::Result<()> {
        self.tx
            .try_send(action)
            .map_err(|e| fdo::Error::Failed(format!("dispatch failed: {e}")))
    }
}

#[zbus::interface(name = "org.screenshot_daemon.Service1")]
impl ScreenshotService {
    /// Capture the full screen.
    async fn fullscreen(&self) -> fdo::Result<()> {
        self.dispatch(HotkeyAction::Fullscreen)
    }

    /// Capture a user-selected region.
    async fn region(&self) -> fdo::Result<()> {
        self.dispatch(HotkeyAction::Region)
    }

    /// Start or stop screen recording (toggle).
    async fn record(&self) -> fdo::Result<()> {
        self.dispatch(HotkeyAction::Record)
    }
}

/// Register the service on the session bus and claim its well-known name.
///
/// The returned `zbus::Connection` MUST be held alive by the caller for as
/// long as the service should remain reachable — dropping it unregisters
/// the object and releases the name.
pub async fn register(
    tx: tokio::sync::mpsc::Sender<HotkeyAction>,
) -> anyhow::Result<zbus::Connection> {
    let connection = zbus::Connection::session().await?;
    connection
        .object_server()
        .at("/org/screenshot_daemon/Service", ScreenshotService::new(tx))
        .await?;
    connection
        .request_name("org.screenshot_daemon.Service1")
        .await?;
    Ok(connection)
}
