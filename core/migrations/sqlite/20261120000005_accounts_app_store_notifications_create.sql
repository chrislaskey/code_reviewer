-- The App Store Server Notifications already handled, see
-- src/accounts/app_store_notifications.rs: one row per notification, by
-- Apple's notificationUUID, so a repeat is skipped. Server only.
CREATE TABLE accounts_app_store_notifications (
  id TEXT PRIMARY KEY NOT NULL,
  type TEXT NOT NULL,
  handled_at TEXT NOT NULL
);
