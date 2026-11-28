//! `app_web sync feed renew`: the sync feed's new id, after the database
//! is restored from a backup, see `app_core::sync::feed_id_renew`.

use app_core::deployment::Deployment;
use app_core::sync;

/// `renew`.
pub async fn run(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
    match words {
        ["renew"] => {
            let feed_id = sync::feed_id_renew(deployment)
                .await
                .map_err(|error| error.to_string())?;
            println!("the feed is {feed_id}; every device resyncs at its next pull");
            Ok(())
        }
        _ => Err("commands: sync feed renew".to_string()),
    }
}
