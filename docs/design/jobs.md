# Recurring jobs

Work that has to happen on a schedule, not in answer to a request: the
purge, and the sweeps of the buckets that follow it. The web server,
the desktop app and the phone apps wake up in very different ways, so
the split follows the one for features: core decides, the apps only
call.

## Core decides what is due

`core/src/jobs.rs` runs every job, each a `RecurringJob` with its name in
`app_jobs`, the role it runs on and how often. The template's are listed
there; an app's own, in `core/src/app/jobs.rs`, run after them, in the
order `jobs::list` gives. Today they are the server's:

| Job | Runs | Order |
|---|---|---|
| `purge` | `purge::run` | first, so the bytes of the rows it removes go the same day |
| `trial_users_delete` | `trial_users::delete_expired`, hourly, the others daily | after the purge |
| `avatars_delete_unreferenced` | `avatars::delete_unreferenced` | after the purge |
| `files_delete_unreferenced` | `bytes::delete_unreferenced` | after the purge |

Each is daily. A sweep is skipped on a server with no bucket for it.

- **`jobs::run_due(deployment, now)`** runs, in order, every job of the
  deployment's role that is due, and records each run. One failing does
  not stop the next.
- **`jobs::run(deployment, job, now)`** runs one now, due or not.
- **`app_jobs`**, one row per job, keeps its last run: when it started
  and finished, its report or its error. It is not synced; a device has
  the table too, for jobs of its own one day.

## Apps only wake it up

An app calls `run_due` as often as it can. A call before a job is due
does nothing, and a call after one was missed runs it, so how often and
how evenly an app wakes up does not matter.

- **The web server** calls it at startup and every 15 minutes
  (`axum/src/jobs.rs`). `app_web jobs run` does the same once, for a
  scheduler outside the server, and `app_web jobs run <name>` runs one
  job now.
- **The desktop and phone apps** have no jobs yet. The device's file
  cleanup, `bytes::delete_unreferenced_on_device`, stays at the end of
  each sync, where a deleted file's bytes go as soon as the device
  learns of the delete. The first device job would be called where the
  sync is: at startup, when the app comes to the foreground, and in the
  sync's loop. A phone suspends the app in the background, so there the
  foreground is what wakes it; see `docs/suggestions/jobs.md` for
  running in the background.

## One run at a time

Several servers on one database each call `run_due`. Starting a run is
one conditional write, `INSERT … ON CONFLICT DO UPDATE … WHERE` the last
run started an interval ago or more, and only the server whose write
changed the row runs the job. It works the same on SQLite and
PostgreSQL, where an advisory lock would only work on one.

## What is accepted

- **Eventual, not exact.** A job runs within a tick of being due, and
  its next is due an interval after it started, so it drifts by up to a
  tick a day.
- **A failure waits a day.** A job that fails, or a server that stops
  in the middle of one, is tried again once the interval is up. Every
  job may run again safely: each looks at what the rows are now, and a
  run cut short is finished by the next.
- **No care for what else is running.** A sync landing during a sweep
  is the gap accepted in `docs/design/offline-files.md`.

## Not chosen

- **A cron crate** (`tokio-cron-scheduler`): cron syntax and its own
  storage, for a schedule that is "daily". No SQLite storage.
- **A job queue** (`apalis`): retries and workers, for jobs that need
  none; its PostgreSQL backend is not 1.0.
- **Only an outside cron** calling the command line: the schedule would
  live outside the app, in every deployment's own setup. It still works,
  through `app_web jobs run`, beside the loop.

## Server tasks, for what cannot wait a tick

Added 2026-11-23, for Code Reviewer, whose server watches its tmux
sessions and must notice an agent finishing within seconds. A job runs
at most every `TICK`, fifteen minutes, so a watcher is a task instead:
a `ServerTask` (`core/src/server_tasks.rs`), listed by the app in
`core/src/app/server_tasks.rs`, a seam like `app/jobs.rs`, and empty in
the template.

- **Started with the server**, by `axum/src/server_tasks.rs`, beside the
  jobs' loop; not for a command on the server's command line.
- **Started again when it fails**, ten seconds later, with the error in
  the log; a task that returns `Ok` is done.
- **Stopped with the server.** The server has no graceful shutdown, so a
  task, like a job, is written to be stopped at any point.
- **Not recorded.** Unlike a job, a task has no row in `app_jobs`: it is
  not due, it runs. Each server runs its own, so with several servers on
  one database a task must not assume it is the only one.
- **In core, with no timer.** The trait is core's, so a task is written
  beside the functions it calls; the restarts and their pause are the
  web server's, which has Tokio's timer.

