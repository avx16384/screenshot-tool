//! Session D-Bus service exposing screenshot actions so compositor-level
//! hotkeys (e.g. Sway `bindsym`) can trigger captures without raw evdev,
//! which is starved by Sway's exclusive EVIOCGRAB on keyboards.

use std::sync::Arc;

use crate::hotkey::HotkeyAction;
use tokio::sync::Notify;
use zbus::fdo;

/// D-Bus object that dispatches capture actions into the daemon's main loop.
///
/// Each method hands a `HotkeyAction` to the same `mpsc::Sender` that the
/// tray menu and the evdev listener use, so all trigger paths converge on
/// the single main loop in [`crate::main`]. The `quit` method signals the
/// shared `shutdown` `Notify` so the main loop can exit cleanly — this lets
/// `close.sh` stop the daemon via D-Bus without `kill`/`pkill`.
pub struct ScreenshotService {
    tx: tokio::sync::mpsc::Sender<HotkeyAction>,
    shutdown: Arc<Notify>,
}

impl ScreenshotService {
    pub fn new(
        tx: tokio::sync::mpsc::Sender<HotkeyAction>,
        shutdown: Arc<Notify>,
    ) -> Self {
        Self { tx, shutdown }
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

    /// Request an orderly shutdown of the daemon.
    ///
    /// Signals the shared `shutdown` `Notify`; the main loop's
    /// `shutdown.notified()` branch then breaks and the process exits
    /// cleanly. This is the clean stop path used by `close.sh` (via
    /// `gdbus call ... .Quit`) so stopping the daemon needs no `kill`.
    async fn quit(&self) -> fdo::Result<()> {
        log::info!("quit requested via D-Bus");
        self.shutdown.notify_one();
        Ok(())
    }
}

/// Register the service on the session bus and claim its well-known name.
///
/// The returned `zbus::Connection` MUST be held alive by the caller for as
/// long as the service should remain reachable — dropping it unregisters
/// the object and releases the name.
pub async fn register(
    tx: tokio::sync::mpsc::Sender<HotkeyAction>,
    shutdown: Arc<Notify>,
) -> anyhow::Result<zbus::Connection> {
    let connection = zbus::Connection::session().await?;
    connection
        .object_server()
        .at(
            "/org/screenshot_daemon/Service",
            ScreenshotService::new(tx, shutdown),
        )
        .await?;
    connection
        .request_name("org.screenshot_daemon.Service1")
        .await?;
    Ok(connection)
}
