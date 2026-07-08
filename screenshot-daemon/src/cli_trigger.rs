//! Client for the `screenshot-daemon trigger <action>` subcommand.
//!
//! Connects to the running daemon's `org.screenshot_daemon.Service1` session
//! D-Bus service and invokes the matching method. Used by compositor-level
//! hotkey bindings (e.g. Sway `bindsym Print exec screenshot-daemon trigger
//! fullscreen`).

use anyhow::Context;
use zbus::proxy;

#[proxy(
    interface = "org.screenshot_daemon.Service1",
    default_service = "org.screenshot_daemon.Service1",
    default_path = "/org/screenshot_daemon/Service"
)]
trait ScreenshotService {
    async fn fullscreen(&self) -> zbus::Result<()>;
    async fn region(&self) -> zbus::Result<()>;
    async fn record(&self) -> zbus::Result<()>;
}

/// Dispatch `action` (`fullscreen` | `region` | `record`) to the running
/// daemon via D-Bus. Exits with a clear error if the daemon is not running
/// or the action name is unknown.
pub async fn run(action: &str) -> anyhow::Result<()> {
    let connection = zbus::Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;
    let proxy = ScreenshotServiceProxy::new(&connection)
        .await
        .context("screenshot-daemon is not running or its D-Bus service is unavailable")?;
    match action {
        "fullscreen" => proxy.fullscreen().await,
        "region" => proxy.region().await,
        "record" => proxy.record().await,
        other => anyhow::bail!(
            "unknown action: {other:?} (expected: fullscreen | region | record)"
        ),
    }
    .context("D-Bus method call failed")?;
    Ok(())
}
