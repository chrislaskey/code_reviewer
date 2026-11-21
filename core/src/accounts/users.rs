//! A person's account, as the pages ask for it. The account's records,
//! its email rules and its reads and writes are `data::accounts_users`.

use uuid::Uuid;

use crate::data::accounts_users::{self, AccountUser, AccountUserError};
use crate::deployment::Deployment;

/// One account by its id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<AccountUser, AccountUserError> {
    let mut tx = deployment.begin().await?;
    let account = accounts_users::get(&mut tx, id).await?;
    tx.commit().await?;
    account.ok_or(AccountUserError::NotFound { id })
}
