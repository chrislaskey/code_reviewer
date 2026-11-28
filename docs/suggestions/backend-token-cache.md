# Cache token lookups in remote mode

**Suggestion, deferred on 2026-11-27.** See
[the backend seam](../design/backend-seam.md).

In remote mode, every signed-in request asks the backend who its token
belongs to. That is one extra call to the backend per request. Start
without a cache. Add one if the measured latency matters.

The cache would sit in `ServerState`, keyed by the token's hash. Each
entry would last a few seconds.

What the cache must not break:

- **Sign-out.** A token signed out must stop working. Either sign-out
  through Axum drops the entry, or entries are short enough to wait out.
- **Account deletion and a trial's expiry.** A deleted user must not keep
  access for the length of an entry. The backend could tell Axum, or
  these paths could drop the entry, as sign-out does.
- **A user's `kind` changing**, such as a trial signing up. The cached
  user must not keep the old kind past the change.
- **Several Axum servers.** Each one has its own cache. So dropping an
  entry on one server doesn't reach the others. Short entries cover this.
