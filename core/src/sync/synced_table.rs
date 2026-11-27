//! A synced table's part in sync, conflicts, pending changes, purge and
//! a local user's move to an account, so that machinery names no table
//! of the app's: it runs each operation over the tables listed in
//! `app::synced_tables`, see `docs/design/template-and-apps.md`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::data::app_communities::Community;
use crate::data::app_events::{Event, Fields};
use crate::data::app_sync_state;
use crate::data::app_users::User;
use crate::data::app_users_communities::Membership;
use crate::database::Transaction;
use crate::deployment::{Deployment, Role};
use crate::sync::bytes::SyncedBytes;
use crate::sync::events::Answer;
use crate::sync::{TableRow, VersionRange};

/// The error of a table's own module, as the machinery passes it on.
pub type TableError = Box<dyn std::error::Error + Send + Sync>;

/// A local user's row, as the create its move to an account queues.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalCreate {
    pub id: Uuid,
    pub inserted_at: DateTime<Utc>,
    /// The rows it points at, whose creates go first.
    pub parents: Vec<Uuid>,
    pub changes: Fields,
}

/// The row a conflict is about, as it stands now.
#[derive(Clone, Debug, PartialEq)]
pub struct CurrentFields {
    pub version: i64,
    /// The fields an event may change.
    pub fields: Fields,
}

/// One synced table. Every operation the machinery runs on each table
/// is required, so a table the app adds cannot miss one; only what some
/// tables do has a default.
#[async_trait]
pub trait SyncedTable: Send + Sync {
    /// The table's name, as in `app_events.table_name` and on the wire.
    fn table(&self) -> &'static str;

    /// The column naming the user who made a row, which a local user's
    /// move gives to the account; none when the row has no maker.
    fn owner_column(&self) -> Option<&'static str>;

