//! Deleting an account, by the person whose it is, see
//! `docs/design/accounts-delete.md`. A full user asks for a code,
//! `code_create`, which is mailed as a sign-in's is, and gives it back
//! to `create`, which deletes everything of theirs in one transaction:
//! their posts and memberships, and a community nobody else is in, each
//! soft-deleted as an event in their name, so every device that pulled
//! a row sees it go; their user, named "Deleted user"; their staff row;
//! and their account, so sign-in finds it no longer and a token of
//! theirs is answered `410`. Their files belong to their communities:
//! those in a community deleted with them go with it, the rest stay,
//! see `files_communities`. Then, outside the transaction, a Stripe plan
//! is ended at once and the customer deleted, their pictures are
//! deleted, and they are mailed; a failure there is logged and left to
//! the daily jobs. The purge removes the rows 90 days later, and the
//! sweep the bytes of the files deleted.
//!
//! A trial user needs no code, having no real email, and goes outright,
//! as their trial would. A local user has no account on the server.

#[cfg(feature = "stripe")]
use crate::data::accounts_subscriptions as accounts_subscriptions_records;
#[cfg(any(feature = "stripe", test))]
use crate::data::accounts_users as accounts_users_records;
use crate::data::memberships as memberships_records;
use crate::data::ops_staff as ops_staff_records;
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::users::{self as accounts_users, AccountUser, AccountUserError};
use crate::accounts::{AccountError, MAX_CODE_ATTEMPTS};
use crate::communities::{self, CommunityError};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::events::{Answer, AnswerStatus, Event, Fields};
use crate::mailer::{MailError, Mailer};
use crate::memberships::{self, MembershipError};
use crate::posts::{self, PostError};
use crate::users::{self, User, UserError};
use crate::{mails, tokens, trial_users};

