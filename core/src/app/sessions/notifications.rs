//! Which devices want a notification when a session's agent finishes (each
//! device's bell, kept in `session_notifications.json` by session name), the
//! notifications waiting for each device to collect them, and each phone's
//! Apple push token (`apple_push_tokens.json`), to reach it while suspended.
//! See decision 29 in `docs/decisions.md`.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::app::sessions::private_file::{read_json_or_default, write_private_json};

/// The most notifications kept for a device that isn't collecting them.
const MAX_WAITING_PER_DEVICE: usize = 20;

/// Session name → the ids of the devices whose bell is on for it.
type Bells = BTreeMap<String, BTreeSet<String>>;
/// Device id → its Apple push token.
type ApplePushTokens = BTreeMap<String, String>;

/// One agent finishing, for one device to show.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SessionNotification {
    pub session_name: String,
    pub title: String,
    pub body: String,
}

/// The bells, and the notifications not yet collected.
pub struct SessionNotifications {
    path: PathBuf,
    bells: Mutex<Bells>,
    apple_push_tokens_path: PathBuf,
    apple_push_tokens: Mutex<ApplePushTokens>,
    waiting: Mutex<HashMap<String, VecDeque<SessionNotification>>>,
    added: Notify,
}

impl SessionNotifications {
    /// The bells and push tokens in their files in `data_directory`, or
    /// none when there are no files.
    pub fn open(data_directory: &Path) -> Result<Self, String> {
        let path = data_directory.join("session_notifications.json");
        let apple_push_tokens_path = data_directory.join("apple_push_tokens.json");
        Ok(Self {
            bells: Mutex::new(read_json_or_default(&path)?),
            path,
            apple_push_tokens: Mutex::new(read_json_or_default(&apple_push_tokens_path)?),
            apple_push_tokens_path,
            waiting: Mutex::new(HashMap::new()),
            added: Notify::new(),
        })
    }

    /// No bells and no push tokens yet, kept from now on in their files in
    /// `data_directory`.
    pub fn empty(data_directory: &Path) -> Self {
        Self {
            path: data_directory.join("session_notifications.json"),
            bells: Mutex::new(Bells::default()),
            apple_push_tokens_path: data_directory.join("apple_push_tokens.json"),
            apple_push_tokens: Mutex::new(ApplePushTokens::default()),
            waiting: Mutex::new(HashMap::new()),
            added: Notify::new(),
        }
    }

    /// Whether `device_id`'s bell is on for `session_name`.
    pub fn is_enabled(&self, session_name: &str, device_id: &str) -> bool {
        lock(&self.bells)
            .get(session_name)
            .is_some_and(|devices| devices.contains(device_id))
    }

    /// Turns `device_id`'s bell for `session_name` on or off.
    pub fn set_enabled(
        &self,
        session_name: &str,
        device_id: &str,
        enabled: bool,
    ) -> Result<(), String> {
        self.change_bells(|bells| {
            if enabled {
                bells
                    .entry(session_name.to_string())
                    .or_default()
                    .insert(device_id.to_string());
            } else if let Some(devices) = bells.get_mut(session_name) {
                devices.remove(device_id);
                if devices.is_empty() {
                    bells.remove(session_name);
                }
            }
        })
    }

