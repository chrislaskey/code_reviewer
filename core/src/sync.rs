//! Sync: how a device's database and the web server's converge, see
//! `docs/design/offline-sync.md`. The server answers a push of events, a
//! pull of the rows a user sees since a cursor, and a backfill of
//! everything in one community, `push`, `pull` and `backfill`, which the
//! web server serves under `/api/sync` for the user its bearer token
//! stands for. A device does one sync with `run`: it pushes its pending
//! events through a `Transport` with its token, applies the answers,
//! pulls pages from its cursor, backfills each community it has just
//! joined, and prunes the rows it can no longer see. Each synced table
//! takes part through `synced_table::SyncedTable`, listed in
//! `app::synced_tables`. A row with bytes, a file's say, created on the
//! device is held back from the push until its bytes are in the
//! server's bucket, which the sync sends last, see `bytes` and
//! `docs/design/offline-files-decisions.md`. The transport is
//! `services::web_server`: HTTP in the apps, and the server's own
//! functions in this process in tests. A device that chose to work
//! offline, `set_offline`, a device setting, makes no request at all
//! until it chooses otherwise.

pub mod bytes;
pub mod conflicts;
pub mod events;
pub mod pending_changes;
pub mod purge;
pub mod synced_table;
pub mod tables;
#[cfg(any(test, feature = "test-tables"))]
pub mod test_notes;

use crate::data::app_sync_backfills;
use crate::data::app_sync_state;
use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::app::memberships::{self, MembershipError};
use crate::app::users::{self, UserError};
use crate::data::app_events::{self, Event, EventKind, EventStatus, StoredEvent};
use crate::data::app_sync_state::Cursor;
use crate::data::app_users::User;
use crate::data::device_settings::{self, DeviceSettingsError, WORK_OFFLINE};
use crate::database::Transaction;
use crate::deployment::{Deployment, now};
use crate::services::web_server::Transport;
use bytes::BytesError;
use events::{Answer, AnswerStatus};
use purge::PurgeError;
use synced_table::{SyncedTable, TableError};

/// The version of the sync's protocol, which a server describes itself
/// with, see `servers`: a device syncs with a server of its own version
/// alone. Raised by a change a device of the version before could not
/// sync through.
pub const PROTOCOL_VERSION: u32 = 1;

/// The header a device sends `PROTOCOL_VERSION` in with every request,
/// which the server's sync routes require to be its own.
pub const PROTOCOL_VERSION_HEADER: &str = "x-protocol-version";

/// Why the server refuses a sync from a device that sent this protocol
/// version, or none; nothing when it is the server's own.
pub fn protocol_version_refusal(sent: Option<&str>) -> Option<String> {
    let sent = sent.and_then(|sent| sent.trim().parse::<u32>().ok());
    match sent {
        Some(version) if version == PROTOCOL_VERSION => None,
        Some(version) => Some(protocol_version_mismatch(PROTOCOL_VERSION, version)),
        None => Some("This app is too old to sync with this server; update the app".to_string()),
    }
}

/// The words for a server and an app on different protocol versions,
/// naming the one to update.
pub fn protocol_version_mismatch(server: u32, app: u32) -> String {
    let older = if server > app {
        "update the app"
    } else {
        "the server needs updating"
    };
    format!("This server syncs with version {server}, and this app with version {app}; {older}")
}

/// How far back from its cursor's time a pull starts, so a row written
/// in a transaction that committed after a later one is not missed.
pub const OVERLAP_SECONDS: i64 = 60;
pub const DEFAULT_LIMIT: usize = 200;
pub const MAX_LIMIT: usize = 500;

impl Cursor {
    /// The row's place in the pull order, from its wire form.
    pub fn of_row(row: &TableRow) -> Option<Cursor> {
        let last_synced_at = row.row.get("last_synced_at")?.as_str()?;
        let id = row.row.get("id")?.as_str()?;
        Some(Cursor {
            last_synced_at: DateTime::parse_from_rfc3339(last_synced_at)
                .ok()?
                .with_timezone(&Utc),
            id: Uuid::parse_str(id).ok()?,
        })
    }
}

/// A row as it travels on the wire: its table, and the row as JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    pub table: String,
    pub row: Value,
}

