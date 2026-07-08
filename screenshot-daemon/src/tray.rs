//! System tray icon for screenshot-daemon.
//!
//! Registers a StatusNotifierItem (SNI) on the session D-Bus so the daemon
//! appears in the system tray with a context menu. Menu items dispatch
//! hotkey actions back to the main loop and the "Quit" item triggers an
//! orderly shutdown.

use std::sync::Arc;
use tokio::sync::Notify;

use crate::hotkey::HotkeyAction;

/// Tray state holding the channels back to the main event loop.
///
/// `tx` dispatches menu-triggered actions into the same mpsc channel that
/// hotkey events use. `shutdown` signals the main loop to exit.
pub struct ScreenshotTray {
    tx: tokio::sync::mpsc::Sender<HotkeyAction>,
    shutdown: Arc<Notify>,
}

impl ScreenshotTray {
    pub fn new(
        tx: tokio::sync::mpsc::Sender<HotkeyAction>,
        shutdown: Arc<Notify>,
    ) -> Self {
        Self { tx, shutdown }
    }
}

impl ksni::Tray for ScreenshotTray {
    /// This item is menu-only: tell the host to show the DBusMenu on click
    /// rather than calling `Activate`/`ContextMenu`. swaybar calls
    /// `ContextMenu()` on right-click, which ksni refuses ("Not supported,
    /// please use `menu`"), and `Activate()` on left-click, which we don't
    /// implement — so without this flag neither click shows a menu. With
    /// `ItemIsMenu=true` the host renders the (already correctly served)
    /// DBusMenu at `/MenuBar` on activation instead.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "screenshot-daemon".into()
    }

    fn icon_name(&self) -> String {
        "screenshot-daemon".into()
    }

    fn title(&self) -> String {
        "Screenshot".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "screenshot-daemon".into(),
            icon_pixmap: Vec::new(),
            title: "Screenshot Daemon".into(),
            description: "Print: fullscreen, Ctrl+Alt+A: region, Ctrl+Alt+R: record".into(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Fullscreen Screenshot".into(),
                icon_name: "camera-photo".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(e) = tray.tx.try_send(HotkeyAction::Fullscreen) {
                        log::warn!("tray: failed to dispatch fullscreen action: {e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Region Screenshot".into(),
                icon_name: "edit-select-area".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(e) = tray.tx.try_send(HotkeyAction::Region) {
                        log::warn!("tray: failed to dispatch region action: {e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Start / Stop Recording".into(),
                icon_name: "media-record".into(),
                activate: Box::new(|tray: &mut Self| {
                    if let Err(e) = tray.tx.try_send(HotkeyAction::Record) {
                        log::warn!("tray: failed to dispatch record action: {e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    log::info!("quit requested from tray menu");
                    tray.shutdown.notify_one();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