#[derive(Debug, Error)]
pub enum DeletionError {
    #[error("only the server deletes accounts")]
    ServerOnly,
    #[error("That code is wrong or has expired; send a new one")]
    CodeDoesNotMatch,
    #[error("A trial is deleted without a code")]
    TrialHasNoCode,
    #[error("the delete was refused: {0}")]
    Refused(String),
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    AccountUser(#[from] AccountUserError),
    #[error(transparent)]
    Post(#[from] PostError),
    #[error(transparent)]
    Membership(#[from] MembershipError),
    #[error(transparent)]
    Community(#[from] CommunityError),
    #[error("Could not send the email; try again")]
    Mail(#[from] MailError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<AccountError> for DeletionError {
    fn from(error: AccountError) -> DeletionError {
        match error {
            AccountError::User(error) => error.into(),
            AccountError::AccountUser(error) => error.into(),
            AccountError::Database(error) => error.into(),
            error => DeletionError::Refused(error.to_string()),
        }
    }
}

/// Mails the signed-in full user a code to confirm deleting their
/// account, replacing any earlier one; ten minutes.
pub async fn code_create(
    deployment: &Deployment,
    mailer: &Mailer,
    user: &User,
) -> Result<(), DeletionError> {
    if deployment.role != Role::Server {
        return Err(DeletionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = live_account_in(&mut tx, user.id).await?;
    if account.trial_expires_at.is_some() {
        return Err(DeletionError::TrialHasNoCode);
    }
    let code = tokens::create_delete_code(&mut tx, account.id).await?;
    tx.commit().await?;
    mailer
        .send(mails::delete_code(&account.email, &code))
        .await?;
    Ok(())
}

/// Deletes the signed-in user's account, with the code `code_create`
/// mailed them, or none for a trial. A wrong code counts, as a
/// sign-in's does, and is committed so it stays counted.
pub async fn create(
    deployment: &Deployment,
    mailer: &Mailer,
    user: &User,
    code: Option<&str>,
) -> Result<(), DeletionError> {
    if deployment.role != Role::Server {
        return Err(DeletionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = live_account_in(&mut tx, user.id).await?;
    if account.trial_expires_at.is_some() {
        trial_users::delete_in(&mut tx, user.id).await?;
        tx.commit().await?;
        return Ok(());
    }
    if let Err(error) = match_code_in(&mut tx, user.id, code.unwrap_or_default()).await {
        tx.commit().await?;
        return Err(error);
    }
    #[cfg(feature = "stripe")]
    let stripe_left = stripe_left_in(&mut tx, user.id).await?;
    delete_in(&mut tx, user).await?;
    tx.commit().await?;

    #[cfg(feature = "stripe")]
    stripe_end(deployment, stripe_left).await;
    #[cfg(feature = "object-storage")]
    bucket_empty(deployment, user.id).await;
    if let Err(error) = mailer.send(mails::account_deleted(&account.email)).await {
        tracing::warn!("could not mail {} that it was deleted: {error}", user.id);
    }
    Ok(())
}

/// Steps 1 to 5 of the design's "What the delete does": everything of
/// the user's soft-deleted as events in their name, then the user, the
/// staff row and the account.
async fn delete_in(tx: &mut Transaction, user: &User) -> Result<(), DeletionError> {
    let role = Role::Server;
    for post in crate::data::posts::list(tx, crate::data::posts::Filter::OwnedBy(user.id)).await? {
        let event = deleted_event(posts::TABLE, post.id, post.version);
        refused_unless_applied(&posts::apply_in(tx, role, user, &event).await?.answer)?;
    }
    for membership in
        memberships_records::list(&mut *tx, memberships_records::Filter::User(user.id)).await?
    {
        let members = memberships_records::list(
            &mut *tx,
            memberships_records::Filter::Community(membership.community_id),
        )
        .await?;
        if members.iter().all(|member| member.user_id == user.id) {
            community_delete_in(tx, user, membership.community_id).await?;
        } else {
            memberships::leave_in(tx, role, user, membership.community_id).await?;
        }
    }
    users::delete_in(tx, user.id).await?;
    ops_staff_records::delete(&mut *tx, user.id).await?;
    accounts_users::delete_in(tx, user.id).await?;
    Ok(())
}

/// The community soft-deleted, which takes its memberships along.
async fn community_delete_in(
    tx: &mut Transaction,
    user: &User,
    id: Uuid,
) -> Result<(), DeletionError> {
    let Some(community) = communities::get_in(tx, id).await? else {
        return Ok(());
    };
    let event = deleted_event(communities::TABLE, id, community.version);
    let applied = communities::apply_in(tx, Role::Server, user, &event).await?;
    refused_unless_applied(&applied.answer)
}

/// The event a person deleting the row by hand would make.
fn deleted_event(table: &str, id: Uuid, version: i64) -> Event {
    let now = now();
    Event::update(
        table,
        id,
        version,
        Fields::from_iter([("deleted_at".to_string(), serde_json::Value::Null)]),
        Fields::from_iter([("deleted_at".to_string(), serde_json::json!(now))]),
        now,
    )
}

fn refused_unless_applied(answer: &Answer) -> Result<(), DeletionError> {
    if answer.status == AnswerStatus::Rejected {
        let reason = answer.reason.clone().unwrap_or_else(|| "rejected".into());
        return Err(DeletionError::Refused(reason));
    }
    Ok(())
}

/// Checks the code against the user's delete code, which then goes; a
/// wrong one counts, and the one that reaches `MAX_CODE_ATTEMPTS`
/// deletes the code.
async fn match_code_in(
    tx: &mut Transaction,
    user_id: Uuid,
    code: &str,
) -> Result<(), DeletionError> {
    let row = tokens::get_valid(
        tx,
        &tokens::delete_code_hash(user_id),
        tokens::Context::DeleteCode,
        now(),
    )
    .await?
    .ok_or(DeletionError::CodeDoesNotMatch)?;
    if row.code_hash.as_deref() != Some(tokens::hash(code.trim()).as_str()) {
        if tokens::count_attempt(tx, row.id).await? >= MAX_CODE_ATTEMPTS {
            tokens::delete_by_id(tx, row.id).await?;
        }
        return Err(DeletionError::CodeDoesNotMatch);
    }
    tokens::delete_by_id(tx, row.id).await?;
    Ok(())
}

async fn live_account_in(tx: &mut Transaction, id: Uuid) -> Result<AccountUser, DeletionError> {
    accounts_users::get_in(tx, id)
        .await?
        .ok_or_else(|| AccountUserError::NotFound { id }.into())
}

/// What is left at Stripe once the rows are deleted: the live
/// subscriptions to end, and the customer to delete.
#[cfg(feature = "stripe")]
struct StripeLeft {
    subscription_ids: Vec<String>,
    customer_id: Option<String>,
}

#[cfg(feature = "stripe")]
async fn stripe_left_in(tx: &mut Transaction, user_id: Uuid) -> Result<StripeLeft, DeletionError> {
    use crate::accounts::purchases::Store;
    use crate::accounts::subscriptions::SubscriptionStatus;

    let subscription_ids = accounts_subscriptions_records::list(&mut *tx, user_id)
        .await?
        .into_iter()
        .map(|(subscription, _product_name)| subscription)
        .filter(|subscription| {
            subscription.store == Store::Stripe
                && subscription.ended_at.is_none()
                && !matches!(
                    subscription.status,
                    SubscriptionStatus::Canceled | SubscriptionStatus::IncompleteExpired
                )
        })
        .map(|subscription| subscription.store_id)
        .collect();
    let customer_id =
        accounts_users_records::billing::get_stripe_customer_id(&mut *tx, user_id).await?;
    Ok(StripeLeft {
        subscription_ids,
        customer_id,
    })
}

/// Step 7: the Stripe plan ended now with no refund, then the customer
/// deleted; a failure is logged, and `stripe_reconcile` ends a plan
/// left live.
#[cfg(feature = "stripe")]
async fn stripe_end(deployment: &Deployment, left: StripeLeft) {
    let Some(stripe) = &deployment.stripe else {
        return;
    };
    for id in &left.subscription_ids {
        if let Err(error) = stripe.subscription_cancel(id).await {
            tracing::warn!("could not end {id} of a deleted account: {error}");
        }
    }
    if let Some(customer_id) = &left.customer_id
        && let Err(error) = stripe.customer_delete(customer_id).await
    {
        tracing::warn!("could not delete the customer {customer_id}: {error}");
    }
}

/// Step 8: their pictures deleted now; a failure is logged, and the
/// sweep takes what no live row points at.
#[cfg(feature = "object-storage")]
async fn bucket_empty(deployment: &Deployment, user_id: Uuid) {
    if let Err(error) = crate::avatars::delete_of_user(deployment, user_id).await {
        tracing::warn!("could not delete the pictures of {user_id}: {error}");
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::accounts::tests::{code_sent_to, signed_up_for_a_device};
    use crate::accounts::{self, Session};
    use crate::database::with_each_database;
    use crate::sync::{self, InProcess};
    use crate::users::tests::device_of;
    use crate::{purge, users};

    fn server(database: crate::database::Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    /// The code mailed to the address for deleting its account.
    async fn code_mailed(server: &Deployment, mailer: &Mailer, session: &Session) -> String {
        code_create(server, mailer, &session.user)
            .await
            .expect("a code mailed");
        code_sent_to(mailer, &session.email)
    }

    /// A code that is not the one given.
    fn other_than(code: &str) -> &'static str {
        if code == "000000" { "111111" } else { "000000" }
    }

    async fn bodies(deployment: &Deployment, user: &User) -> Vec<String> {
        let mut bodies: Vec<String> = posts::list(deployment, user)
            .await
            .expect("the posts")
            .into_iter()
            .map(|listed| listed.post.body)
            .collect();
        bodies.sort();
        bodies
    }

    #[tokio::test]
    async fn deletions_create_deletes_everything_of_the_user_everywhere() {
        with_each_database(|database| async move {
            let server = server(database);
            let mailer = Mailer::memory();
            let alice = signed_up_for_a_device(&server, "alice@example.com").await;
            let bob = signed_up_for_a_device(&server, "bob@example.com").await;
            let shared = communities::create(&server, &alice.user, "Shared")
                .await
                .expect("a community");
            memberships::join(&server, &bob.user, shared.id)
                .await
                .expect("joined");
            posts::create(&server, &alice.user, "alice's", &[shared.id])
                .await
                .expect("a post");
            posts::create(&server, &bob.user, "bob's", &[shared.id])
                .await
                .expect("a post");
            let personal = memberships::list_mine(&server, &alice.user)
                .await
                .expect("the memberships")
                .into_iter()
                .map(|membership| membership.community_id)
                .find(|id| *id != shared.id)
                .expect("alice's own community");

            let bobs_device = device_of(&bob.user, &bob.token).await;
            sync::run(&bobs_device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(bodies(&bobs_device, &bob.user).await, ["alice's", "bob's"]);

            let code = code_mailed(&server, &mailer, &alice).await;
            create(&server, &mailer, &alice.user, Some(&code))
                .await
                .expect("deleted");

            assert_eq!(bodies(&server, &bob.user).await, ["bob's"]);
            communities::get(&server, &bob.user, shared.id)
                .await
                .expect("a shared community stays");
            let mut tx = server.begin().await.expect("a transaction");
            let personal = communities::get_in(&mut tx, personal)
                .await
                .expect("read")
                .expect("kept until the purge");
            let deleted_user = users::get_in(&mut tx, alice.user.id)
                .await
                .expect("read")
                .expect("kept until the purge");
            tx.commit().await.expect("committed");
            assert!(personal.deleted_at.is_some(), "her own community goes");
            assert!(deleted_user.deleted_at.is_some());
            assert_eq!(deleted_user.name, users::DELETED_NAME);

            assert!(matches!(
                users::get_by_token(&server, &alice.token).await,
                Err(UserError::AccountDeleted)
            ));
            let mail = mailer
                .last_sent_to("alice@example.com")
                .expect("a mail after");
            assert!(mail.subject.contains("deleted"));

            sync::run(&bobs_device, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(bodies(&bobs_device, &bob.user).await, ["bob's"]);

            let alices_device = device_of(&alice.user, &alice.token).await;
            let error = sync::run(&alices_device, &InProcess { server: &server })
                .await
                .expect_err("refused");
            assert!(matches!(error, sync::SyncError::AccountDeleted));

            accounts::sign_in(&server, &mailer, "alice@example.com")
                .await
                .expect("answered");
            let mail = mailer.last_sent_to("alice@example.com").expect("a mail");
            assert!(mail.subject.contains("No "), "no account to sign in to");
            let again = signed_up_for_a_device(&server, "alice@example.com").await;
            assert_ne!(again.user.id, alice.user.id, "a new account");
        })
        .await;
    }

    #[cfg(feature = "object-storage")]
    #[tokio::test]
    async fn deletions_create_leaves_the_files_of_communities_with_other_members() {
        use crate::files::tests::{new_file, server as server_with_bucket};
        use crate::files::{self, FileNew};

        with_each_database(|database| async move {
            let (server, storage) = server_with_bucket(database);
            let mailer = Mailer::memory();
            let alice = signed_up_for_a_device(&server, "alice@example.com").await;
            let bob = signed_up_for_a_device(&server, "bob@example.com").await;
            let shared = communities::create(&server, &alice.user, "Shared")
                .await
                .expect("a community");
            memberships::join(&server, &bob.user, shared.id)
                .await
                .expect("joined");
            let personal = memberships::list_mine(&server, &alice.user)
                .await
                .expect("the memberships")
                .into_iter()
                .map(|membership| membership.community_id)
                .find(|id| *id != shared.id)
                .expect("alice's own community");
            let mut created = Vec::new();
            for (name, community_id) in [("shared.txt", shared.id), ("private.txt", personal)] {
                let new = FileNew {
                    community_ids: vec![community_id],
                    ..new_file(name, b"bytes", community_id)
                };
                storage
                    .put(
                        &files::storage_key(alice.user.id, new.id),
                        b"bytes".to_vec(),
                    )
                    .await
                    .expect("the upload");
                created.push(
                    files::create(&server, &alice.user, &new)
                        .await
                        .expect("a file"),
                );
            }

            let code = code_mailed(&server, &mailer, &alice).await;
            create(&server, &mailer, &alice.user, Some(&code))
                .await
                .expect("deleted");

            let shown = files::get(&server, &bob.user, created[0].id)
                .await
                .expect("the community keeps it");
            assert_eq!(shown.creator, None);
            let mut tx = server.begin().await.expect("a transaction");
            let private = files::get_in(&mut tx, created[1].id)
                .await
                .expect("read")
                .expect("kept until the purge");
            tx.commit().await.expect("committed");
            assert!(private.deleted_at.is_some(), "it went with her community");
            for file in &created {
                assert!(
                    storage
                        .size(&files::storage_key(alice.user.id, file.id))
                        .await
                        .expect("looked")
                        .is_some(),
                    "the bytes go by the sweep, not by person"
                );
            }
        })
        .await;
    }

    #[tokio::test]
    async fn deletions_create_refuses_a_wrong_or_used_code() {
        with_each_database(|database| async move {
            let server = server(database);
            let mailer = Mailer::memory();
            let alice = signed_up_for_a_device(&server, "alice@example.com").await;

            let error = create(&server, &mailer, &alice.user, Some("123456"))
                .await
                .expect_err("no code asked for");
            assert!(matches!(error, DeletionError::CodeDoesNotMatch));

            let first = code_mailed(&server, &mailer, &alice).await;
            let second = code_mailed(&server, &mailer, &alice).await;
            if first != second {
                let error = create(&server, &mailer, &alice.user, Some(&first))
                    .await
                    .expect_err("replaced by the second");
                assert!(matches!(error, DeletionError::CodeDoesNotMatch));
            }
            let error = create(&server, &mailer, &alice.user, Some(other_than(&second)))
                .await
                .expect_err("wrong");
            assert!(matches!(error, DeletionError::CodeDoesNotMatch));
            users::get_by_token(&server, &alice.token)
                .await
                .expect("still there");

            for _ in 1..MAX_CODE_ATTEMPTS {
                let _ = create(&server, &mailer, &alice.user, Some(other_than(&second))).await;
            }
            let error = create(&server, &mailer, &alice.user, Some(&second))
                .await
                .expect_err("gone after too many wrong ones");
            assert!(matches!(error, DeletionError::CodeDoesNotMatch));
            users::get_by_token(&server, &alice.token)
                .await
                .expect("still there");
        })
        .await;
    }

    #[tokio::test]
    async fn deletions_create_of_a_trial_removes_it_outright_without_a_code() {
        with_each_database(|database| async move {
            let server = server(database);
            let mailer = Mailer::memory();
            let trial = trial_users::create(&server, 10).await.expect("a trial");
            let error = code_create(&server, &mailer, &trial.user)
                .await
                .expect_err("no code for a trial");
            assert!(matches!(error, DeletionError::TrialHasNoCode));

            create(&server, &mailer, &trial.user, None)
                .await
                .expect("deleted");
            assert!(matches!(
                users::get_by_token(&server, &trial.token).await,
                Err(UserError::UnknownToken)
            ));
            assert_eq!(trial_users::count(&server).await.expect("counted"), 0);
            assert!(mailer.sent().is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn deletions_create_then_the_purge_removes_the_user_and_account_after_90_days() {
        with_each_database(|database| async move {
            let server = server(database);
            let mailer = Mailer::memory();
            let alice = signed_up_for_a_device(&server, "alice@example.com").await;
            posts::create(&server, &alice.user, "alice's", &[])
                .await
                .expect("a post");
            let code = code_mailed(&server, &mailer, &alice).await;
            create(&server, &mailer, &alice.user, Some(&code))
                .await
                .expect("deleted");

            purge::run(&server, now() + Duration::days(89))
                .await
                .expect("purged");
            let mut tx = server.begin().await.expect("a transaction");
            assert!(
                users::get_in(&mut tx, alice.user.id)
                    .await
                    .expect("read")
                    .is_some()
            );
            tx.commit().await.expect("committed");

            purge::run(&server, now() + Duration::days(91))
                .await
                .expect("purged");
            let mut tx = server.begin().await.expect("a transaction");
            assert!(
                users::get_in(&mut tx, alice.user.id)
                    .await
                    .expect("read")
                    .is_none()
            );
            assert!(
                accounts_users_records::lifecycle::get(&mut tx, alice.user.id)
                    .await
                    .expect("read")
                    .is_none(),
                "the account goes with the user"
            );
            tx.commit().await.expect("committed");
        })
        .await;
    }

    /// Alice on a monthly Stripe plan bought through a checkout, as
    /// `stripe_reconcile`'s test does: her Stripe customer's id.
    #[cfg(feature = "stripe")]
    async fn on_a_stripe_plan(
        server: &Deployment,
        stripe: &crate::accounts::stripe::Stripe,
        alice: &User,
    ) -> String {
        use crate::accounts::stripe::{
            Stripe, StripeCheckoutSession, StripeCheckoutSessionStatus, StripePaymentStatus,
            StripePrice, StripeProduct, StripeSubscription,
        };
        use crate::accounts::{checkouts, online, plans, products, stripe_reconcile};

        stripe.memory_product_put(StripeProduct {
            id: "prod_single".to_string(),
            name: "Single user subscription".to_string(),
            description: None,
            active: true,
        });
        stripe.memory_price_put(StripePrice {
            id: "price_monthly".to_string(),
            product_id: "prod_single".to_string(),
            lookup_key: Some("single_user_monthly".to_string()),
            amount: Some(900),
            currency: "usd".to_string(),
            interval: Some("month".to_string()),
            active: true,
        });
        products::sync(server, stripe).await.expect("the catalog");
        let monthly = products::list(server).await.expect("the catalog")[0].prices[0].id;
        let created = checkouts::create(
            server,
            stripe,
            alice,
            monthly,
            "http://server.test",
            online::ReturnTo::Web,
        )
        .await
        .expect("a checkout");
        let customer_id = match stripe {
            Stripe::Memory(memory) => memory.lock().expect("the memory").checkout_sessions_asked[0]
                .customer_id
                .clone(),
            Stripe::Api(_) => unreachable!("tests use memory"),
        };
        stripe.memory_subscription_put(StripeSubscription {
            id: "sub_1".to_string(),
            customer_id: customer_id.clone(),
            item_id: "si_1".to_string(),
            price_id: "price_monthly".to_string(),
            status: "active".to_string(),
            current_period_end: Some(now().timestamp() + 30 * 24 * 60 * 60),
            cancel_at_period_end: false,
            ended_at: None,
            schedule_id: None,
            scheduled_price_id: None,
            scheduled_at: None,
        });
        stripe.memory_checkout_session_put(StripeCheckoutSession {
            id: created.checkout.stripe_session_id.clone(),
            url: None,
            client_reference_id: Some(created.checkout.id.to_string()),
            status: StripeCheckoutSessionStatus::Complete,
            payment_status: StripePaymentStatus::Paid,
            payment_intent_id: None,
            subscription_id: Some("sub_1".to_string()),
            amount_total: Some(900),
            currency: Some("usd".to_string()),
        });
        stripe_reconcile::run(server, stripe)
            .await
            .expect("reconciled");
        assert!(plans::current(server, alice).await.expect("read").is_some());
        customer_id
    }

    #[cfg(feature = "stripe")]
    fn stripe_state(stripe: &crate::accounts::stripe::Stripe) -> (String, usize) {
        match stripe {
            crate::accounts::stripe::Stripe::Memory(memory) => {
                let memory = memory.lock().expect("the memory");
                (
                    memory.subscriptions["sub_1"].status.clone(),
                    memory.customers.len(),
                )
            }
            crate::accounts::stripe::Stripe::Api(_) => unreachable!("tests use memory"),
        }
    }

    #[cfg(feature = "stripe")]
    #[tokio::test]
    async fn deletions_create_ends_a_stripe_plan_and_deletes_the_customer() {
        with_each_database(|database| async move {
            let stripe = crate::accounts::stripe::Stripe::memory();
            let server = server(database).with_stripe(stripe.clone());
            let mailer = Mailer::memory();
            let alice = signed_up_for_a_device(&server, "alice@example.com").await;
            on_a_stripe_plan(&server, &stripe, &alice.user).await;
            assert_eq!(stripe_state(&stripe), ("active".to_string(), 1));

            let code = code_mailed(&server, &mailer, &alice).await;
            create(&server, &mailer, &alice.user, Some(&code))
                .await
                .expect("deleted");
            assert_eq!(stripe_state(&stripe), ("canceled".to_string(), 0));
        })
        .await;
    }

    #[cfg(feature = "stripe")]
    #[tokio::test]
    async fn deletions_create_leaves_a_plan_stripe_missed_to_the_reconcile() {
        with_each_database(|database| async move {
            let stripe = crate::accounts::stripe::Stripe::memory();
            // No Stripe on the deployment: as if the call after the commit failed
            let server = server(database);
            let mailer = Mailer::memory();
            let alice = signed_up_for_a_device(&server, "alice@example.com").await;
            on_a_stripe_plan(&server, &stripe, &alice.user).await;

            let code = code_mailed(&server, &mailer, &alice).await;
            create(&server, &mailer, &alice.user, Some(&code))
                .await
                .expect("deleted");
            assert_eq!(stripe_state(&stripe).0, "active");

            crate::accounts::stripe_reconcile::run(&server, &stripe)
                .await
                .expect("reconciled");
            assert_eq!(stripe_state(&stripe).0, "canceled");
        })
        .await;
    }
}
