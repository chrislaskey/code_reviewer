//! The app's own state the web server keeps while it is up, such as a
//! cache its routes share: one `ServerState`, made with `default()` when
//! the server starts, which a handler takes as `State<Arc<ServerState>>`.
//! Every app edits it to add its own; the template keeps none.

/// What the app keeps on the server between requests.
#[derive(Debug, Default)]
pub struct ServerState {}
