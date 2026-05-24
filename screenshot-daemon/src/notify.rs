use futures_lite::StreamExt;
/// D-Bus notification via org.freedesktop.Notifications.
/// Supports click-to-open the saved screenshot.
use zbus::{proxy, Connection};

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: &str);
}

pub async fn send_notification(summary: &str, body: &str) -> anyhow::Result<u32> {
    send_notification_with_open(summary, body, None).await
}

pub async fn send_notification_with_open(
    summary: &str,
    body: &str,
    open_path: Option<&str>,
) -> anyhow::Result<u32> {
    let connection = Connection::session().await?;
    let proxy = NotificationsProxy::new(&connection).await?;

    let actions: Vec<&str> = if open_path.is_some() {
        vec!["open", "Open"]
    } else {
        vec![]
    };

    let id = proxy
        .notify(
            "screenshot-daemon",
            0,
            "camera-photo",
            summary,
            body,
            &actions,
            std::collections::HashMap::new(),
            5000,
        )
        .await?;

    // If we have a path to open, listen for ActionInvoked signal
    if let Some(path) = open_path {
        let path_owned = path.to_string();
        tokio::spawn(async move {
            let proxy = match NotificationsProxy::new(&connection).await {
                Ok(p) => p,
                Err(_) => return,
            };
            let mut stream = match proxy.receive_action_invoked().await {
                Ok(s) => s,
                Err(_) => return,
            };
            // Wait for our notification's action (with timeout)
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(6);
            loop {
                match tokio::time::timeout_at(deadline, stream.next()).await {
                    Ok(Some(msg)) => {
                        if let Ok(args) = msg.args() {
                            if args.id == id && args.action_key == "open" {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(&path_owned)
                                    .spawn();
                                return;
                            }
                        }
                    }
                    _ => return,
                }
            }
        });
    }

    Ok(id)
}
