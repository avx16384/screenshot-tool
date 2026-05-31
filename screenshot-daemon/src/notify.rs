use futures_lite::StreamExt;
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

#[proxy(
    interface = "org.freedesktop.portal.OpenURI",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait OpenURI {
    fn open_file(
        &self,
        parent_window: &str,
        fd: zbus::zvariant::OwnedFd,
        options: std::collections::HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<()>;
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
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(6);
            loop {
                match tokio::time::timeout_at(deadline, stream.next()).await {
                    Ok(Some(msg)) => {
                        if let Ok(args) = msg.args() {
                            if args.id == id && args.action_key == "open" {
                                open_file_via_portal(&path_owned).await.ok();
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

async fn open_file_via_portal(path: &str) -> anyhow::Result<()> {
    let connection = Connection::session().await?;
    let proxy = OpenURIProxy::new(&connection).await?;

    let file = std::fs::File::open(path)?;
    let owned_fd: std::os::fd::OwnedFd = file.into();
    let fd = zbus::zvariant::OwnedFd::from(owned_fd);

    proxy
        .open_file("", fd, std::collections::HashMap::new())
        .await?;

    Ok(())
}
