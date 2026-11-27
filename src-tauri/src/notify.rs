//! The "updates are ready" notification, sent to the desktop's notification
//! server over D-Bus with zbus, async all the way.
//!
//! Not tauri-plugin-notification. It hands the call to notify-rust, which
//! blocks on zbus (`zbus::block_on`) from a task of the async runtime. Since
//! Open and Save went through the portal (0.25.1) zbus is built with its
//! tokio feature (tauri-plugin-dialog → rfd → ashpd), and there that call is
//! "Cannot start a runtime from within a runtime": a panic, and with
//! panic = "abort" Komble was gone. It happened five minutes after every
//! start that found updates (0.25.1 – 0.26.0).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use zbus::zvariant::Value;

/// The notification we sent last, so a new count replaces it instead of
/// stacking under it.
static LAST: AtomicU32 = AtomicU32::new(0);

/// "N updates are ready." Errors (no notification server, no session bus) are
/// the caller's to drop: a missed note must never take the app down.
pub async fn updates_ready(count: usize) -> zbus::Result<()> {
    let bus = zbus::Connection::session().await?;
    // the desktop entry names the app on the card (its icon and "Komble")
    let hints: HashMap<&str, Value<'_>> = HashMap::from([("desktop-entry", Value::from("komble"))]);
    let actions: [&str; 0] = [];
    let reply = bus
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Komble",
                LAST.load(Ordering::Relaxed),
                "komble",
                "Komble — updates available",
                body(count),
                &actions[..],
                hints,
                -1i32,
            ),
        )
        .await?;
    if let Ok(id) = reply.body().deserialize::<u32>() {
        LAST.store(id, Ordering::Relaxed);
    }
    Ok(())
}

fn body(count: usize) -> String {
    if count == 1 {
        "1 update is ready to install.".into()
    } else {
        format!("{count} updates are ready to install.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_counts_in_words() {
        assert_eq!(body(1), "1 update is ready to install.");
        assert_eq!(body(44), "44 updates are ready to install.");
    }

    /// The crash this module replaces: a notification sent from a task of
    /// the async runtime. With no bus to reach, the call must come back as
    /// an error, never as a panic. The address is a dead one on purpose: a
    /// test must not put a note on the desktop of whoever runs it.
    #[tokio::test(flavor = "multi_thread")]
    async fn sending_from_the_runtime_does_not_panic() {
        std::env::set_var(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/komble-test-bus",
        );
        let sent = tokio::spawn(updates_ready(3)).await;
        assert!(
            matches!(sent, Ok(Err(_))),
            "expected an error, got {sent:?}"
        );
    }
}