impl TableRow {
    fn id(&self) -> Option<Uuid> {
        Uuid::parse_str(self.row.get("id")?.as_str()?).ok()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PushRequest {
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PushResponse {
    pub events: Vec<Answer>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PullRequest {
    /// The device's stored cursor, pulled from with the overlap.
    #[serde(default)]
    pub cursor: Option<Cursor>,
    /// The `next` of the page before, continued exactly.
    #[serde(default)]
    pub after: Option<Cursor>,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Everything in one community, for a device that just joined it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackfillRequest {
    pub community_id: Uuid,
    /// The `next` of the page before.
    #[serde(default)]
    pub after: Option<Cursor>,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// One page of a pull or a backfill.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Page {
    /// In cursor order, each row after the rows it points at.
    pub rows: Vec<TableRow>,
    /// Where the next page starts, when this one was full.
    #[serde(default)]
    pub next: Option<Cursor>,
    /// The place to store once the page is applied: the last row's in
    /// cursor order, not counting the rows brought along for it, or on
    /// the last page the server's time, which the device has everything
    /// up to.
    #[serde(default)]
    pub cursor: Option<Cursor>,
    /// The oldest deletion the server still has, after its last purge.
    #[serde(default)]
    pub purge_horizon: Option<DateTime<Utc>>,
    /// The device's cursor is older than the horizon, so it may have
    /// missed a deletion: drop everything and pull from nothing. The
    /// page is empty.
    #[serde(default)]
    pub resync: bool,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// A synced table's own module failed.
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    Bytes(#[from] BytesError),
    #[error(transparent)]
    Membership(#[from] MembershipError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Purge(#[from] PurgeError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    /// The device sent no token, or one the server does not know.
    #[error("The server did not accept this device's token")]
    Unauthorized,
    /// The token's account was deleted: a 410.
    #[error("This account was deleted")]
    AccountDeleted,
    /// A backfill of a community the user is not in.
    #[error("not a member of community {community_id}")]
    NotAMember { community_id: Uuid },
    /// The server could not be reached, or answered with an error of
    /// its own: the message is for the person, the detail inside for
    /// the log.
    #[error("Could not reach the server")]
    Transport(String),
    /// The server syncs with another protocol version: a 426, with its
    /// words, until one side is updated.
    #[error("{0}")]
    ProtocolVersion(String),
    /// The server refused the request and said why, in words for the
    /// person: a sign-up with a taken email, say.
    #[error("{0}")]
    Refused(String),
    #[error("the server answered with something unexpected: {0}")]
    Malformed(String),
}

/// Answers a push from the user: applies the events in the order given,
/// each in its own transaction, through the resource of its table.
pub async fn push(
    deployment: &Deployment,
    user: &User,
    request: PushRequest,
) -> Result<PushResponse, SyncError> {
    let mut answers = Vec::with_capacity(request.events.len());
    for event in &request.events {
        let answer = match tables::get(&event.table) {
            Some(synced) => synced.apply_event(deployment, user, event).await?,
            None => Answer::rejected(event.id, "unknown_table", None),
        };
        answers.push(answer);
    }
    Ok(PushResponse { events: answers })
}

/// Answers a pull from the user: the rows they see that the server wrote
/// after the cursor, every table merged by its place in the order and
/// cut at the limit, deleted or not, each after the rows it points at.
/// A cursor older than the purge horizon is answered with `resync`
/// instead, since a deletion it never saw may be gone.
pub async fn pull(
    deployment: &Deployment,
    user: &User,
    request: PullRequest,
) -> Result<Page, SyncError> {
    let purge_horizon = purge::horizon(deployment).await?;
    if let (Some(horizon), Some(cursor)) = (purge_horizon, request.cursor)
        && cursor.last_synced_at < horizon
    {
        return Ok(Page {
            purge_horizon,
            resync: true,
            ..Page::default()
        });
    }
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let after = match (request.after, request.cursor) {
        (Some(after), _) => after,
        (None, Some(cursor)) => Cursor {
            last_synced_at: cursor.last_synced_at - Duration::seconds(OVERLAP_SECONDS),
            id: Uuid::nil(),
        },
        (None, None) => Cursor::start(),
    };

    let mut tx = deployment.begin().await?;
    let mut rows: Vec<TableRow> = Vec::new();
    for synced in tables::list() {
        rows.extend(
            synced
                .list_synced_after(&mut tx, user, &after, limit)
                .await?,
        );
    }
    let mut page = page_of(&mut tx, rows, limit).await?;
    tx.commit().await?;
    page.purge_horizon = purge_horizon;
    if page.next.is_none() {
        // The device now has everything up to this moment, whatever the
        // last row's time, so a quiet stretch never makes it look stale
        page.cursor = Some(Cursor {
            last_synced_at: now(),
            id: Uuid::nil(),
        });
    }
    Ok(page)
}

/// Answers a backfill from the user: every post and file in the
/// community, as their live rows there after the cursor, each with the
/// rows it points at. The user must be in the community.
pub async fn backfill(
    deployment: &Deployment,
    user: &User,
    request: BackfillRequest,
) -> Result<Page, SyncError> {
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let after = request.after.unwrap_or_else(Cursor::start);
    let mut tx = deployment.begin().await?;
    if memberships::get_live_in(&mut tx, user.id, request.community_id)
        .await?
        .is_none()
    {
        return Err(SyncError::NotAMember {
            community_id: request.community_id,
        });
    }
    let mut rows: Vec<TableRow> = Vec::new();
    for synced in tables::list() {
        rows.extend(
            synced
                .list_live_in_community_after(&mut tx, request.community_id, &after, limit)
                .await?,
        );
    }
    let page = page_of(&mut tx, rows, limit).await?;
    tx.commit().await?;
    Ok(page)
}

/// A row's place in the pull order.
fn place_of(row: &TableRow) -> Result<Cursor, SyncError> {
    let id = row
        .id()
        .ok_or_else(|| SyncError::Malformed(format!("a row of {} without an id", row.table)))?;
    let last_synced_at =
        Cursor::of_row(row).map_or(DateTime::<Utc>::UNIX_EPOCH, |cursor| cursor.last_synced_at);
    Ok(Cursor { last_synced_at, id })
}

/// The first `limit` rows in cursor order, each after the rows it points
/// at, with where the next page starts when there are more.
async fn page_of(
    tx: &mut Transaction,
    rows: Vec<TableRow>,
    limit: usize,
) -> Result<Page, SyncError> {
    let mut placed = rows
        .into_iter()
        .map(|row| Ok((place_of(&row)?, row)))
        .collect::<Result<Vec<_>, SyncError>>()?;
    placed.sort_by_key(|(place, _)| *place);
    let more = placed.len() > limit;
    placed.truncate(limit);
    let cursor = placed.last().map(|(place, _)| *place);
    let mut out = Vec::with_capacity(placed.len());
    let mut sent = BTreeSet::new();
    for (_, row) in placed {
        send_with_parents(tx, &mut out, &mut sent, row).await?;
    }
    Ok(Page {
        rows: out,
        next: if more { cursor } else { None },
        cursor,
        purge_horizon: None,
        resync: false,
    })
}

/// Puts the row on the wire after the rows it points at, and theirs in
/// turn, so a device with foreign keys can write them in order: a
/// membership after its user and community, a post after its owner, a
/// post's community row after the post, the community and the membership
/// it was added under. Each table says which, see
/// `SyncedTable::parents`. A row brought along for another is sent once.
async fn send_with_parents(
    tx: &mut Transaction,
    out: &mut Vec<TableRow>,
    sent: &mut BTreeSet<(String, Uuid)>,
    row: TableRow,
) -> Result<(), SyncError> {
    let key = (row.table.clone(), place_of(&row)?.id);
    if sent.contains(&key) {
        return Ok(());
    }
    let synced = synced_table_of(&row.table)?;
    for parent in synced.parents(tx, &row).await? {
        Box::pin(send_with_parents(tx, out, sent, parent)).await?;
    }
    sent.insert(key);
    out.push(row);
    Ok(())
}

/// The synced table of the name a row or an event gives.
fn synced_table_of(table: &str) -> Result<&'static dyn SyncedTable, SyncError> {
    tables::get(table)
        .ok_or_else(|| SyncError::Malformed(format!("a row of an unknown table '{table}'")))
}

/// What one sync did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Nobody is signed in on the device: nothing was sent or asked for.
    pub signed_out: bool,
    /// The user is a local user: nothing was sent or asked for, and
    /// nothing ever is, see `local_users`.
    #[serde(default)]
    pub local: bool,
    /// The device is working offline: nothing was sent or asked for.
    pub offline: bool,
    /// Events the server applied.
    pub pushed: usize,
    /// Events that came back as conflicts.
    pub conflicts: usize,
    /// Events the server rejected.
    pub rejected: usize,
    /// Files whose bytes this sync sent to the server's bucket.
    pub uploaded: usize,
    /// Files waiting past the `bytes::PER_SYNC` this sync asked
    /// about, left for the next.
    #[serde(default)]
    pub files_left: usize,
    /// Files' bytes removed from the device: of files deleted or no
    /// longer seen, and leftovers no file points at.
    pub cleaned: usize,
    /// Rows received by the pull.
    pub pulled: usize,
    /// Rows received by the backfills of communities just joined.
    pub backfilled: usize,
    /// Posts and files removed from the device because its user no longer
    /// sees them.
    pub pruned: u64,
    /// The device was away longer than the server keeps deletions, so it
    /// dropped its rows and pulled from nothing; its events were kept.
    pub resynced: bool,
    /// Events still waiting after this sync.
    pub pending: i64,
    /// Why the push failed as a whole, when it did; the pull still ran.
    pub push_error: Option<String>,
}

impl Report {
    /// Whether another sync should start at once: files were left for
    /// it, and this one got somewhere, so a file failing every time
    /// waits for the next sync on its own schedule instead.
    pub fn more_files_to_send(&self) -> bool {
        self.files_left > 0 && (self.uploaded > 0 || self.pushed > 0)
    }
}

/// Where a device stands: whether it works offline, what waits, what
/// conflicted, where the pull is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub signed_in: bool,
    /// The user signed in is a local user, who never syncs, see
    /// `local_users`.
    #[serde(default)]
    pub local: bool,
    pub offline: bool,
    pub pending: i64,
    pub conflicts: i64,
    pub cursor: Option<Cursor>,
}

/// The most events a device sends in one push request.
pub const PUSH_BATCH: usize = 500;
/// The most bytes a push request may be: the web server refuses a
/// larger one whole, so a device never sends one.
pub const MAX_PUSH_BYTES: usize = 5 * 1024 * 1024;
/// The most bytes of events a device puts in one push request, which
/// leaves a fifth of `MAX_PUSH_BYTES` for the JSON around them.
const PUSH_BATCH_BYTES: usize = MAX_PUSH_BYTES / 5 * 4;

/// One sync on a device, as the user signed in and with the device's
/// token: ask the server which waiting files' bytes it has, push the
/// pending events but those of a file it has not, and apply the answers,
/// pull from the cursor, starting over from nothing if the server says
/// so, backfill each community just joined, prune what the user no
/// longer sees, then send the waiting files' bytes and push their
/// events, and remove the bytes no file on the device points at any
/// more. The bytes go last, so nothing else waits on them. A push that
/// fails as a whole changes nothing on the device and the pull still
/// runs. Working offline, it makes no request and only reports what
/// waits.
pub async fn run<T: Transport>(
    deployment: &Deployment,
    transport: &T,
) -> Result<Report, SyncError> {
    let mut report = Report::default();
    let before = status(deployment).await?;
    if !before.signed_in {
        report.signed_out = true;
        return Ok(report);
    }
    if before.local {
        report.local = true;
        return Ok(report);
    }
    if before.offline {
        report.offline = true;
        report.pending = before.pending;
        return Ok(report);
    }
    let (user, token) = users::signed_in_with_token(deployment).await?;
    let token = Some(token.as_str());
    let mut waiting = bytes::ask_about_waiting(deployment, transport, token).await?;
    report.files_left = waiting.len().saturating_sub(bytes::PER_SYNC);
    let held_back = bytes::held_back(deployment, &waiting).await?;
    if let Err(error) =
        push_pending(deployment, transport, token, &user, &held_back, &mut report).await
    {
        report.push_error = Some(error.to_string());
    }
    pull_pages(deployment, transport, token, &user, &mut report).await?;
    backfill_pending(deployment, transport, token, &user, &mut report).await?;
    prune(deployment, &user, &mut report).await?;
    report.uploaded = bytes::upload_waiting(deployment, transport, token, &mut waiting).await?;
    if report.uploaded > 0 {
        let held_back = bytes::held_back(deployment, &waiting).await?;
        if let Err(error) =
            push_pending(deployment, transport, token, &user, &held_back, &mut report).await
        {
            report.push_error = Some(error.to_string());
        }
    }
    report.cleaned += bytes::delete_unreferenced_on_device(deployment, now()).await?;
    report.pending = status(deployment).await?.pending;
    Ok(report)
}

pub async fn status(deployment: &Deployment) -> Result<Status, SyncError> {
    let mut tx = deployment.begin().await?;
    let signed_in: Option<users::SignedIn> =
        device_settings::get(&mut tx, device_settings::SIGNED_IN).await?;
    let status = Status {
        local: signed_in.as_ref().is_some_and(|user| user.token.is_none()),
        signed_in: signed_in.is_some(),
        offline: device_settings::get(&mut tx, WORK_OFFLINE).await?,
        pending: app_events::count(&mut tx, EventStatus::Pending).await?,
        conflicts: app_events::count(&mut tx, EventStatus::Conflict).await?,
        cursor: app_sync_state::get(&mut tx).await?,
    };
    tx.commit().await?;
    Ok(status)
}

/// Chooses to work offline, or not: a device setting, so it holds across
/// restarts. Writes keep queueing either way.
pub async fn set_offline(deployment: &Deployment, offline: bool) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    device_settings::set(&mut tx, WORK_OFFLINE, &offline).await?;
    tx.commit().await?;
    Ok(())
}

/// Pushes the pending events and applies the answers, leaving out the
/// events of the files held back, whose bytes the server does not have,
/// and of their rows in communities:
/// oldest first, `PUSH_BATCH` events or `PUSH_BATCH_BYTES` a request,
/// whichever is fewer, each request's answers applied before the next
/// is sent, so a push cut short has kept what the server already
/// answered.
async fn push_pending<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    user: &User,
    held_back: &BTreeSet<Uuid>,
    report: &mut Report,
) -> Result<(), SyncError> {
    // What waits as the push begins; a change made while it runs waits
    // for the next sync
    let mut waiting: Option<BTreeSet<Uuid>> = None;
    loop {
        // Read again each time: the answers to one batch can remove
        // events that were waiting behind it
        let mut tx = deployment.begin().await?;
        let mut batch = app_events::pending(&mut tx).await?;
        tx.commit().await?;
        batch.retain(|stored| !held_back.contains(&stored.event.row_id));
        let waiting =
            waiting.get_or_insert_with(|| batch.iter().map(|stored| stored.event.id).collect());
        batch.retain(|stored| waiting.contains(&stored.event.id));
        batch.truncate(PUSH_BATCH);
        batch.truncate(count_within_push_batch_bytes(&batch)?);
        if batch.is_empty() {
            return Ok(());
        }
        for stored in &batch {
            waiting.remove(&stored.event.id);
        }
        push_batch(deployment, transport, token, user, &batch, report).await?;
    }
}

/// How many of the events, from the first, one push request holds:
/// those whose JSON is within `PUSH_BATCH_BYTES`, and one at least.
fn count_within_push_batch_bytes(batch: &[StoredEvent]) -> Result<usize, SyncError> {
    let mut bytes = 0;
    for (index, stored) in batch.iter().enumerate() {
        bytes += serde_json::to_vec(&stored.event)
            .map_err(|error| SyncError::Malformed(error.to_string()))?
            .len();
        if bytes > PUSH_BATCH_BYTES && index > 0 {
            return Ok(index);
        }
    }
    Ok(batch.len())
}

/// Sends one request of pending events and applies its answers, in one
/// transaction.
async fn push_batch<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    user: &User,
    pending: &[StoredEvent],
    report: &mut Report,
) -> Result<(), SyncError> {
    let request = PushRequest {
        events: pending.iter().map(|stored| stored.event.clone()).collect(),
    };
    let response = transport.push(token, request).await?;

    let mut tx = deployment.begin().await?;
    // The server's row of each answered row, by the row's place in the
    // write order, applied once every answer is in
    let mut answered: BTreeMap<(usize, Uuid), TableRow> = BTreeMap::new();
    for answer in response.events {
        let Some(stored) = pending.iter().find(|stored| stored.event.id == answer.id) else {
            continue;
        };
        let event = &stored.event;
        let server = answer.server;
        match answer.status {
            AnswerStatus::Applied => {
                app_events::delete(&mut tx, event.id).await?;
                report.pushed += 1;
            }
            AnswerStatus::Conflict => {
                app_events::mark_conflict(&mut tx, event.id, &answer.conflicts).await?;
                report.conflicts += 1;
            }
            AnswerStatus::Rejected => {
                report.rejected += 1;
                let reason = answer.reason.unwrap_or_else(|| "rejected".to_string());
                match (event.kind, server.is_some()) {
                    // The server's row wins
                    (_, true) => app_events::delete(&mut tx, event.id).await?,
                    // A row's bytes may be their only copy, so it is never
                    // dropped: it waits while the server only lacks the
                    // bytes, and is kept for the person otherwise
                    (EventKind::Create, false) if bytes::of_table(&event.table).is_ok() => {
                        let refused = if reason == bytes::NOT_UPLOADED {
                            app_events::count_attempt(&mut tx, event.id, &reason).await?
                        } else {
                            app_events::refuse(&mut tx, event.id, &reason).await?;
                            true
                        };
                        if refused {
                            report.conflicts += 1;
                        }
                        continue;
                    }
                    // The server will never have this row: drop it
                    (EventKind::Create, false) => {
                        app_events::delete(&mut tx, event.id).await?;
                        app_events::delete_pending_of_row(&mut tx, &event.table, event.row_id)
                            .await?;
                        delete_row(&mut tx, &event.table, event.row_id).await?;
                        continue;
                    }
                    // The row is gone from the server, purged or never
                    // there, so the device's copy goes too, and the change
                    // is kept for the person as a conflict with nothing on
                    // the other side, to let go
                    (EventKind::Update, false) if reason.starts_with("unknown_") => {
                        delete_row(&mut tx, &event.table, event.row_id).await?;
                        let conflicts = event
                            .changes
                            .iter()
                            .map(|(field, yours)| {
                                (
                                    field.clone(),
                                    app_events::FieldConflict {
                                        yours: yours.clone(),
                                        server: Value::Null,
                                    },
                                )
                            })
                            .collect();
                        app_events::mark_conflict(&mut tx, event.id, &conflicts).await?;
                        report.conflicts += 1;
                        continue;
                    }
                    // Kept pending with the reason, and tried again by the
                    // next sync, up to the limit
                    (EventKind::Update, false) => {
                        if app_events::count_attempt(&mut tx, event.id, &reason).await? {
                            report.conflicts += 1;
                        }
                        continue;
                    }
                }
            }
        }
        if let Some(server) = server {
            // The server holds the row under another id, such as a
            // membership revived rather than made again: the device's goes
            let server_id = server.id().unwrap_or(event.row_id);
            if server_id != event.row_id {
                app_events::delete_pending_of_row(&mut tx, &event.table, event.row_id).await?;
                delete_row(&mut tx, &event.table, event.row_id).await?;
            }
            answered.insert((tables::position(&server.table), server_id), server);
        }
    }
    for row in answered.into_values() {
        apply_row(&mut tx, user, &row).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn pull_pages<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    user: &User,
    report: &mut Report,
) -> Result<(), SyncError> {
    let mut request = PullRequest {
        cursor: status(deployment).await?.cursor,
        after: None,
        limit: Some(DEFAULT_LIMIT),
    };
    loop {
        let page = transport.pull(token, request.clone()).await?;
        if page.resync {
            reset(deployment, user).await?;
            report.resynced = true;
            request = PullRequest {
                cursor: None,
                after: None,
                limit: Some(DEFAULT_LIMIT),
            };
            continue;
        }
        let mut tx = deployment.begin().await?;
        for row in &page.rows {
            apply_row(&mut tx, user, row).await?;
        }
        report.pulled += page.rows.len();
        if let Some(cursor) = page.cursor {
            app_sync_state::upsert(&mut tx, &cursor).await?;
        }
        tx.commit().await?;
        match page.next {
            Some(next) => {
                request = PullRequest {
                    cursor: None,
                    after: Some(next),
                    limit: Some(DEFAULT_LIMIT),
                }
            }
            None => return Ok(()),
        }
    }
}

/// Drops every synced row, the cursor and the backfills to do, to pull
/// from nothing: what a device does when it was away longer than the
/// server keeps deletions. The events stay, with the rows of the pending
/// ones, which the server's answers replace on the next push; a conflict
/// belongs to the person. The user's own row stays too, since their
/// posts point at it.
async fn reset(deployment: &Deployment, user: &User) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    for synced in tables::list().iter().rev() {
        synced.delete_for_resync(&mut tx, user).await?;
    }
    app_sync_state::maintenance::delete_cursor_and_backfills(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Asks for everything in each community the user has just joined,
/// since the cursor cannot see rows that became visible without
/// changing, and forgets the community once every page is in.
async fn backfill_pending<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    user: &User,
    report: &mut Report,
) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    let communities = app_sync_backfills::list(&mut tx).await?;
    tx.commit().await?;
    for community_id in communities {
        let mut after = None;
        loop {
            let page = transport
                .backfill(
                    token,
                    BackfillRequest {
                        community_id,
                        after,
                        limit: Some(DEFAULT_LIMIT),
                    },
                )
                .await?;
            let mut tx = deployment.begin().await?;
            for row in &page.rows {
                apply_row(&mut tx, user, row).await?;
            }
            report.backfilled += page.rows.len();
            if page.next.is_none() {
                app_sync_backfills::delete(&mut tx, community_id).await?;
            }
            tx.commit().await?;
            match page.next {
                Some(next) => after = Some(next),
                None => break,
            }
        }
    }
    Ok(())
}

/// Removes the rows the user no longer sees, each table its own, see
/// `SyncedTable::prune`: a community left, a post or file taken out of
/// one, the rows brought along for a removal.
async fn prune(deployment: &Deployment, user: &User, report: &mut Report) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    let mut pruned = 0;
    for synced in tables::list() {
        pruned += synced.prune(&mut tx, user).await?;
    }
    tx.commit().await?;
    let (pruned_with_bytes, cleaned) = bytes::prune(deployment, user).await?;
    report.pruned = pruned + pruned_with_bytes;
    report.cleaned += cleaned;
    Ok(())
}