    /// Foreign keys from this table to other synced rows, as (column, table).
    /// Resync and purge keep parents until their dependent rows can go.
    fn foreign_keys(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The table's part in the Rust server's sync, or none for a table
    /// the app's remote backend serves instead, see
    /// `docs/design/backend-seam.md`.
    fn server(&self) -> Option<&dyn SyncedTableServer>;

    /// Applies an event of the user's in the caller's transaction and
    /// answers it, recording it on the server or queueing it on a device.
    async fn apply_in(
        &self,
        tx: &mut Transaction,
        role: Role,
        user: &User,
        event: &Event,
    ) -> Result<Answer, TableError>;

    /// Applies an event in a transaction of its own: what the server
    /// does with a pushed event.
    async fn apply_event(
        &self,
        deployment: &Deployment,
        user: &User,
        event: &Event,
    ) -> Result<Answer, TableError> {
        let mut tx = deployment.begin().await?;
        let answer = self.apply_in(&mut tx, deployment.role, user, event).await?;
        tx.commit().await?;
        Ok(answer)
    }

    /// Writes the server's row over the device's, unless an event of the
    /// device's for it is still pending; true when it was written.
    async fn apply_server_row(
        &self,
        tx: &mut Transaction,
        user: &User,
        row: &TableRow,
    ) -> Result<bool, TableError>;

    /// Removes a device's row the server will never have.
    async fn delete_row(&self, tx: &mut Transaction, id: Uuid) -> Result<(), TableError>;

    /// The local user's rows, as the creates their move queues.
    async fn local_user_creates(
        &self,
        tx: &mut Transaction,
        user: &User,
    ) -> Result<Vec<LocalCreate>, TableError>;

    /// What a row is, in words for a person: a post's first line, a
    /// file's name. None for a table whose rows a page does not name.
    async fn label(&self, _tx: &mut Transaction, _id: Uuid) -> Result<Option<String>, TableError> {
        Ok(None)
    }

    /// A live row's version and the fields an event may change, for a
    /// conflict to be reapplied over. None for a table whose conflicts
    /// are only let go.
    async fn current_fields(
        &self,
        _tx: &mut Transaction,
        _id: Uuid,
    ) -> Result<Option<CurrentFields>, TableError> {
        Ok(None)
    }

    /// Moves the device's rows from one community to another, through the
    /// account's membership of it, when a local user's communities are
    /// merged into the account's on a server with a single community,
    /// before the first community is deleted; nothing for a table not in
    /// communities. A table that implements
    /// `list_live_in_community_after` must implement this too, or its
    /// rows are deleted with the community.
    async fn move_rows_to_community(
        &self,
        _tx: &mut Transaction,
        _from_community_id: Uuid,
        _to: &Membership,
    ) -> Result<(), TableError> {
        Ok(())
    }

    /// Deletes the user's rows with their account, as events in their
    /// name; nothing for a table whose rows outlive their maker.
    async fn delete_of_user(
        &self,
        _tx: &mut Transaction,
        _role: Role,
        _user: &User,
    ) -> Result<(), TableError> {
        Ok(())
    }

    /// Deletes the live rows under the membership at its moment, when it
    /// is left or its community deleted; nothing for a table whose rows
    /// are not a member's.
    async fn delete_with_membership(
        &self,
        _tx: &mut Transaction,
        _role: Role,
        _membership: &Membership,
    ) -> Result<(), TableError> {
        Ok(())
    }

    /// Revives the rows that left with the membership at `left_at`, when
    /// it is joined again.
    async fn revive_with_membership(
        &self,
        _tx: &mut Transaction,
        _role: Role,
        _membership: &Membership,
        _left_at: DateTime<Utc>,
    ) -> Result<(), TableError> {
        Ok(())
    }

    /// Deletes the live rows in the community at its moment, when it is
    /// deleted; nothing for a table not in communities.
    async fn delete_with_community(
        &self,
        _tx: &mut Transaction,
        _role: Role,
        _community: &Community,
    ) -> Result<(), TableError> {
        Ok(())
    }

    /// Removes from a device the rows the user no longer sees, after a
    /// pull; how many went.
    async fn prune(&self, _tx: &mut Transaction, _user: &User) -> Result<u64, TableError> {
        Ok(0)
    }

    /// Drops the device's rows for a resync from nothing, but those an
    /// event still pending is about, which the next push answers.
    async fn delete_for_resync(
        &self,
        tx: &mut Transaction,
        _user: &User,
    ) -> Result<(), TableError> {
        Ok(app_sync_state::maintenance::delete_not_pending(tx, self.table()).await?)
    }

    /// What the rows carry beside their fields, a file's bytes say, see
    /// `sync::bytes`; none for most tables.
    fn bytes(&self) -> Option<&dyn SyncedBytes> {
        None
    }

    /// Whether the rows count among a local user's things, in the
    /// question asked before a move.
    fn counted_before_a_move(&self) -> bool {
        false
    }
}

/// A synced table's part in the Rust server's sync: the pages a pull and a
/// backfill answer, and the parents sent before a row. A table whose
/// server is a remote backend needs none of it.
#[async_trait]
pub trait SyncedTableServer: Send + Sync {
    /// A page of the rows the server wrote after the cursor that the
    /// user sees, deleted or not.
    async fn list_synced_after(
        &self,
        tx: &mut Transaction,
        user: &User,
        range: &VersionRange,
        limit: usize,
    ) -> Result<Vec<TableRow>, TableError>;

    /// The rows this one points at, which a pull sends before it.
    async fn parents(
        &self,
        tx: &mut Transaction,
        row: &TableRow,
    ) -> Result<Vec<TableRow>, TableError>;

    /// The live rows in a community after the cursor, for a device that
    /// just joined it; none for a table not in communities.
    async fn list_live_in_community_after(
        &self,
        _tx: &mut Transaction,
        _community_id: Uuid,
        _range: &VersionRange,
        _limit: usize,
    ) -> Result<Vec<TableRow>, TableError> {
        Ok(Vec::new())
    }
}
