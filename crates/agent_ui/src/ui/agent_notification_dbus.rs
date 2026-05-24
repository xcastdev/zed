use anyhow::Result;
use ashpd::desktop::{
    Icon,
    notification::{Notification, NotificationProxy, Priority},
};

pub async fn send(id: &str, title: &str, body: Option<&str>) -> Result<()> {
    let proxy = NotificationProxy::new().await?;
    let mut notification = Notification::new(title)
        .icon(Some(Icon::with_names(["dev.zed.Zed", "zed"])))
        .priority(Some(Priority::Normal));
    if let Some(body) = body {
        notification = notification.body(Some(body));
    }
    proxy.add_notification(id, notification).await?;
    Ok(())
}