/// Writes a server row over the device's, unless the device has an
/// event for it still pending; true when it was written. A live
/// membership of the device's user that the device did not have, or
/// had left, records the community to backfill.
async fn apply_row(tx: &mut Transaction, user: &User, row: &TableRow) -> Result<bool, SyncError> {
    let synced = synced_table_of(&row.table)?;
    Ok(synced.apply_server_row(tx, user, row).await?)
}

async fn delete_row(tx: &mut Transaction, table: &str, id: Uuid) -> Result<(), SyncError> {
    let synced = synced_table_of(table)?;
    Ok(synced.delete_row(tx, id).await?)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;
    use crate::accounts::{self, Session};
    use crate::app::communities::CommunityError;
    #[cfg(feature = "object-storage")]
    use crate::app::files;
    pub(crate) use crate::app::users::tests::device_of;
    use crate::app::{communities, memberships, posts, posts_communities};
    use crate::data::{app_communities, app_posts, app_users, app_users_communities};
    use crate::database::with_each_database;
    use crate::deployment::{Role, now};
    #[cfg(feature = "sync-http")]
    use crate::services::web_server::Http;
    use crate::services::web_server::InProcess;
    use crate::sync::bytes::{Upload, UploadNew};
    use crate::sync::test_notes::NoteError;
    use crate::sync::{conflicts, purge, test_notes};

    /// A user signed up on the server, with a token for their devices,
    /// and without the community a sign-up starts with, so a test
    /// counts only the rows it makes.
    pub(crate) async fn user_with_token(server: &Deployment, email: &str) -> (User, String) {
        let session = user_with_token_and_first_community(server, email).await;
        let memberships = memberships::list_mine(server, &session.user)
            .await
            .expect("the memberships");
        let mut tx = server.begin().await.expect("a transaction");
        for membership in memberships {
            communities::delete_row(&mut tx, membership.community_id)
                .await
                .expect("removed");
            app_events::delete_of_row(&mut tx, app_communities::TABLE, membership.community_id)
                .await
                .expect("removed");
            app_events::delete_of_row(&mut tx, app_users_communities::TABLE, membership.id)
                .await
                .expect("removed");
        }
        tx.commit().await.expect("committed");
        (session.user, session.token)
    }

    /// A user as a sign-up leaves them, with a token for their devices.
    async fn user_with_token_and_first_community(
        server: &Deployment,
        email: &str,
    ) -> accounts::Session {
        accounts::tests::signed_up_for_a_device(server, email).await
    }

    async fn sync(device: &Deployment, server: &Deployment) -> Report {
        run(device, &InProcess { server }).await.expect("a sync")
    }

    async fn bodies(deployment: &Deployment, user: &User) -> Vec<String> {
        test_notes::list(deployment, user)
            .await
            .expect("the notes")
            .into_iter()
            .map(|note| note.body)
            .collect()
    }

    async fn post_bodies(deployment: &Deployment, user: &User) -> Vec<String> {
        posts::list(deployment, user)
            .await
            .expect("the posts")
            .into_iter()
            .map(|listed| listed.post.body)
            .collect()
    }

    async fn pending(deployment: &Deployment) -> i64 {
        status(deployment).await.expect("the status").pending
    }

    #[tokio::test]
    async fn sync_page_rows_backfill_cursor_and_settings_commit_together() {
        with_each_database(|database| async move {
            let device = Deployment::new(database, Role::Device);
            let user = users::tests::alice();
            let mut tx = device.begin().await.expect("transaction");
            users::upsert_in(&mut tx, &user).await.expect("user");
            tx.commit().await.expect("commit");
            let now = crate::deployment::now();
            let community = app_communities::Community {
                id: Uuid::now_v7(),
                name: "Gardening".to_string(),
                inserted_at: now,
                updated_at: now,
                version: 1,
                last_synced_at: Some(now),
                deleted_at: None,
                deleted_by_id: None,
            };
            let membership = app_users_communities::Membership {
                id: Uuid::now_v7(),
                user_id: user.id,
                community_id: community.id,
                role: app_users_communities::ROLE_MEMBER.to_string(),
                inserted_at: now,
                updated_at: now,
                version: 1,
                last_synced_at: Some(now),
                deleted_at: None,
                deleted_by_id: None,
            };
            let rows = [
                communities::table_row(&community).expect("row"),
                memberships::table_row(&membership).expect("row"),
            ];
            let cursor = Cursor {
                last_synced_at: now,
                id: membership.id,
            };
            for commit in [false, true] {
                let mut tx = device.begin().await.expect("transaction");
                for row in &rows {
                    assert!(apply_row(&mut tx, &user, row).await.expect("apply"));
                }
                app_sync_state::upsert(&mut tx, &cursor)
                    .await
                    .expect("cursor");
                device_settings::set(&mut tx, device_settings::WORK_OFFLINE, &true)
                    .await
                    .expect("setting");
                if commit {
                    tx.commit().await.expect("commit");
                } else {
                    drop(tx);
                }
                let mut tx = device.begin().await.expect("transaction");
                assert_eq!(
                    communities::get_in(&mut tx, community.id)
                        .await
                        .expect("community")
                        .is_some(),
                    commit
                );
                assert_eq!(
                    memberships::get_in(&mut tx, membership.id)
                        .await
                        .expect("membership")
                        .is_some(),
                    commit
                );
                assert_eq!(
                    app_sync_state::get(&mut tx)
                        .await
                        .expect("cursor")
                        .is_some(),
                    commit
                );
                let backfills = app_sync_backfills::list(&mut tx).await.expect("backfills");
                assert_eq!(
                    backfills,
                    if commit {
                        vec![community.id]
                    } else {
                        Vec::new()
                    }
                );
                assert_eq!(
                    device_settings::get_stored(&mut tx, device_settings::WORK_OFFLINE)
                        .await
                        .expect("setting"),
                    commit.then_some(serde_json::json!(true))
                );
                assert!(
                    app_events::pending(&mut tx)
                        .await
                        .expect("no new local events")
                        .is_empty()
                );
                tx.commit().await.expect("commit");
            }
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_note_written_offline_reaches_the_server_and_the_other_device() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let b = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "from a", None)
                .await
                .expect("a note");
            assert_eq!(pending(&a).await, 1);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 1);
            assert_eq!(report.pending, 0);
            let on_a = test_notes::get(&a, &alice, note.id)
                .await
                .expect("the note on a");
            assert_eq!(on_a.version, 1);
            assert!(on_a.last_synced_at.is_some());
            assert_eq!(bodies(&server, &alice).await, vec!["from a"]);

            let report = sync(&b, &server).await;
            assert_eq!(report.pulled, 2, "alice's row and the note");
            assert_eq!(
                test_notes::get(&b, &alice, note.id)
                    .await
                    .expect("the note on b"),
                on_a
            );
            assert_eq!(users::get(&b, alice.id).await.expect("alice on b"), alice);
            assert!(status(&b).await.expect("the status").cursor.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_sign_up_starts_with_a_personal_community_which_its_device_pulls() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let session = user_with_token_and_first_community(&server, "alice@example.com").await;
            let (alice, token) = (session.user, session.token);
            let on_server = communities::list(&server, &alice).await.expect("listed");
            assert_eq!(on_server.len(), 1);
            assert_eq!(
                on_server[0].community.name,
                accounts::SIGN_UP_COMMUNITY_NAME
            );
            assert!(on_server[0].membership.is_some(), "and she is in it");

            let a = device_of(&alice, &token).await;
            sync(&a, &server).await;
            let on_device = communities::list(&a, &alice).await.expect("listed");
            assert_eq!(on_device, on_server);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_pulls_only_what_the_devices_user_sees() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, alice_token) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            test_notes::create(&server, &alice, "alice's", None)
                .await
                .expect("a note");
            let bobs = test_notes::create(&server, &bob, "bob's", None)
                .await
                .expect("a note");

            let b = device_of(&bob, &bob_token).await;
            let report = sync(&b, &server).await;
            assert_eq!(report.pulled, 2, "bob's row and bob's note");
            assert_eq!(bodies(&b, &bob).await, vec!["bob's"]);
            let error = users::get(&b, alice.id).await.expect_err("not on b");
            assert!(matches!(error, UserError::NotFound { .. }));

            let a = device_of(&alice, &alice_token).await;
            sync(&a, &server).await;
            assert_eq!(bodies(&a, &alice).await, vec!["alice's"]);
            let error = test_notes::get(&a, &alice, bobs.id)
                .await
                .expect_err("not on a");
            assert!(matches!(error, NoteError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_without_a_token_the_server_knows_is_refused() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, "not-a-token").await;
            test_notes::create(&a, &alice, "mine", None)
                .await
                .expect("a note");

            let error = run(&a, &InProcess { server: &server })
                .await
                .expect_err("refused");
            assert!(matches!(error, SyncError::Unauthorized));
            assert!(bodies(&server, &alice).await.is_empty());
            assert_eq!(pending(&a).await, 1);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_an_edit_from_the_servers_version_applies_everywhere() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let b = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "first", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            sync(&b, &server).await;

            test_notes::update(&b, &alice, note.id, "edited on b")
                .await
                .expect("edited");
            sync(&b, &server).await;
            sync(&a, &server).await;

            let on_a = test_notes::get(&a, &alice, note.id)
                .await
                .expect("the note on a");
            assert_eq!(on_a.body, "edited on b");
            assert_eq!(on_a.version, 2);
            assert_eq!(bodies(&server, &alice).await, vec!["edited on b"]);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_the_same_field_edited_on_two_devices_keeps_the_first_and_returns_a_conflict() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let b = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "start", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            sync(&b, &server).await;

            test_notes::update(&a, &alice, note.id, "a's edit")
                .await
                .expect("edited on a");
            test_notes::update(&b, &alice, note.id, "b's edit")
                .await
                .expect("edited on b");
            sync(&a, &server).await;
            let report = sync(&b, &server).await;
            assert_eq!(report.conflicts, 1);

            assert_eq!(bodies(&server, &alice).await, vec!["a's edit"]);
            let on_b = test_notes::get(&b, &alice, note.id)
                .await
                .expect("the note on b");
            assert_eq!(on_b.body, "a's edit");
            assert_eq!(on_b.version, 2);
            let conflicts = conflicts::list(&b).await.expect("the conflicts");
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].about.as_deref(), Some("a's edit"));
            assert_eq!(conflicts[0].fields[0].name, "body");
            assert_eq!(conflicts[0].fields[0].yours, json!("b's edit"));
            assert_eq!(conflicts[0].fields[0].now, json!("a's edit"));
            assert_eq!(status(&b).await.expect("the status").conflicts, 1);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_different_fields_edited_on_two_devices_both_apply() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let b = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "start", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            sync(&b, &server).await;

            test_notes::update(&a, &alice, note.id, "a's edit")
                .await
                .expect("edited on a");
            test_notes::delete(&b, &alice, note.id)
                .await
                .expect("deleted on b");
            sync(&a, &server).await;
            let report = sync(&b, &server).await;
            assert_eq!(report.conflicts, 0);
            assert_eq!(report.pushed, 1);

            assert!(bodies(&server, &alice).await.is_empty());
            let mut tx = server.begin().await.expect("a transaction");
            let row = test_notes::get_in(&mut tx, note.id)
                .await
                .expect("the row")
                .expect("kept");
            assert_eq!(row.body, "a's edit");
            assert!(row.deleted_at.is_some());
            assert_eq!(row.deleted_by_id, Some(alice.id));
            assert_eq!(row.version, 3);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_two_edits_made_offline_on_one_device_do_not_conflict_with_each_other() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "one", None)
                .await
                .expect("a note");
            test_notes::update(&a, &alice, note.id, "two")
                .await
                .expect("edited");
            test_notes::update(&a, &alice, note.id, "three")
                .await
                .expect("edited again");
            assert_eq!(pending(&a).await, 3);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 3);
            assert_eq!(report.conflicts, 0);
            assert_eq!(bodies(&server, &alice).await, vec!["three"]);
            assert_eq!(
                test_notes::get(&a, &alice, note.id)
                    .await
                    .expect("the note")
                    .version,
                3
            );
        })
        .await;
    }

    /// A server whose push answers never reach the device the first time.
    struct PushLost<'a> {
        server: InProcess<'a>,
        lost: std::sync::Mutex<bool>,
    }

    impl Transport for PushLost<'_> {
        async fn push(
            &self,
            token: Option<&str>,
            request: PushRequest,
        ) -> Result<PushResponse, SyncError> {
            let response = self.server.push(token, request).await?;
            let mut lost = self.lost.lock().expect("the lock");
            if *lost {
                *lost = false;
                return Err(SyncError::Transport("the connection dropped".to_string()));
            }
            Ok(response)
        }

        async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
            self.server.pull(token, request).await
        }

        async fn backfill(
            &self,
            token: Option<&str>,
            request: BackfillRequest,
        ) -> Result<Page, SyncError> {
            self.server.backfill(token, request).await
        }

        async fn sign_in_with_device_code(
            &self,
            code: &str,
            verifier: &str,
        ) -> Result<Session, SyncError> {
            self.server.sign_in_with_device_code(code, verifier).await
        }

        async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
            self.server.sign_out(token).await
        }

        async fn bytes_uploads_create(
            &self,
            token: Option<&str>,
            table: &str,
            request: UploadNew,
        ) -> Result<Upload, SyncError> {
            self.server
                .bytes_uploads_create(token, table, request)
                .await
        }

        async fn bytes_upload(
            &self,
            token: Option<&str>,
            table: &str,
            upload: &Upload,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.bytes_upload(token, table, upload, path).await
        }

        async fn bytes_download(
            &self,
            token: Option<&str>,
            table: &str,
            id: Uuid,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.bytes_download(token, table, id, path).await
        }
    }

    /// A server that keeps the size of each push it is sent, refuses
    /// one over `MAX_PUSH_BYTES` as the web server does, and whose
    /// connection drops at the request numbered `fails_at`, from 1.
    struct PushCounted<'a> {
        server: InProcess<'a>,
        sizes: std::sync::Mutex<Vec<usize>>,
        fails_at: Option<usize>,
    }

    impl Transport for PushCounted<'_> {
        async fn push(
            &self,
            token: Option<&str>,
            request: PushRequest,
        ) -> Result<PushResponse, SyncError> {
            let number = {
                let mut sizes = self.sizes.lock().expect("the lock");
                sizes.push(request.events.len());
                sizes.len()
            };
            if self.fails_at == Some(number) {
                return Err(SyncError::Transport("the connection dropped".to_string()));
            }
            let bytes = serde_json::to_vec(&request).expect("as JSON").len();
            if bytes > MAX_PUSH_BYTES {
                return Err(SyncError::Refused(format!("a push of {bytes} bytes")));
            }
            self.server.push(token, request).await
        }

        async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
            self.server.pull(token, request).await
        }

        async fn backfill(
            &self,
            token: Option<&str>,
            request: BackfillRequest,
        ) -> Result<Page, SyncError> {
            self.server.backfill(token, request).await
        }

        async fn sign_in_with_device_code(
            &self,
            code: &str,
            verifier: &str,
        ) -> Result<Session, SyncError> {
            self.server.sign_in_with_device_code(code, verifier).await
        }

        async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
            self.server.sign_out(token).await
        }

        async fn bytes_uploads_create(
            &self,
            token: Option<&str>,
            table: &str,
            request: UploadNew,
        ) -> Result<Upload, SyncError> {
            self.server
                .bytes_uploads_create(token, table, request)
                .await
        }

        async fn bytes_upload(
            &self,
            token: Option<&str>,
            table: &str,
            upload: &Upload,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.bytes_upload(token, table, upload, path).await
        }

        async fn bytes_download(
            &self,
            token: Option<&str>,
            table: &str,
            id: Uuid,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.bytes_download(token, table, id, path).await
        }
    }

    #[tokio::test]
    async fn sync_a_push_goes_in_batches_and_one_cut_short_keeps_what_was_answered() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let count = 2 * PUSH_BATCH + 200;
            for number in 0..count {
                test_notes::create(&a, &alice, &format!("note {number}"), None)
                    .await
                    .expect("a note");
            }

            let dropped = PushCounted {
                server: InProcess { server: &server },
                sizes: std::sync::Mutex::new(Vec::new()),
                fails_at: Some(2),
            };
            let report = run(&a, &dropped).await.expect("a sync");
            assert!(report.push_error.is_some());
            assert_eq!(report.pushed, PUSH_BATCH, "the first request's");
            assert_eq!(report.pending as usize, count - PUSH_BATCH);
            assert_eq!(bodies(&server, &alice).await.len(), PUSH_BATCH);

            let counted = PushCounted {
                server: InProcess { server: &server },
                sizes: std::sync::Mutex::new(Vec::new()),
                fails_at: None,
            };
            let report = run(&a, &counted).await.expect("a sync");
            assert_eq!(
                *counted.sizes.lock().expect("the lock"),
                vec![PUSH_BATCH, 200]
            );
            assert_eq!(report.pending, 0);
            assert_eq!(bodies(&server, &alice).await.len(), count);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_push_of_long_notes_goes_in_requests_the_server_takes() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let longest = "é".repeat(test_notes::MAX_BODY_LENGTH);
            for _ in 0..PUSH_BATCH {
                test_notes::create(&a, &alice, &longest, None)
                    .await
                    .expect("a note");
            }

            let counted = PushCounted {
                server: InProcess { server: &server },
                sizes: std::sync::Mutex::new(Vec::new()),
                fails_at: None,
            };
            let report = run(&a, &counted).await.expect("a sync");
            assert_eq!(report.push_error, None);
            let sizes = counted.sizes.lock().expect("the lock").clone();
            assert_eq!(sizes.len(), 3, "twenty kilobytes a note: {sizes:?}");
            assert_eq!(sizes.iter().sum::<usize>(), PUSH_BATCH);
            assert_eq!(report.pending, 0);
            assert_eq!(bodies(&server, &alice).await.len(), PUSH_BATCH);
        })
        .await;
    }

    #[test]
    fn sync_a_push_request_holds_one_event_at_least_whatever_its_size() {
        let huge = Event::create(
            test_notes::TABLE,
            Uuid::now_v7(),
            app_events::Fields::from_iter([(
                "body".to_string(),
                json!("a".repeat(MAX_PUSH_BYTES)),
            )]),
            now(),
        );
        let stored = |event: &Event| StoredEvent {
            event: event.clone(),
            user_id: Uuid::now_v7(),
            status: app_events::EventStatus::Pending,
            conflicts: app_events::Conflicts::new(),
            version: None,
            last_error: None,
            attempts: 0,
        };
        let batch = [stored(&huge), stored(&huge)];
        assert_eq!(count_within_push_batch_bytes(&batch).expect("counted"), 1);
        assert_eq!(count_within_push_batch_bytes(&[]).expect("counted"), 0);
    }

    #[tokio::test]
    async fn sync_a_row_made_offline_keeps_the_time_it_was_made() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _token) = user_with_token(&server, "alice@example.com").await;
            let last_week = now() - Duration::days(7);
            let offline = Event::create(
                test_notes::TABLE,
                Uuid::now_v7(),
                app_events::Fields::from_iter([("body".to_string(), json!("written offline"))]),
                last_week,
            );
            test_notes::apply_event(&server, &alice, &offline)
                .await
                .expect("applied");
            let note = test_notes::get(&server, &alice, offline.row_id)
                .await
                .expect("the note");
            assert_eq!(note.inserted_at, last_week);
            assert!(note.updated_at > last_week, "the server's own clock");

            let before = now();
            let ahead = Event::create(
                test_notes::TABLE,
                Uuid::now_v7(),
                app_events::Fields::from_iter([("body".to_string(), json!("a clock set ahead"))]),
                now() + Duration::days(7),
            );
            test_notes::apply_event(&server, &alice, &ahead)
                .await
                .expect("applied");
            let note = test_notes::get(&server, &alice, ahead.row_id)
                .await
                .expect("the note");
            assert!(note.inserted_at >= before && note.inserted_at <= now());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_retried_event_gets_the_same_answer_and_is_not_applied_twice() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "start", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            test_notes::update(&a, &alice, note.id, "edited")
                .await
                .expect("edited");
            let transport = PushLost {
                server: InProcess { server: &server },
                lost: std::sync::Mutex::new(true),
            };

            let report = run(&a, &transport).await.expect("a sync");
            assert!(report.push_error.is_some());
            assert_eq!(report.pending, 1);
            assert_eq!(
                test_notes::get(&a, &alice, note.id)
                    .await
                    .expect("the note")
                    .body,
                "edited"
            );

            let report = run(&a, &transport).await.expect("a sync");
            assert_eq!(report.pushed, 1);
            assert_eq!(report.pending, 0);
            let on_server = test_notes::get(&server, &alice, note.id)
                .await
                .expect("the note");
            assert_eq!(on_server.version, 2);
            assert_eq!(
                test_notes::get(&a, &alice, note.id)
                    .await
                    .expect("the note"),
                on_server
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_pulled_row_never_replaces_a_row_with_a_pending_event() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let b = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "start", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            sync(&b, &server).await;
            test_notes::update(&a, &alice, note.id, "a's edit")
                .await
                .expect("edited on a");
            sync(&a, &server).await;

            test_notes::update(&b, &alice, note.id, "b's edit, offline")
                .await
                .expect("edited on b");
            let transport = PushLost {
                server: InProcess { server: &server },
                lost: std::sync::Mutex::new(true),
            };
            let report = run(&b, &transport).await.expect("a sync");
            assert!(report.push_error.is_some());
            assert_eq!(report.pulled, 2, "a's edit, and alice's row in the overlap");
            assert_eq!(
                test_notes::get(&b, &alice, note.id)
                    .await
                    .expect("the note")
                    .body,
                "b's edit, offline"
            );
        })
        .await;
    }

    /// A server on which the device makes another edit while its push is
    /// in flight.
    struct EditDuringPush<'a> {
        server: InProcess<'a>,
        device: &'a Deployment,
        user: &'a User,
        note_id: Uuid,
    }

    impl Transport for EditDuringPush<'_> {
        async fn push(
            &self,
            token: Option<&str>,
            request: PushRequest,
        ) -> Result<PushResponse, SyncError> {
            let response = self.server.push(token, request).await?;
            test_notes::update(
                self.device,
                self.user,
                self.note_id,
                "edited during the push",
            )
            .await
            .map_err(|error| SyncError::Table(error.into()))?;
            Ok(response)
        }

        async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
            self.server.pull(token, request).await
        }

        async fn backfill(
            &self,
            token: Option<&str>,
            request: BackfillRequest,
        ) -> Result<Page, SyncError> {
            self.server.backfill(token, request).await
        }

        async fn sign_in_with_device_code(
            &self,
            code: &str,
            verifier: &str,
        ) -> Result<Session, SyncError> {
            self.server.sign_in_with_device_code(code, verifier).await
        }

        async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
            self.server.sign_out(token).await
        }

        async fn bytes_uploads_create(
            &self,
            token: Option<&str>,
            table: &str,
            request: UploadNew,
        ) -> Result<Upload, SyncError> {
            self.server
                .bytes_uploads_create(token, table, request)
                .await
        }

        async fn bytes_upload(
            &self,
            token: Option<&str>,
            table: &str,
            upload: &Upload,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.bytes_upload(token, table, upload, path).await
        }

        async fn bytes_download(
            &self,
            token: Option<&str>,
            table: &str,
            id: Uuid,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.bytes_download(token, table, id, path).await
        }
    }

    #[tokio::test]
    async fn sync_an_event_queued_during_a_push_stays_pending_and_keeps_its_row() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "start", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            test_notes::update(&a, &alice, note.id, "first edit")
                .await
                .expect("edited");
            let transport = EditDuringPush {
                server: InProcess { server: &server },
                device: &a,
                user: &alice,
                note_id: note.id,
            };

            let report = run(&a, &transport).await.expect("a sync");
            assert_eq!(report.pushed, 1);
            assert_eq!(report.pending, 1);
            let on_a = test_notes::get(&a, &alice, note.id)
                .await
                .expect("the note");
            assert_eq!(on_a.body, "edited during the push");
            assert_eq!(on_a.version, 1);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 1);
            assert_eq!(report.conflicts, 0);
            assert_eq!(
                bodies(&server, &alice).await,
                vec!["edited during the push"]
            );
            assert_eq!(
                test_notes::get(&a, &alice, note.id)
                    .await
                    .expect("the note")
                    .version,
                3
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_conflict_reapplied_reaches_both_devices_and_one_let_go_sends_nothing() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let b = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "start", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            sync(&b, &server).await;
            test_notes::update(&a, &alice, note.id, "a's edit")
                .await
                .expect("edited on a");
            test_notes::update(&b, &alice, note.id, "b's edit")
                .await
                .expect("edited on b");
            sync(&a, &server).await;
            sync(&b, &server).await;
            let conflict = conflicts::list(&b).await.expect("the conflicts").remove(0);

            conflicts::reapply(&b, &alice, conflict.id)
                .await
                .expect("reapplied");
            assert!(conflicts::list(&b).await.expect("the conflicts").is_empty());
            assert_eq!(
                test_notes::get(&b, &alice, note.id)
                    .await
                    .expect("the note")
                    .body,
                "b's edit"
            );
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 1);
            sync(&a, &server).await;
            assert_eq!(bodies(&server, &alice).await, vec!["b's edit"]);
            assert_eq!(bodies(&a, &alice).await, vec!["b's edit"]);

            test_notes::update(&a, &alice, note.id, "a again")
                .await
                .expect("edited on a");
            test_notes::update(&b, &alice, note.id, "b again")
                .await
                .expect("edited on b");
            sync(&a, &server).await;
            sync(&b, &server).await;
            let conflict = conflicts::list(&b).await.expect("the conflicts").remove(0);
            conflicts::delete(&b, conflict.id).await.expect("let go");
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 0);
            assert_eq!(bodies(&server, &alice).await, vec!["a again"]);
            assert_eq!(bodies(&b, &alice).await, vec!["a again"]);
            let error = conflicts::delete(&b, conflict.id).await.expect_err("gone");
            assert!(matches!(error, conflicts::ConflictError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_pull_pages_in_cursor_order_and_resends_the_overlap() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let alice = users::create(&server, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let first = test_notes::create(&server, &alice, "first", None)
                .await
                .expect("a note");
            let second = test_notes::create(&server, &alice, "second", None)
                .await
                .expect("a note");
            let third = test_notes::create(&server, &alice, "third", None)
                .await
                .expect("a note");

            let page = pull(
                &server,
                &alice,
                PullRequest {
                    cursor: None,
                    after: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("a page");
            assert_eq!(page.rows.len(), 2);
            assert_eq!(page.rows[0].row["id"], json!(alice.id));
            assert_eq!(page.rows[1].row["id"], json!(first.id));
            let next = page.next.expect("another page");
            assert_eq!(next.id, first.id);
            assert_eq!(page.cursor, Some(next));

            let page = pull(
                &server,
                &alice,
                PullRequest {
                    cursor: None,
                    after: Some(next),
                    limit: Some(2),
                },
            )
            .await
            .expect("a page");
            assert_eq!(page.rows.len(), 3, "alice again, for the notes");
            assert_eq!(page.rows[0].row["id"], json!(alice.id));
            assert_eq!(page.rows[1].row["id"], json!(second.id));
            assert_eq!(page.rows[2].row["id"], json!(third.id));
            assert!(page.next.is_none());
            let cursor = page.cursor.expect("a cursor");
            assert_eq!(
                cursor.id,
                Uuid::nil(),
                "the server's time, on the last page"
            );
            assert!(cursor.last_synced_at >= third.last_synced_at.expect("synced"));

            let page = pull(
                &server,
                &alice,
                PullRequest {
                    cursor: Some(cursor),
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("a page");
            assert_eq!(
                page.rows.len(),
                4,
                "the overlap sends the last minute again"
            );
        })
        .await;
    }

    /// A server that must not be reached.
    struct Unreachable;

    impl Transport for Unreachable {
        async fn push(
            &self,
            _token: Option<&str>,
            _request: PushRequest,
        ) -> Result<PushResponse, SyncError> {
            panic!("the device is working offline and must not push")
        }

        async fn pull(
            &self,
            _token: Option<&str>,
            _request: PullRequest,
        ) -> Result<Page, SyncError> {
            panic!("the device is working offline and must not pull")
        }

        async fn backfill(
            &self,
            _token: Option<&str>,
            _request: BackfillRequest,
        ) -> Result<Page, SyncError> {
            panic!("the device is working offline and must not backfill")
        }

        async fn sign_in_with_device_code(
            &self,
            _code: &str,
            _verifier: &str,
        ) -> Result<Session, SyncError> {
            panic!("the device is working offline and must not sign in")
        }

        async fn sign_out(&self, _token: &str) -> Result<(), SyncError> {
            panic!("the device is working offline and must not sign out")
        }

        async fn bytes_uploads_create(
            &self,
            _token: Option<&str>,
            _table: &str,
            _request: UploadNew,
        ) -> Result<Upload, SyncError> {
            panic!("the device is working offline and must not ask about a file")
        }

        async fn bytes_upload(
            &self,
            _token: Option<&str>,
            _table: &str,
            _upload: &Upload,
            _path: &Path,
        ) -> Result<(), SyncError> {
            panic!("the device is working offline and must not send a file")
        }

        async fn bytes_download(
            &self,
            _token: Option<&str>,
            _table: &str,
            _id: Uuid,
            _path: &Path,
        ) -> Result<(), SyncError> {
            panic!("the device is working offline and must not fetch a file")
        }
    }

    #[tokio::test]
    async fn sync_working_offline_makes_no_request_and_holds_until_chosen_otherwise() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            set_offline(&a, true).await.expect("offline");
            assert!(status(&a).await.expect("the status").offline);
            test_notes::create(&a, &alice, "written offline", None)
                .await
                .expect("a note");

            let report = run(&a, &Unreachable).await.expect("a sync");
            assert!(report.offline);
            assert_eq!(report.pending, 1);
            assert!(report.push_error.is_none());
            assert!(bodies(&server, &alice).await.is_empty());

            set_offline(&a, false).await.expect("online");
            let report = sync(&a, &server).await;
            assert!(!report.offline);
            assert_eq!(report.pushed, 1);
            assert_eq!(bodies(&server, &alice).await, vec!["written offline"]);
            let status = status(&a).await.expect("the status");
            assert!(!status.offline);
            assert!(status.cursor.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_delete_on_the_server_hides_the_note_on_the_device() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let note = test_notes::create(&server, &alice, "on the web", None)
                .await
                .expect("a note");
            sync(&a, &server).await;
            assert_eq!(bodies(&a, &alice).await, vec!["on the web"]);

            test_notes::delete(&server, &alice, note.id)
                .await
                .expect("deleted");
            sync(&a, &server).await;
            assert!(bodies(&a, &alice).await.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_push_rejects_an_event_of_an_unknown_table() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let alice = users::create(&server, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let event = Event::create(
                "app_nothing",
                Uuid::now_v7(),
                app_events::Fields::new(),
                now(),
            );
            let response = push(
                &server,
                &alice,
                PushRequest {
                    events: vec![event],
                },
            )
            .await
            .expect("answered");
            assert_eq!(response.events[0].status, AnswerStatus::Rejected);
            assert_eq!(response.events[0].reason.as_deref(), Some("unknown_table"));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_rejected_create_drops_the_devices_row() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let a = device_of(&alice, &token).await;
            let note = test_notes::create(&a, &alice, "fine", None)
                .await
                .expect("a note");
            {
                // Rewrite the queued create to one the server refuses
                let mut tx = a.begin().await.expect("a transaction");
                let stored = app_events::pending(&mut tx)
                    .await
                    .expect("the events")
                    .remove(0);
                app_events::delete(&mut tx, stored.event.id)
                    .await
                    .expect("deleted");
                let mut broken = stored.event.clone();
                broken.changes = fields(json!({ "body": "x", "version": 5 }));
                crate::sync::events::queue(&mut tx, alice.id, &broken)
                    .await
                    .expect("queued");
                tx.commit().await.expect("committed");
            }

            let report = sync(&a, &server).await;
            assert_eq!(report.rejected, 1);
            assert_eq!(report.pending, 0);
            assert!(bodies(&a, &alice).await.is_empty());
            assert!(bodies(&server, &alice).await.is_empty());
            let error = test_notes::get(&a, &alice, note.id)
                .await
                .expect_err("dropped");
            assert!(matches!(error, NoteError::NotFound { .. }));
        })
        .await;
    }

    fn fields(value: Value) -> app_events::Fields {
        match value {
            Value::Object(fields) => fields,
            _ => panic!("fields are an object"),
        }
    }

    async fn community_names(deployment: &Deployment, user: &User, post_id: Uuid) -> Vec<String> {
        posts::get(deployment, user, post_id)
            .await
            .expect("the post")
            .communities
            .into_iter()
            .map(|community| community.name)
            .collect()
    }

    #[tokio::test]
    async fn sync_joining_on_the_web_fills_the_device_and_leaving_empties_it() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            let note = test_notes::create(&server, &alice, "roses", Some(gardening.id))
                .await
                .expect("a note");
            let b = device_of(&bob, &bob_token).await;
            let report = sync(&b, &server).await;
            assert_eq!(report.backfilled, 0);
            assert!(bodies(&b, &bob).await.is_empty());
            let listed = communities::list(&b, &bob).await.expect("the communities");
            assert_eq!(listed[0].community.name, "Gardening");
            assert!(listed[0].membership.is_none());

            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            let report = sync(&b, &server).await;
            assert!(report.backfilled > 0, "{report:?}");
            assert_eq!(bodies(&b, &bob).await, vec!["roses"]);
            assert_eq!(
                test_notes::get(&b, &bob, note.id)
                    .await
                    .expect("the note")
                    .community_id,
                Some(gardening.id)
            );
            let mut tx = b.begin().await.expect("a transaction");
            let owner = users::get_in(&mut tx, alice.id)
                .await
                .expect("the owner")
                .expect("pulled with the note");
            tx.commit().await.expect("committed");
            assert_eq!(owner.name, "alice");
            let listed = communities::list(&b, &bob).await.expect("the communities");
            assert!(listed[0].membership.is_some());

            let report = sync(&b, &server).await;
            assert_eq!(report.backfilled, 0, "once is enough");

            memberships::delete(&server, &bob, gardening.id)
                .await
                .expect("left");
            let report = sync(&b, &server).await;
            assert_eq!(report.pruned, 1);
            assert!(bodies(&b, &bob).await.is_empty());
            let error = test_notes::get(&b, &bob, note.id).await.expect_err("gone");
            assert!(matches!(error, NoteError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_community_deleted_on_the_web_empties_a_members_device() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            test_notes::create(&server, &alice, "roses", Some(gardening.id))
                .await
                .expect("a note");
            let b = device_of(&bob, &bob_token).await;
            sync(&b, &server).await;
            assert_eq!(bodies(&b, &bob).await, vec!["roses"]);

            communities::delete(&server, &alice, gardening.id)
                .await
                .expect("deleted");
            let report = sync(&b, &server).await;
            assert_eq!(report.pruned, 1);
            assert!(bodies(&b, &bob).await.is_empty());
            assert!(
                communities::list(&b, &bob)
                    .await
                    .expect("the communities")
                    .is_empty()
            );
            assert!(
                memberships::list_mine(&b, &bob)
                    .await
                    .expect("memberships")
                    .is_empty()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_name_changed_on_a_device_reaches_the_server_and_other_members() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, alice_token) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            let a = device_of(&alice, &alice_token).await;
            let b = device_of(&bob, &bob_token).await;
            sync(&a, &server).await;
            sync(&b, &server).await;

            let renamed = users::update(&a, &alice, "Alice B")
                .await
                .expect("renamed offline");
            assert_eq!(renamed.name, "Alice B");
            assert_eq!(
                renamed.version, alice.version,
                "a device leaves the version"
            );
            sync(&a, &server).await;
            let on_server = users::get(&server, alice.id).await.expect("alice");
            assert_eq!(on_server.name, "Alice B");
            assert_eq!(on_server.version, alice.version + 1);
            assert_eq!(users::get(&a, alice.id).await.expect("alice"), on_server);

            sync(&b, &server).await;
            assert_eq!(
                users::get(&b, alice.id).await.expect("alice").name,
                "Alice B"
            );

            let event = Event::update(
                app_users::TABLE,
                alice.id,
                on_server.version,
                crate::data::app_events::Fields::from_iter([(
                    "name".to_string(),
                    json!("Alice B"),
                )]),
                crate::data::app_events::Fields::from_iter([(
                    "name".to_string(),
                    json!("Mallory"),
                )]),
                now(),
            );
            let answer = users::apply_event(&server, &bob, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected, "bob renaming alice");
            assert_eq!(answer.reason.as_deref(), Some("not_yours"));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_community_renamed_and_deleted_on_a_device_reaches_the_server() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, alice_token) = user_with_token(&server, "alice@example.com").await;
            let (bob, _) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            let a = device_of(&alice, &alice_token).await;
            sync(&a, &server).await;

            let renamed = communities::update(&a, &alice, gardening.id, "Weeding")
                .await
                .expect("renamed offline");
            assert_eq!(renamed.version, 1, "a device leaves the version");
            sync(&a, &server).await;
            let on_server = communities::get(&server, &bob, gardening.id)
                .await
                .expect("found");
            assert_eq!(on_server.community.name, "Weeding");
            assert_eq!(on_server.community.version, 2);

            communities::delete(&a, &alice, gardening.id)
                .await
                .expect("deleted offline");
            assert!(
                memberships::list_mine(&a, &alice)
                    .await
                    .expect("memberships")
                    .is_empty(),
                "left at once on the device"
            );
            assert_eq!(pending(&a).await, 1, "one event, the rest copied down");
            sync(&a, &server).await;
            assert_eq!(pending(&a).await, 0);
            let error = communities::get(&server, &bob, gardening.id)
                .await
                .expect_err("gone on the server");
            assert!(matches!(error, CommunityError::NotFound { .. }));
            assert!(
                memberships::list_mine(&server, &bob)
                    .await
                    .expect("memberships")
                    .is_empty(),
                "bob's membership went with it"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_joining_from_the_device_pushes_the_membership_and_backfills() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            test_notes::create(&server, &alice, "roses", Some(gardening.id))
                .await
                .expect("a note");
            let b = device_of(&bob, &bob_token).await;
            sync(&b, &server).await;

            let membership = memberships::create(&b, &bob, gardening.id)
                .await
                .expect("joined offline");
            assert_eq!(membership.version, 0);
            assert_eq!(pending(&b).await, 1);
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 1);
            assert!(report.backfilled > 0, "{report:?}");
            assert_eq!(bodies(&b, &bob).await, vec!["roses"]);
            let mine = memberships::list_mine(&b, &bob).await.expect("memberships");
            assert_eq!(mine.len(), 1);
            assert_eq!(mine[0].id, membership.id);
            assert_eq!(mine[0].version, 1);
            assert_eq!(
                memberships::list_mine(&server, &bob)
                    .await
                    .expect("memberships")[0]
                    .id,
                membership.id
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_post_taken_out_of_a_community_leaves_the_other_members_device_and_comes_back() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            let post = posts::create(&server, &alice, "roses", &[gardening.id])
                .await
                .expect("a post");
            let b = device_of(&bob, &bob_token).await;
            sync(&b, &server).await;
            assert_eq!(post_bodies(&b, &bob).await, vec!["roses"]);

            posts_communities::remove(&server, &alice, post.id, gardening.id)
                .await
                .expect("removed");
            let report = sync(&b, &server).await;
            assert_eq!(report.pruned, 1);
            assert!(post_bodies(&b, &bob).await.is_empty());

            posts_communities::add(&server, &alice, post.id, gardening.id)
                .await
                .expect("added back");
            let report = sync(&b, &server).await;
            assert_eq!(report.pruned, 0);
            assert_eq!(post_bodies(&b, &bob).await, vec!["roses"]);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_device_rejoining_a_community_gets_the_servers_membership_back() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, alice_token) = user_with_token(&server, "alice@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            let first = memberships::list_mine(&server, &alice)
                .await
                .expect("memberships")
                .remove(0);
            memberships::delete(&server, &alice, gardening.id)
                .await
                .expect("left");

            // A device that never synced joins with a membership of its own
            let a = device_of(&alice, &alice_token).await;
            {
                let mut tx = a.begin().await.expect("a transaction");
                communities::apply_server_row(
                    &mut tx,
                    &communities::table_row(&gardening).expect("a row").row,
                )
                .await
                .expect("the community on a");
                tx.commit().await.expect("committed");
            }
            let made_on_a = memberships::create(&a, &alice, gardening.id)
                .await
                .expect("joined offline");
            assert_ne!(made_on_a.id, first.id);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 1);
            let mine = memberships::list_mine(&a, &alice)
                .await
                .expect("memberships");
            assert_eq!(mine.len(), 1);
            assert_eq!(mine[0].id, first.id, "the server's row, revived");
            assert_eq!(mine[0].version, 3);
            let on_server = memberships::list_mine(&server, &alice)
                .await
                .expect("memberships");
            assert_eq!(on_server.len(), 1);
            assert_eq!(on_server[0].id, first.id);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_adding_a_post_to_a_community_left_meanwhile_is_refused_and_dropped() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (bob, bob_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &bob, "Gardening")
                .await
                .expect("a community");
            let b = device_of(&bob, &bob_token).await;
            sync(&b, &server).await;
            memberships::delete(&server, &bob, gardening.id)
                .await
                .expect("left on the web");

            let post = posts::create(&b, &bob, "late", &[gardening.id])
                .await
                .expect("a post, offline");
            assert_eq!(community_names(&b, &bob, post.id).await, vec!["Gardening"]);
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 1, "the post");
            assert_eq!(report.rejected, 1, "its community row");
            assert_eq!(report.pending, 0);
            assert_eq!(post_bodies(&b, &bob).await, vec!["late"]);
            assert!(community_names(&b, &bob, post.id).await.is_empty());
            assert!(community_names(&server, &bob, post.id).await.is_empty());
            assert!(
                memberships::list_mine(&b, &bob)
                    .await
                    .expect("memberships")
                    .is_empty()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_pull_sends_each_row_after_the_rows_it_points_at() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let alice = users::create(&server, "alice@example.com", "Alice")
                .await
                .expect("a user");
            let bob = users::create(&server, "bob@example.com", "Bob")
                .await
                .expect("a user");
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined");
            let note = test_notes::create(&server, &alice, "roses", Some(gardening.id))
                .await
                .expect("a note");
            // The community changes last, so by the cursor alone it would
            // come after the note
            communities::update(&server, &alice, gardening.id, "Gardens")
                .await
                .expect("renamed");

            let page = pull(
                &server,
                &bob,
                PullRequest {
                    cursor: None,
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("a page");
            let tables: Vec<&str> = page.rows.iter().map(|row| row.table.as_str()).collect();
            let position = |table: &str, id: Uuid| {
                page.rows
                    .iter()
                    .position(|row| row.table == table && row.row["id"] == json!(id))
                    .unwrap_or_else(|| panic!("{table} {id} in {tables:?}"))
            };
            assert!(position(app_users::TABLE, alice.id) < position(test_notes::TABLE, note.id));
            assert!(
                position(app_communities::TABLE, gardening.id)
                    < position(test_notes::TABLE, note.id)
            );
            assert_eq!(
                page.rows
                    .iter()
                    .filter(|row| {
                        row.table == app_communities::TABLE && row.row["id"] == json!(gardening.id)
                    })
                    .count(),
                1,
                "sent once"
            );
            assert_eq!(page.cursor.expect("a cursor").id, Uuid::nil());
        })
        .await;
    }

    /// Runs an update of one row's times, for rows that must look old.
    async fn backdate(deployment: &Deployment, statement: &'static str, id: Uuid) {
        let at = now() - Duration::days(100);
        let mut tx = deployment.begin().await.expect("a transaction");
        match &mut tx {
            Transaction::Sqlite(tx) => {
                sqlx::query(statement)
                    .bind(at)
                    .bind(id.hyphenated())
                    .execute(&mut **tx)
                    .await
                    .expect("backdated");
            }
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => {
                sqlx::query(statement)
                    .bind(at)
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .expect("backdated");
            }
        }
        tx.commit().await.expect("committed");
    }

    /// Deletes the note on the server as if long ago, and purges it.
    async fn delete_long_ago_and_purge(server: &Deployment, alice: &User, note_id: Uuid) {
        test_notes::delete(server, alice, note_id)
            .await
            .expect("deleted");
        backdate(
            server,
            "UPDATE app_test_notes SET deleted_at = $1, last_synced_at = $1 WHERE id = $2",
            note_id,
        )
        .await;
        let report = purge::run(server, now()).await.expect("purged");
        assert_eq!(report.removed[test_notes::TABLE], 1);
    }

    /// Makes the device's last pull look older than the purge window.
    async fn backdate_cursor(device: &Deployment) {
        backdate(
            device,
            "UPDATE app_sync_state SET cursor_last_synced_at = $1 WHERE cursor_id != $2",
            Uuid::max(),
        )
        .await;
    }

    #[tokio::test]
    async fn sync_a_device_away_longer_than_the_purge_window_starts_over_and_matches() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            let kept = test_notes::create(&server, &alice, "kept", Some(gardening.id))
                .await
                .expect("a note");
            let gone = test_notes::create(&server, &alice, "gone", Some(gardening.id))
                .await
                .expect("a note");
            let a = device_of(&alice, &token).await;
            sync(&a, &server).await;
            assert_eq!(bodies(&a, &alice).await, vec!["gone", "kept"]);

            delete_long_ago_and_purge(&server, &alice, gone.id).await;
            backdate_cursor(&a).await;
            let report = sync(&a, &server).await;
            assert!(report.resynced, "{report:?}");
            assert!(
                report.backfilled > 0,
                "joined again, as far as the device knows"
            );
            assert_eq!(bodies(&a, &alice).await, vec!["kept"]);
            let error = test_notes::get(&a, &alice, gone.id)
                .await
                .expect_err("gone");
            assert!(matches!(error, NoteError::NotFound { .. }));
            assert_eq!(
                test_notes::get(&a, &alice, kept.id)
                    .await
                    .expect("the note")
                    .community_id,
                Some(gardening.id)
            );
            assert_eq!(
                memberships::list_mine(&a, &alice)
                    .await
                    .expect("memberships")
                    .len(),
                1
            );

            let report = sync(&a, &server).await;
            assert!(!report.resynced, "once is enough: {report:?}");
            assert_eq!(report.backfilled, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_pending_change_survives_a_resync_and_the_server_answers_it_next() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let note = test_notes::create(&server, &alice, "start", None)
                .await
                .expect("a note");
            let old = test_notes::create(&server, &alice, "old", None)
                .await
                .expect("a note");
            let a = device_of(&alice, &token).await;
            sync(&a, &server).await;
            test_notes::update(&a, &alice, note.id, "edited offline")
                .await
                .expect("edited");
            delete_long_ago_and_purge(&server, &alice, old.id).await;
            backdate_cursor(&a).await;

            let transport = PushLost {
                server: InProcess { server: &server },
                lost: std::sync::Mutex::new(true),
            };
            let report = run(&a, &transport).await.expect("a sync");
            assert!(report.push_error.is_some());
            assert!(report.resynced);
            assert_eq!(report.pending, 1, "the change is kept");
            assert_eq!(
                bodies(&a, &alice).await,
                vec!["edited offline"],
                "and so is its row, as the person left it"
            );

            let report = run(&a, &transport).await.expect("a sync");
            assert_eq!(report.pushed, 1);
            assert!(!report.resynced);
            assert_eq!(bodies(&server, &alice).await, vec!["edited offline"]);
            assert_eq!(
                test_notes::get(&a, &alice, note.id)
                    .await
                    .expect("the note")
                    .version,
                2
            );
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_change_to_a_row_the_server_purged_is_kept_as_a_conflict_to_let_go() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let note = test_notes::create(&server, &alice, "start", None)
                .await
                .expect("a note");
            let a = device_of(&alice, &token).await;
            sync(&a, &server).await;
            test_notes::update(&a, &alice, note.id, "edited offline")
                .await
                .expect("edited");
            delete_long_ago_and_purge(&server, &alice, note.id).await;

            let report = sync(&a, &server).await;
            assert_eq!(report.rejected, 1);
            assert_eq!(report.conflicts, 1);
            assert_eq!(report.pending, 0);
            assert!(
                bodies(&a, &alice).await.is_empty(),
                "the copy of a purged row goes"
            );
            let conflicts = conflicts::list(&a).await.expect("the conflicts");
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].about, None, "a note that is gone");
            assert_eq!(conflicts[0].fields[0].yours, json!("edited offline"));
            assert_eq!(conflicts[0].fields[0].now, Value::Null);
            let error = conflicts::reapply(&a, &alice, conflicts[0].id)
                .await
                .expect_err("nothing to reapply to");
            assert!(matches!(error, conflicts::ConflictError::Gone));
            conflicts::delete(&a, conflicts[0].id)
                .await
                .expect("let go");
            assert_eq!(status(&a).await.expect("the status").conflicts, 0);
        })
        .await;
    }

    /// A sign-in, a sign-up the first time, finished on the server's own
    /// database with the code the memory mailer caught, as a device: its
    /// code exchanged over HTTP.
    #[cfg(feature = "sync-http")]
    async fn device_session_over(
        server: &Http,
        servers_database: &Deployment,
        email: &str,
    ) -> Session {
        let mailer = crate::services::mailer::Mailer::memory();
        let sent = accounts::sign_in(
            servers_database,
            &mailer,
            email,
            true,
            crate::helpers::locales::DEFAULT,
        )
        .await
        .expect("signed in");
        let code = crate::accounts::tests::code_sent_to(&mailer, email);
        let (verifier, challenge) = accounts::verifier_and_challenge();
        let matched = accounts::sign_in_with_code(servers_database, &sent.pending, &code)
            .await
            .expect("the code matched");
        let device_code =
            accounts::sign_in_confirm_for_device(servers_database, &matched.pending, &challenge)
                .await
                .expect("finished");
        accounts::sign_in_with_device_code_through(server, &device_code.code, &verifier)
            .await
            .expect("exchanged over HTTP")
    }

    #[tokio::test]
    async fn sync_a_post_taken_out_before_a_device_joined_is_never_sent_to_it() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let (alice, _) = user_with_token(&server, "alice@example.com").await;
            let (bob, bobs_token) = user_with_token(&server, "bob@example.com").await;
            let gardening = communities::create(&server, &alice, "Gardening")
                .await
                .expect("a community");
            let cooking = communities::create(&server, &alice, "Cooking")
                .await
                .expect("a community");
            let post = posts::create(&server, &alice, "roses", &[gardening.id, cooking.id])
                .await
                .expect("a post");
            posts_communities::remove(&server, &alice, post.id, gardening.id)
                .await
                .expect("taken out of gardening");
            memberships::create(&server, &bob, gardening.id)
                .await
                .expect("joined after");

            let page = pull(&server, &bob, PullRequest::default())
                .await
                .expect("a page");
            assert!(
                page.rows.iter().all(|row| row.table != app_posts::TABLE),
                "the post is in cooking only, which bob is not in"
            );
            let device = device_of(&bob, &bobs_token).await;
            run(&device, &InProcess { server: &server })
                .await
                .expect("a sync");
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                posts::get_in(&mut tx, post.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }

    /// A file added on one device reaches a running web server's bucket
    /// and is fetched by another, over HTTP and the bucket's signed
    /// URLs, as the apps do; run as `sync_two_devices_over_http` is,
    /// with the server's bucket set up:
    ///
    /// ```sh
    /// SYNC_SERVER_URL=http://localhost:4010 TEST_SERVER_DATABASE_URL=sqlite:../axum/data/app.db \
    ///   cargo test -p app_core --features sync-http a_file_over_http -- --ignored
    /// ```
    #[cfg(feature = "sync-http")]
    #[tokio::test]
    #[ignore]
    async fn sync_a_file_over_http() {
        use crate::app::files::FileState;
        use crate::deployment::FileStore;

        let url = std::env::var("SYNC_SERVER_URL").expect("SYNC_SERVER_URL");
        let server = Http::new(&url).expect("a transport");
        let servers_database =
            crate::database::tests::open_from_env("TEST_SERVER_DATABASE_URL").await;
        let servers_database = Deployment::new(servers_database, Role::Server);
        let email = format!("{}@example.com", Uuid::now_v7().simple());
        let a_folder = tempfile::tempdir().expect("a folder");
        let b_folder = tempfile::tempdir().expect("a folder");
        let a = users::tests::device()
            .await
            .with_file_store(FileStore::Folder(a_folder.path().to_path_buf()));
        let b = users::tests::device()
            .await
            .with_file_store(FileStore::Folder(b_folder.path().to_path_buf()));
        let alice = {
            let session = device_session_over(&server, &servers_database, &email).await;
            accounts::keep_signed_in(&a, session)
                .await
                .expect("signed in on a")
        };
        let session = device_session_over(&server, &servers_database, &email).await;
        accounts::keep_signed_in(&b, session)
            .await
            .expect("signed in on b");

        let bytes: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
        run(&a, &server).await.expect("a sync for her communities");
        let personal = memberships::list_mine(&a, &alice)
            .await
            .expect("the memberships")[0]
            .community_id;
        let file = files::create_with_bytes(&a, &alice, "over http.bin", "", &[personal], &bytes)
            .await
            .expect("a file");
        let report = run(&a, &server).await.expect("a sync");
        assert_eq!(report.uploaded, 1, "{report:?}");
        assert_eq!(report.pending, 0, "{report:?}");
        let shown = files::get(&a, &alice, file.id).await.expect("on a");
        assert_eq!(shown.state, FileState::Synced);

        let report = run(&a, &server).await.expect("a sync");
        assert_eq!(report.uploaded, 0);
        run(&b, &server).await.expect("a sync");
        let path = files::local_copy(&b, &server, &alice, file.id)
            .await
            .expect("fetched on b");
        assert_eq!(std::fs::read(path).expect("the bytes"), bytes);

        files::delete(&b, &alice, file.id)
            .await
            .expect("deleted on b");
        let report = run(&b, &server).await.expect("a sync");
        assert_eq!(report.cleaned, 1);
        accounts::sign_out_on_device(&b, &server)
            .await
            .expect("signed out on b");
        accounts::sign_out_on_device(&a, &server)
            .await
            .expect("signed out on a");
    }

    /// Many files added on a device, `FILES` of them, 1,000 unless set,
    /// reach a running web server's bucket a sync of
    /// `crate::sync::bytes::PER_SYNC` at a time, each sync starting the next as
    /// the app does; prints how long each took. Run as
    /// `sync_a_file_over_http` is:
    ///
    /// ```sh
    /// SYNC_SERVER_URL=http://localhost:4010 TEST_SERVER_DATABASE_URL=sqlite:../axum/data/app.db \
    ///   cargo test -p app_core --features sync-http many_files_over_http -- --ignored --nocapture
    /// ```
    #[cfg(feature = "sync-http")]
    #[tokio::test]
    #[ignore]
    async fn sync_many_files_over_http() {
        use crate::app::files::FileState;
        use crate::deployment::FileStore;

        let url = std::env::var("SYNC_SERVER_URL").expect("SYNC_SERVER_URL");
        let count: usize = std::env::var("FILES")
            .ok()
            .and_then(|count| count.parse().ok())
            .unwrap_or(1000);
        let server = Http::new(&url).expect("a transport");
        let servers_database =
            crate::database::tests::open_from_env("TEST_SERVER_DATABASE_URL").await;
        let servers_database = Deployment::new(servers_database, Role::Server);
        let email = format!("{}@example.com", Uuid::now_v7().simple());
        let folder = tempfile::tempdir().expect("a folder");
        let a = users::tests::device()
            .await
            .with_file_store(FileStore::Folder(folder.path().to_path_buf()));
        let alice = {
            let session = device_session_over(&server, &servers_database, &email).await;
            accounts::keep_signed_in(&a, session)
                .await
                .expect("signed in on a")
        };
        run(&a, &server).await.expect("a sync for her communities");
        let personal = memberships::list_mine(&a, &alice)
            .await
            .expect("the memberships")[0]
            .community_id;

        let started = std::time::Instant::now();
        let mut ids = Vec::with_capacity(count);
        for number in 0..count {
            let bytes = format!("emoji number {number}");
            let file = files::create_with_bytes(
                &a,
                &alice,
                &format!("{number}.txt"),
                "text/plain",
                &[personal],
                bytes.as_bytes(),
            )
            .await
            .expect("a file");
            ids.push(file.id);
        }
        eprintln!(
            "{count} files kept on the device in {:?}",
            started.elapsed()
        );

        let started = std::time::Instant::now();
        let mut syncs = 0;
        loop {
            let one = std::time::Instant::now();
            let report = run(&a, &server).await.expect("a sync");
            syncs += 1;
            eprintln!(
                "sync {syncs}: {:?}, uploaded {}, pushed {}, left {}, pulled {}",
                one.elapsed(),
                report.uploaded,
                report.pushed,
                report.files_left,
                report.pulled
            );
            if !report.more_files_to_send() {
                assert_eq!(report.pending, 0, "{report:?}");
                break;
            }
        }
        eprintln!(
            "{count} files sent in {syncs} syncs, {:?}",
            started.elapsed()
        );
        for id in ids {
            let shown = files::get(&a, &alice, id).await.expect("on a");
            assert_eq!(shown.state, FileState::Synced);
        }
        accounts::sign_out_on_device(&a, &server)
            .await
            .expect("signed out on a");
    }

    /// Two devices against a running web server, `SYNC_SERVER_URL`,
    /// whose database, `TEST_SERVER_DATABASE_URL`, the test opens itself
    /// to sign a user up and read nothing from the server's log, then
    /// exchanges the device codes over HTTP as the apps do:
    ///
    /// ```sh
    /// SYNC_SERVER_URL=http://localhost:4010 TEST_SERVER_DATABASE_URL=sqlite:../axum/data/app.db \
    ///   cargo test -p app_core --features sync-http two_devices -- --ignored
    /// ```
    #[cfg(feature = "sync-http")]
    #[tokio::test]
    #[ignore]
    async fn sync_two_devices_over_http() {
        let url = std::env::var("SYNC_SERVER_URL").expect("SYNC_SERVER_URL");
        let server = Http::new(&url).expect("a transport");
        let servers_database =
            crate::database::tests::open_from_env("TEST_SERVER_DATABASE_URL").await;
        let servers_database = Deployment::new(servers_database, Role::Server);
        let email = format!("{}@example.com", Uuid::now_v7().simple());
        let a = users::tests::device().await;
        let b = users::tests::device().await;
        let alice = {
            let session = device_session_over(&server, &servers_database, &email).await;
            accounts::keep_signed_in(&a, session)
                .await
                .expect("signed in on a")
        };
        let session = device_session_over(&server, &servers_database, &email).await;
        accounts::keep_signed_in(&b, session)
            .await
            .expect("signed in on b");
        let post = posts::create(&a, &alice, "over http from a", &[])
            .await
            .expect("a post");
        run(&a, &server).await.expect("a sync");
        run(&b, &server).await.expect("a sync");
        assert_eq!(
            posts::get(&b, &alice, post.id)
                .await
                .expect("the post on b")
                .post
                .body,
            "over http from a"
        );

        posts::update(&a, &alice, post.id, "a's edit", &[])
            .await
            .expect("edited on a");
        posts::update(&b, &alice, post.id, "b's edit", &[])
            .await
            .expect("edited on b");
        run(&a, &server).await.expect("a sync");
        let report = run(&b, &server).await.expect("a sync");
        assert_eq!(report.conflicts, 1);
        assert_eq!(
            posts::get(&b, &alice, post.id)
                .await
                .expect("the post on b")
                .post
                .body,
            "a's edit"
        );
        conflicts::delete(&b, conflicts::list(&b).await.expect("one")[0].id)
            .await
            .expect("let go");
        accounts::sign_out_on_device(&b, &server)
            .await
            .expect("signed out on b");
        accounts::sign_out_on_device(&a, &server)
            .await
            .expect("signed out on a");
    }

    #[test]
    fn sync_protocol_version_refusal_names_the_side_to_update() {
        let own = PROTOCOL_VERSION.to_string();
        assert_eq!(protocol_version_refusal(Some(&own)), None);
        let older = protocol_version_refusal(Some("0")).expect("refused");
        assert!(older.ends_with("update the app"), "{older}");
        let newer = (PROTOCOL_VERSION + 1).to_string();
        let newer = protocol_version_refusal(Some(&newer)).expect("refused");
        assert!(newer.ends_with("the server needs updating"), "{newer}");
        assert!(protocol_version_refusal(None).is_some(), "none sent");
        assert!(
            protocol_version_refusal(Some("one")).is_some(),
            "not a number"
        );
    }
}