    /// The ids of the devices whose bell is on for `session_name`.
    pub fn list_device_ids_for(&self, session_name: &str) -> Vec<String> {
        lock(&self.bells)
            .get(session_name)
            .map(|devices| devices.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// The sessions with at least one bell on.
    pub fn list_watched_session_names(&self) -> Vec<String> {
        lock(&self.bells).keys().cloned().collect()
    }

    /// Turns off every bell of these sessions, for sessions that are gone.
    pub fn remove_sessions(&self, session_names: &[String]) -> Result<(), String> {
        self.change_bells(|bells| {
            for name in session_names {
                bells.remove(name);
            }
        })
    }

    /// Turns off every bell of a device, drops what it hadn't collected and
    /// forgets its push token.
    pub fn remove_device(&self, device_id: &str) -> Result<(), String> {
        lock(&self.waiting).remove(device_id);
        self.change_apple_push_tokens(|tokens| {
            tokens.remove(device_id);
        })?;
        self.change_bells(|bells| {
            for devices in bells.values_mut() {
                devices.remove(device_id);
            }
            bells.retain(|_, devices| !devices.is_empty());
        })
    }

    /// Keeps `token` as the Apple push token of `device_id`.
    pub fn set_apple_push_token(&self, device_id: &str, token: &str) -> Result<(), String> {
        self.change_apple_push_tokens(|tokens| {
            tokens.insert(device_id.to_string(), token.to_string());
        })
    }

    /// Forgets `device_id`'s push token if it's still `token`, for one
    /// Apple says is no longer valid.
    pub fn remove_apple_push_token(&self, device_id: &str, token: &str) -> Result<(), String> {
        self.change_apple_push_tokens(|tokens| {
            if tokens.get(device_id).is_some_and(|kept| kept == token) {
                tokens.remove(device_id);
            }
        })
    }

    /// The device ids and push tokens of the phones whose bell is on for
    /// `session_name`.
    pub fn list_apple_push_tokens_for(&self, session_name: &str) -> Vec<(String, String)> {
        let device_ids = lock(&self.bells)
            .get(session_name)
            .cloned()
            .unwrap_or_default();
        let tokens = lock(&self.apple_push_tokens);
        device_ids
            .into_iter()
            .filter_map(|device_id| {
                let token = tokens.get(&device_id)?.clone();
                Some((device_id, token))
            })
            .collect()
    }

    /// Queues `notification` for each device whose bell is on for its session.
    pub fn add(&self, notification: SessionNotification) {
        let device_ids = lock(&self.bells)
            .get(&notification.session_name)
            .cloned()
            .unwrap_or_default();
        let mut waiting = lock(&self.waiting);
        for device_id in device_ids {
            let queue = waiting.entry(device_id).or_default();
            if queue.len() == MAX_WAITING_PER_DEVICE {
                queue.pop_front();
            }
            queue.push_back(notification.clone());
        }
        drop(waiting);
        self.added.notify_waiters();
    }

    /// The notifications waiting for `device_id`, else the ones added within
    /// `timeout`, else none.
    pub async fn wait_for(&self, device_id: &str, timeout: Duration) -> Vec<SessionNotification> {
        let added = self.added.notified();
        tokio::pin!(added);
        added.as_mut().enable();
        let taken = self.take_waiting(device_id);
        if !taken.is_empty() {
            return taken;
        }
        let _ = tokio::time::timeout(timeout, added).await;
        self.take_waiting(device_id)
    }

    fn take_waiting(&self, device_id: &str) -> Vec<SessionNotification> {
        lock(&self.waiting)
            .remove(device_id)
            .map(Vec::from)
            .unwrap_or_default()
    }

    /// Changes the bells and writes them, keeping the old ones if the write fails.
    fn change_bells(&self, change: impl FnOnce(&mut Bells)) -> Result<(), String> {
        let mut bells = lock(&self.bells);
        let mut changed = bells.clone();
        change(&mut changed);
        if changed != *bells {
            write_private_json(&self.path, &changed)?;
            *bells = changed;
        }
        Ok(())
    }

    /// Changes the push tokens and writes them, keeping the old ones if the write fails.
    fn change_apple_push_tokens(
        &self,
        change: impl FnOnce(&mut ApplePushTokens),
    ) -> Result<(), String> {
        let mut tokens = lock(&self.apple_push_tokens);
        let mut changed = tokens.clone();
        change(&mut changed);
        if changed != *tokens {
            write_private_json(&self.apple_push_tokens_path, &changed)?;
            *tokens = changed;
        }
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while holding the lock leaves the value whole, so a poisoned one is still usable.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_in(directory: &tempfile::TempDir) -> SessionNotifications {
        SessionNotifications::open(directory.path()).unwrap()
    }

    fn finished(session_name: &str) -> SessionNotification {
        SessionNotification {
            session_name: session_name.into(),
            title: session_name.into(),
            body: "Finished".into(),
        }
    }

    #[test]
    fn each_device_has_its_own_bell_and_the_bells_outlive_a_restart() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = open_in(&directory);

        notifications.set_enabled("cr-a", "desktop", true).unwrap();
        notifications.set_enabled("cr-a", "phone", true).unwrap();
        notifications.set_enabled("cr-a", "phone", false).unwrap();

        let reopened = open_in(&directory);
        assert!(reopened.is_enabled("cr-a", "desktop"));
        assert!(!reopened.is_enabled("cr-a", "phone"));
        assert_eq!(reopened.list_watched_session_names(), ["cr-a"]);
    }

    #[test]
    fn removing_a_session_or_a_device_turns_its_bells_off() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = open_in(&directory);
        notifications.set_enabled("cr-a", "desktop", true).unwrap();
        notifications.set_enabled("cr-b", "desktop", true).unwrap();
        notifications.set_enabled("cr-b", "phone", true).unwrap();

        notifications.remove_sessions(&["cr-a".into()]).unwrap();
        notifications.remove_device("desktop").unwrap();

        assert_eq!(notifications.list_watched_session_names(), ["cr-b"]);
        assert!(notifications.is_enabled("cr-b", "phone"));
        assert!(!notifications.is_enabled("cr-b", "desktop"));
    }

    #[tokio::test]
    async fn a_notification_goes_only_to_the_devices_whose_bell_is_on() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = open_in(&directory);
        notifications.set_enabled("cr-a", "desktop", true).unwrap();

        notifications.add(finished("cr-a"));
        notifications.add(finished("cr-b"));

        assert_eq!(
            notifications.wait_for("desktop", Duration::ZERO).await,
            [finished("cr-a")]
        );
        assert!(
            notifications
                .wait_for("phone", Duration::ZERO)
                .await
                .is_empty()
        );
        assert!(
            notifications
                .wait_for("desktop", Duration::ZERO)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_waiting_device_gets_a_notification_as_soon_as_it_is_added() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = std::sync::Arc::new(open_in(&directory));
        notifications.set_enabled("cr-a", "desktop", true).unwrap();

        let waiter = {
            let notifications = std::sync::Arc::clone(&notifications);
            tokio::spawn(async move {
                notifications
                    .wait_for("desktop", Duration::from_secs(10))
                    .await
            })
        };
        tokio::task::yield_now().await;
        notifications.add(finished("cr-a"));

        assert_eq!(waiter.await.unwrap(), [finished("cr-a")]);
    }

    #[test]
    fn only_the_newest_notifications_are_kept_for_a_device_not_collecting_them() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = open_in(&directory);
        notifications.set_enabled("cr-a", "phone", true).unwrap();

        for _ in 0..MAX_WAITING_PER_DEVICE + 5 {
            notifications.add(finished("cr-a"));
        }

        assert_eq!(
            notifications.take_waiting("phone").len(),
            MAX_WAITING_PER_DEVICE
        );
    }

