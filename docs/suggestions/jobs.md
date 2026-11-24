# Jobs: what was left out

Built from `docs/design/jobs.md`. Left for later, on purpose:

- **Jobs on a device.** None yet. The first one is called by the Tauri
  app beside the sync: at startup, on the foreground, and in the sync's
  loop, for the active account.
- **Jobs on a phone in the background.** iOS's `BGTaskScheduler` and
  Android's `WorkManager` wake an app now and then, when the system
  chooses, for about 30 seconds on iOS. Through a mobile plugin, Swift
  and Kotlin, or `tauri-plugin-background-service`, they would call
  `run_due` like any other wake-up. Not worth it while a device's work
  can wait for the app to open.
- **Jobs for every account on a device**, not only the active one.
- **Retrying a failed job sooner** than its interval.
- **Seeing the jobs**: `app_web jobs list`, or a page, from `app_jobs`.