    #[test]
    fn a_push_token_reaches_its_phone_while_its_bell_is_on() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = open_in(&directory);
        notifications.set_enabled("cr-a", "phone", true).unwrap();
        notifications.set_enabled("cr-a", "tablet", true).unwrap();
        notifications.set_apple_push_token("phone", "aa11").unwrap();
        notifications.set_apple_push_token("other", "bb22").unwrap();

        let reopened = open_in(&directory);
        assert_eq!(
            reopened.list_apple_push_tokens_for("cr-a"),
            [("phone".to_string(), "aa11".to_string())]
        );
        assert!(reopened.list_apple_push_tokens_for("cr-b").is_empty());
    }

    #[test]
    fn a_push_token_goes_with_its_device_or_when_apple_rejects_it() {
        let directory = tempfile::tempdir().unwrap();
        let notifications = open_in(&directory);
        notifications.set_enabled("cr-a", "phone", true).unwrap();
        notifications.set_enabled("cr-a", "tablet", true).unwrap();
        notifications.set_apple_push_token("phone", "new").unwrap();
        notifications
            .set_apple_push_token("tablet", "cc33")
            .unwrap();

        notifications
            .remove_apple_push_token("phone", "old")
            .unwrap();
        notifications.remove_device("tablet").unwrap();
        assert_eq!(
            notifications.list_apple_push_tokens_for("cr-a"),
            [("phone".to_string(), "new".to_string())]
        );

        notifications
            .remove_apple_push_token("phone", "new")
            .unwrap();
        assert!(notifications.list_apple_push_tokens_for("cr-a").is_empty());
    }
}
