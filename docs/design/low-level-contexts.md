# Low-level Rust contexts: analysis and refactor proposal

Status: superseded in layout by `code-organization.md` (2026-11-21), which moved each schema and its validation into its `data` module, named after its table; the bulk exceptions below still hold. Earlier status: storage-context extraction implemented across the existing resources, infrastructure and documented bulk lifecycle operations. PostgreSQL runtime parity and upstream dependency warnings remain recorded validation limits. The known SQLite advisory is being retained while waiting for a published upstream fix, by the project owner's decision; see `dependency-security.md`.

## Original request

> In the V2 of this project, which you can still find inside of a git branch, we built Elixir modules called contexts that contained the basic low-level functions that you could do in order to change records in the database using elixir ecto schemas. The git branch is `origin/v2-elixir-and-rust`. I like to refer to these files as low-level contexts. They kind of work as the universal entry point for reading and writing. Records. The idea is anything in the application that needs to touch these records in the database go through these functions rather than calling records directly. Well there might be a few extreme examples and exceptions, usually around operational dashboards, everything for most of the core business logic should be using them. By using this pattern, what it lets us do is have application logic That we can reliably count on being triggered each time things change. For example, telemetry data, logging, event publishing, etc. We can always build separate modules that I often call high-level contexts that have more cross-cutting business logic than includes you know third-party side effects, manipulation of multiple resources, etc. Those high-level contacts may call one or many low-level contacts. If you look in the Elixir implementation you'll see that these low-level contexts are restricted to only having the same core functions for each module. They are very concrete term functions like list, get, create, update, delete, as well as sort And filter functions. I'm very familiar with Elixir and feel that pattern is strong and works there. I am not as familiar with Rust, but I do think a similar pattern would be useful to set as a baseline for this application.The goal of this work next is to analyze what code has been written inside of the Rust application for existing database entries, Confirm a pattern like the one I described would work well for this application, and that there is a path to refactor towards it, and then finally a plan to refactor the existing resources towards this low-level context pattern. Let's save this prompt as well as additional summary in a markdown file and then within that markdown file start doing the analysis recommendation and then implementation path for converting over to a pattern like this. Elixir is obviously a different language and has a different implementation path. In Elixir, we use behaviors and macros to make it reusable. In Rust, we may have a different way to implement this. It doesn't have to be exactly the same. We want to write idiomatic Rust that's clear, concise, and easy to maintain, that makes it easy for anyone reading the Rust code to understand what's going on. Once this analysis is complete, we'll pass it over to a different model, likely Opus to implement it. But for now, let's do the first level analysis and markdown file creation we discuss together using this chat. Thanks.

## Summary and recommendation

Adopt this pattern. Define one small record-access module per resource, with concrete operations and typed inputs. High-level contexts compose these operations within a shared transaction. Keep SQLx and the existing SQLite/PostgreSQL adapters; no ORM replacement, generic repository framework, new crate, or schema migration is needed for the initial refactor.

The principal benefit is an explicit place for record invariants and observation of changes. Merely renaming the existing SQL adapters would not achieve that: workflows currently call adapters directly, and changes have different meanings in local writes, server event application, replication, and maintenance.

Use ordinary Rust modules and functions first. Uniform names are a convention, not a reason to implement a trait for unrelated resources. A trait becomes useful when an actual caller must operate generically over implementations; that need has not been established here. Keep small private helpers: translating the Elixir ban on private functions literally would reduce Rust readability.

## Evidence and scope

Reviewed current checkout at `277fb493e31b4dc380a4f554f62690731cb0e317` and the locally available remote-tracking V2 branch at `00ff66ba14ffaa560a37d12d56557c835727221d`. The branch was inspected without checking it out or fetching. This is a structural inventory with representative implementation reads, not an exhaustive correctness audit of every SQL statement.

Historical evidence (paths relative to that branch):

- `phoenix/lib/app/data/context.ex`: behavior and macro provide list/get/fetch, create/update/delete, get-or-create/create-or-update variants, joins, filters, sorts and changesets. Reads exclude soft-deleted records by default; privileged options include deleted records or hard-delete. Versioned updates use optimistic locking.
- `phoenix/lib/app/data/schemas/posts/posts.ex`: a narrow context adds visibility filters; its documentation assigns writing workflows to `App.Data.Posting`.
- `phoenix/priv/checks/data_context_only_behaviour_functions.rs`: architectural check permits only overridable behavior functions and forbids additional private functions too. The boundary is explicitly enforced, not simply a naming convention.

Current evidence:

- [database.rs](../../core/src/database.rs): `Transaction` abstracts both backends; `sql!` dispatches on its connection. This already supports composition without backend-generic business code.
- [posts.rs](../../core/src/app/posts.rs): combines row types, validation, visible reads, owner/community enrichment, event application, community changes, replication, and pruning. `create` applies a post event and sets community associations in one transaction. Splitting that transaction would be a regression.
- [sqlite/posts.rs](../../core/src/sqlite/posts.rs): concrete SQL operations include visible lists, unrestricted get, upsert and deletion of unreachable rows. Default adapter `get` includes deleted rows. Exposing it as the ordinary context get would change the intended safety boundary.
- [memberships.rs](../../core/src/app/memberships.rs) and [posts_communities.rs](../../core/src/app/posts_communities.rs): domain actions and event processing; relationship identities and revival must survive the refactor.
- [users.rs](../../core/src/app/users.rs): profile changes, account-related helpers, token lookup, anonymization and replication coexist.
- [accounts/products.rs](../../core/src/accounts/products.rs): catalog persistence, sale projections and Stripe synchronization coexist.
- [accounts/deletions.rs](../../core/src/accounts/deletions.rs): orchestrates several resources and directly calls adapters for posts, files, memberships, staff and account data. [accounts/stripe_reconcile.rs](../../core/src/accounts/stripe_reconcile.rs) also directly queries adapters.
- [local_users.rs](../../core/src/app/local_users.rs): ownership transfer and cleanup include bulk SQL; [purge.rs](../../core/src/app/purge.rs) uses cross-table maintenance operations. These need explicit treatment, not a claim that ordinary CRUD already covers every write.
- [lib.rs](../../core/src/lib.rs) keeps backend modules private to the crate, but sibling core modules can still use them. Privacy currently prevents external access, not internal bypasses.

Design constraints: [offline sync](offline-sync.md) and [multi-app](multi-app.md). Synced business rows use events; accounts writes do not universally use those events. Infrastructure tables with an `app_` prefix are not ordinary replicated business rows. Preserve these distinctions.

## Resource inventory and migration scope

The paired backend module inventory gives the following initial ownership map. Reconcile it against migrations and every production `sql!` call before implementation; some modules span tables and some resources share an adapter.

| Group | Existing resources/modules | Destination and concerns |
| --- | --- | --- |
| Synced business records | users, posts, communities, memberships, posts_communities, files | One context per resource; preserve authorization, soft deletion, versions, visibility and event handling |
| Accounts identity | accounts_users, tokens, credentials | Account record contexts; preserve hashing, expiry, confirmation and two-row signup atomicity |
| Commerce | accounts_products, accounts_prices, accounts_checkouts, accounts_subscriptions, accounts_purchases | Record contexts separate from catalog projections, payments and reconciliation |
| Provider receipts | accounts_stripe_events, accounts_app_store_notifications; store_transactions orchestration | Preserve receipt uniqueness, retry behavior and provider identifiers; determine actual table ownership before extraction |
| Operations | ops_staff | Staff record context; preserve access checks and CLI semantics |
| Infrastructure | events, jobs, purges, sync_state, sync_backfills, device_settings | Separate record storage from processing; retain atomic claims, queue state transitions and cursor semantics |
| Cross-resource SQL | local_users, trial_users, purge/deletions | Move ordinary access through owning contexts; enumerate justified bulk maintenance exceptions |

High-level contexts include signup/signin, account deletion, posting with community selection, joining/leaving communities, local-account conversion, purchase reconciliation, uploads, and synchronization. Preserve existing public APIs initially so Axum, Tauri and Svelte do not require a coordinated migration.

## Proposed boundary

Suggested internal layout: `core/src/data/posts.rs`, `data/communities.rs`, `data/accounts/products.rs`, and corresponding resources. `data` is crate-private. Existing public modules initially remain workflow facades. Row types can move to their owning data modules with compatibility re-exports; avoid duplicating models. Pure validation may live beside the resource.

Normal low-level operations: `list`, `get`, `create`, `update`, `delete`; add `upsert` only where its conflict semantics are justified. Implement only supported operations, not meaningless CRUD for every table. Use `Filter`, `Sort`, `ListOptions`, `Create`, and `Update` types as needed. Filter and sort helpers build bound SQL through the adapters; do not expose arbitrary column names or caller-supplied SQL. Lists should have deterministic ordering, including an ID tie-breaker. Preserve current pagination behavior in the extraction; do not silently impose new limits.

Prefer `Result<Option<Record>, ResourceError>` for low-level get; a workflow can translate absence to its existing not-found error. Preserve existing external error responses. Ordinary reads exclude deleted rows and require appropriate access scope. An unrestricted read for sync or maintenance must be separately restricted and explicit, never an omitted optional user filter.

Low-level functions borrow `&mut Transaction`; they do not begin or commit transactions. Pass actor/role information explicitly when required. Public application operations continue accepting `Deployment`, opening the transaction and committing once. Update the architectural wording that says every core function accepts Deployment: it applies to application entry points, not composable internal record operations.

For synced writes, separate intent from persistence:

1. A workflow constructs the existing event and calls its resource event handler.
2. The handler validates authority, editable fields and merge/version rules, then calls the resource data context with the accepted change.
3. The data context persists the row and applies universal record invariants/transaction-local observation.
4. The handler records the event answer or device queue entry in the same transaction.
5. The application entry point commits once after all resource work succeeds.

Do not have data CRUD construct another event when called by an event handler: that would recurse or double-record. Restrict write inputs so arbitrary sibling workflows cannot accidentally submit unchecked synced changes. Prefer a small accepted-change type whose fields/construction are private to the resource implementation; settle its exact module visibility in the pilot. Do not introduce a broad public `skip_validation` flag.

Replication must persist authoritative server versions without issuing a new local command. Keep protocol operations in a restricted sync-facing module which uses the same record persistence boundary with explicit origin. Maintenance needs similarly explicit hard-delete behavior. These are restricted entry points, not additions to the ordinary CRUD vocabulary.

Cross-resource joins remain valid SQL. Resource ownership does not imply loading entire tables into Rust or one query per record. A context may read related tables to implement visibility. Writes to other resources belong in orchestration or narrowly documented atomic maintenance operations.

## Reliable hooks: define what is guaranteed

A context guarantees interception only for paths actually routed through it. Audit event application, replication, bulk writes, cascading deletes, cleanup and jobs before claiming complete coverage. Database cascades do not invoke Rust functions for each affected row.

Distinguish attempted operations, accepted row changes and committed business changes. Logging before commit may describe work later rolled back. Replication should not send another customer email or duplicate a business event; event replay should not republish a completed action. No-op updates and conflict-only answers need defined behavior.

Keep transaction-local invariants and existing event persistence synchronous with the write. External side effects belong after commit in a workflow. Durable external delivery requires persisted intent and retry/idempotency handling; an in-memory post-commit callback alone can lose delivery on process failure. Do not assume the existing sync event log is a general-purpose external outbox. Adding durable publishing infrastructure is a separate design/schema decision requiring approval, not part of this extraction.

Initially preserve existing event behavior and establish explicit observation points. Do not add a generic callback registry, telemetry product, or third-party integration during the refactor. Never log credentials, tokens, provider payloads or unrestricted record bodies by default.

## Incremental implementation plan

1. **Baseline and inventory.** Enumerate migrations, all production adapter/SQL call sites, transaction owners, cascade effects and current tests. Separate test fixture SQL from production bypasses. Record exceptions by path, operation and rationale. Capture baseline verification failures.
2. **Posts pilot.** Extract row access and typed read scope into `data::posts`; keep current public functions and return types. Route event persistence and server-row replication through the boundary. Preserve post/community atomicity. Prove ordinary reads exclude tombstones and enforce visibility. Keep the existing event handler authoritative.
3. **Complete the related synced resources.** Convert communities, memberships and post-community associations together where needed, then users and files. Preserve association revival, delete propagation, pending-write protection and server-only fields. Move projections and multi-resource behavior into workflows without changing public transport contracts.
4. **Identity and lifecycle.** Convert accounts users, tokens, credentials and staff. Migrate signup, trial promotion, local conversion and deletion callers. Audit bulk ownership changes and cleanup separately; keep every existing transaction boundary unless a separately reviewed fix is necessary.
5. **Commerce.** Extract product/price storage first, then checkouts, subscriptions, purchases and provider receipts. Keep Stripe/App Store calls in workflows. Preserve atomic receipt deduplication and domain updates; preserve reconciliation and refund semantics.
6. **Infrastructure and maintenance.** Separate event/job/settings/cursor record operations from processors. Preserve job claims as atomic SQL operations, never read-then-write CRUD. Route purge and bulk lifecycle work through owners where practical; retain a small documented exception set when SQL atomicity or performance warrants it.
7. **Enforce and finish.** Restrict backend access as far as Rust module privacy permits; relocate adapter modules beneath the data boundary if needed. Enforce remaining internal restrictions using the project's existing check infrastructure where suitable. A trait cannot prohibit extra functions or direct SQL. Avoid building a custom linter without the approval required by AGENTS.md. Update architecture documentation and remove temporary shims only when callers have migrated.

Each step should be independently reviewable and preserve runtime behavior. Do not replace all resource modules in one patch. No new schema or dependencies should be necessary for the pilot.

## Acceptance criteria and handoff

- Every converted production read/write uses its resource context or appears in an explicit, justified exception list.
- Existing HTTP/Tauri payloads, errors and Svelte call sites remain compatible.
- Meaningful tests cover rollback of combined post/community writes; inaccessible/deleted reads; create/update/delete on server and device; replay/idempotency; merge conflicts; replication with pending writes; association revival; cascades/pruning; and no duplicate queue/log entries.
- Later phases retain account lifecycle and payment receipt/retry tests, plus atomic job-claim tests. Add tests for actual uncovered behavior, not wrapper delegation.
- Run the required format, Clippy, workspace test and audit commands. Also compile/test with the core `postgres` feature and a configured `TEST_POSTGRES_URL` for backend parity. The helper in database.rs skips PostgreSQL when that URL is absent; a green default run does not prove parity.
- Before implementation, agree on transaction borrowing, restricted replication/maintenance access, and the distinction between row observation and committed business publication. The recommendation above is the proposed baseline, not a claim those decisions are already approved.

The implementing model should start with the inventory and posts pilot, report concrete API/module changes, and preserve existing behavior before expanding. This document deliberately leaves exact accepted-change type signatures to the pilot, where Rust privacy constraints and the existing event call graph can be checked in compiling code.

## Verification for this analysis-only change

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --all-targets`: passed on the approved rerun outside the sandbox (194 core, 31 Tauri, 45 web tests passed; 2 core HTTP integration tests ignored). The initial sandbox run failed four macOS clipboard tests because NSPasteboard returned NULL. PostgreSQL parity was not separately verified.
- `cargo audit`: failed on existing `libsqlite3-sys 0.37.0`, reported as RUSTSEC-2026-0459, with an advised upgrade to >=0.39.0; also reported five allowed warnings. Dependency remediation is outside this documentation task.

Only this Markdown file was added; application code and dependencies were not changed.

## Posts pilot implementation

The first increment adds `core/src/data/posts/` with private SQLite/PostgreSQL adapters. Ordinary `list` requires `Filter::VisibleTo` or `Filter::OwnedBy`; `get` requires a user ID and excludes tombstones. Existing sort order and public APIs are preserved. Event processing remains in `posts.rs`; only that module can construct `AcceptedPost`, required by create/update/replication persistence. The accepted row is borrowed, so it cannot be modified between acceptance and persistence through that value.

Protocol reads are explicitly under `replication`; physical device cleanup is under `maintenance`. Those internal read/cleanup entry points remain crate-visible for association and sync workflows. This is a deliberate restricted-by-convention boundary for the pilot; private backend modules enforce the lower SQL boundary. No generic framework or custom linter was introduced.

Account deletion, local conversion and post-community event handling now read through the posts context. Row types remain in `posts.rs` to preserve imports. Workflows still own transaction begin/commit, including the combined post/community operation. Existing event recording, merge checks, version assignment, pending protection and cascading soft deletion remain in those workflows. Future observation hooks belong in the context write functions, with committed publication semantics still requiring separate design.

### Posts exceptions retained for lifecycle phases

| Adapter paths (SQLite and PostgreSQL) | Operation | Reason retained |
| --- | --- | --- |
| `local_users.rs` | Bulk ownership transfer, tombstone cleanup, item count, user deletion cascades | Atomic conversion of a local database; migrate with identity/lifecycle phase |
| `trial_users.rs` | Trial deletion and expiration via parent deletion | Multi-resource cleanup and foreign-key cascades; migrate with lifecycle phase |
| `purges.rs` | Retention hard deletion of posts | Cross-resource retention job; migrate with maintenance phase |
| `events.rs` | Missing-row cleanup referencing posts | Queue/log housekeeping; migrate with infrastructure phase |
| Other resource SQL | Visibility joins referencing posts | Read predicates owned by the querying resource; joins are permitted |
| Migrations and test fixture SQL | Schema evolution and deliberate state setup | Outside production record operations |

`app_posts_communities.post_id` cascades on physical post deletion. User deletion also cascades through owned posts. These database effects do not invoke Rust per-row hooks. The pilot therefore does not claim interception of all lifecycle changes.

### Remaining production adapter callers

No shared SQL-dispatch calls remain. All production record SQL in `core/` is inside private adapters beneath `core/src/data/`. Backend root modules hold only migration runners. Tests retain deliberate fixture SQL; database setup retains connection/migration code and test-schema setup. The final increment below records the remaining semantic exceptions to per-resource writes.

### Pilot verification

- Format check and workspace Clippy with warnings denied passed.
- Workspace tests passed: 196 core, 31 Tauri, 45 web; two core HTTP integration tests remain ignored. Tests ran outside the sandbox for macOS pasteboard access.
- `cargo check -p app_core --features postgres` passed. `cargo test -p app_core --features postgres posts::tests` passed all 15 posts tests; this narrower feature combination emitted an existing unused `open_from_env` test-helper warning. `TEST_POSTGRES_URL` was absent, so runtime PostgreSQL coverage remains outstanding.
- Security audit still fails on the previously recorded `libsqlite3-sys` advisory, with five allowed warnings. No dependency changes were made.
- No direct `sql!(..., posts::...)` calls remain. Both backend posts modules are private children of the context. Remaining bulk lifecycle exceptions are enumerated above.

The posts pilot established the convention extended in the increment below.

## Communities and relationship contexts implemented

Added `data::communities`, `data::memberships`, and `data::posts_communities`, each owning private, unchanged SQLite/PostgreSQL adapters. Adapter modules are no longer exposed through the backend roots. Existing public resource functions, wire data, event application, merge/version logic, pending-write protection, and transaction ownership are preserved.

- Communities: ordinary list/get require the acting user; get now centralizes the existing trial-isolation check and excludes tombstones. Public community discovery remains available to nonmembers as before. Membership checks for writes remain in workflows.
- Memberships: ordinary get excludes tombstones; list requires a user or community relationship filter. Event join/rejoin explicitly uses the protocol pair lookup, preserving row identity and revival semantics.
- Post-community associations: ordinary lists require post IDs or a membership ID and exclude tombstones. These are internal workflow scopes, not substitutes for authorization. Protocol operations explicitly support pair lookup, rows deleted at a membership's leave time, sync cursor pages and authorized backfill.
- Accepted-write values have private constructors in their owning resource workflows. Create/update and authoritative upsert require those values. Soft deletion, propagation and revival continue to update rows through the same context persistence functions; they do not issue duplicate events.
- Account deletion, local conversion, current-community selection, association handling and test reads now call these contexts. All old `sql!(..., communities::...)`, `memberships::...`, and `posts_communities::...` calls were removed.

Bulk lifecycle exceptions from the posts pilot still apply to these resources: local-account ownership transfer and tombstone cleanup, trial cleanup, retention purge, sync-state reset, parent foreign-key cascades and SQL visibility joins. They remain owned by lifecycle/infrastructure adapters until their planned phases. No per-row hook guarantee is claimed for those bulk or cascading operations.

Added behavioral tests for rollback of community creation with its membership and event records, and for deleted community/membership/association rows being absent from ordinary reads while retained by protocol reads. Existing tests continue covering join/leave/rejoin, explicit association removal/revival, post deletion during a leave, community deletion propagation, replay and multi-device sync.

The users and files increment below completes the ordinary synced business-resource contexts before the identity/lifecycle phase.

### Relationship-context verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and whitespace checks passed.
- `cargo test --workspace --all-targets` passed: 198 core, 31 Tauri and 45 web tests; two HTTP integration tests remain ignored.
- `cargo test -p app_core --features postgres,sync-http --lib` passed: 123 tests, two ignored. `TEST_POSTGRES_URL` is absent, so the feature build is verified but runtime PostgreSQL parity remains outstanding.
- All six moved adapters match their original SQL exactly. No direct calls to their former backend paths remain.
- `cargo audit` still reports the previously recorded `libsqlite3-sys` vulnerability and five allowed warnings. Dependencies and schemas were not changed.

## Users and files contexts implemented

Added `data::users` and `data::files`, each owning private SQLite/PostgreSQL adapters with unchanged SQL. Their backend-root modules were removed. Row types, application entry points, wire data, upload/storage side effects, event handling and transaction ownership remain in their existing resource workflows.

- User ordinary get excludes deleted profiles. Its existing ID-based semantics are preserved: this extraction does not add a new visibility requirement to profile lookup. Name validation, ownership checks, avatar restrictions, versions and event creation remain in the users workflow.
- User protocol get/list retain tombstones. Batch profile lookup deliberately continues to return anonymized deleted users for projections. Authentication has its own token-hash lookup: it retains tombstones so `get_by_token` can distinguish `AccountDeleted` from `UnknownToken`. Hashing and expiry policy remain outside persistence.
- User lifecycle creation, name/avatar updates and anonymization use accepted-write values constructible only in `users.rs`. Soft delete uses the existing anonymization SQL through context `delete`. Authoritative upsert remains available through the existing `users::upsert_in` workflow for signin, local users and avatar responses; its callers remain trusted internal lifecycle/replication paths. This extraction does not turn that internal helper into an authorization boundary.
- File ordinary list/get require the owner ID and exclude deleted rows. Raw event/replication reads retain tombstones. Upload checks, immutable metadata, pending-create handling, ownership, versions and merge behavior remain in the file workflow. Create/update/authoritative upsert require a privately constructed `AcceptedFile`.
- File cleanup explicitly lists IDs, owners and deletion state through `maintenance::list_ids`, including tombstones. This preserves the differing server/device byte-retention rules. Refused or forgotten local row deletion uses `maintenance::delete`; user soft deletion still follows the event workflow.
- Unconfirmed-user cleanup and avatar-reference reads now pass through user maintenance access. Unconfirmed cleanup is still a bulk operation with foreign-key cascades, not a per-row hook guarantee.
- Account deletion and local conversion now list files through the context. No `sql!(..., users::...)` or `files::...` calls remain; other workflows retain their existing public/internal resource helpers, which now delegate to the contexts.

Bulk exceptions remain: local-account ownership transfer and cleanup, trial cleanup, purge, sync-state reset, foreign-key cascades, and read joins in other adapters. Their planned lifecycle/infrastructure phases still apply. No schemas or dependencies changed.

Added behavioral tests for user anonymization being absent from ordinary reads but available to projections; user deletion and event rollback; owner-only file reads with tombstone references retained for cleanup; and file update/event rollback. Existing tests continue covering deleted-account authentication, profile/avatar sync, file uploads/retries/conflicts, pending protection, local conversion, and byte cleanup.

The identity-context increment below converts account users, tokens, credentials and staff and their direct adapter callers.

### Users/files verification

- Format, workspace Clippy with warnings denied, and whitespace checks passed.
- Workspace tests passed: 202 core, 31 Tauri and 45 web tests; two HTTP integration tests remain ignored.
- All four moved users/files adapters match their prior SQL exactly. No direct calls to their former backend paths remain.
- The supplemental `postgres,sync-http,object-storage` core test run compiled both database backends and passed 166 tests, but failed two existing job assertions that expect payment jobs even with payment features disabled (two HTTP tests ignored). The jobs code is unchanged by this refactor.
- `cargo audit` still reports the existing `libsqlite3-sys` advisory and five allowed warnings. No dependency changes were made.
- `cargo test -p app_core --all-features --lib` passed: 202 tests, two ignored, including the job assertions with their payment features enabled. This verifies compilation of both adapters with the complete feature set. `TEST_POSTGRES_URL` is absent, so runtime PostgreSQL parity remains outstanding.

## Identity contexts implemented

Added `data::accounts_users`, `data::tokens`, `data::credentials` and `data::ops_staff`, each with private SQLite/PostgreSQL adapters. These names follow the existing adapter resource names. All eight adapters retain their original SQL. Existing resource functions remain the public/internal workflows, and they own transaction boundaries and policy.

- Accounts: ordinary get/get-by-email exclude deleted rows. Create accepts a validated account. A typed update accepts only confirmation or trial-signup changes constructed by `accounts::users`; email normalization and validation stay in the existing callers. Soft deletion remains explicit. Trusted lifecycle reads retain deleted rows for the existing trial, checkout, deletion and test callers that previously queried the adapter directly.
- Billing: account customer mappings are accessed through the account context's feature-gated billing operations. Set-if-missing remains an atomic statement, preserving concurrent checkout behavior. Stripe calls remain outside persistence.
- Tokens: create requires a workflow-constructed token with hashed secrets; get requires a hash, token context and strict expiry cutoff. Code replacement requires a workflow-produced hash, resets attempts, and refreshes its timestamp using the existing statement. Attempt increments retain `UPDATE ... RETURNING`, avoiding a read-then-write race. Context-specific deletion and API/session-only signout deletion preserve their predicates. Expiration is explicit maintenance access, with cutoffs still supplied by token policy.
- Credentials: upsert accepts only an encoded hash produced inside the credential workflow. Length checks, Argon2 hashing, constant-work fallback verification and `spawn_blocking` stay there. The context exposes internal hash retrieval for verification; no hash or secret is added to a transport type or log.
- Staff: create requires an assignment constructed after the workflow's server-role and account lookup checks. Idempotent insert and existing get/delete semantics remain intact. Account deletion uses the context to remove staff in its existing transaction.

All production and fixture `sql!(..., accounts_users::...)`, `tokens::...`, `credentials::...` and `ops_staff::...` calls were routed to the corresponding contexts, including checkout/subscription customer mapping and trial lookup. Signup, signin, local conversion and deletion continue composing their existing workflow helpers, which now use the contexts.

Physical cleanup stays explicit under account/token maintenance access. Account deletion cascades to credentials, tokens and staff in SQL; unconfirmed account cleanup also removes its application user first. Bulk trial/local-user/purge/sync-reset adapters and cross-resource read joins remain the documented exceptions for later lifecycle/maintenance work. A cascade does not invoke a Rust hook for every child. These server-only identity records continue to use plain transactional writes, not application sync events.

Added behavioral tests for rollback of signup account/profile/credential/token records together; rollback and commit of account cleanup cascading credentials, tokens and staff; and code reset preserving token identity and destination while resetting attempts and keeping context isolation. Existing tests cover expiry, password replacement, confirmation, email uniqueness, deleted-account auth, staff idempotency, device signin, trial conversion and account deletion.

The commerce increment below converts product/price storage, checkouts, subscriptions, purchases and provider receipts. Bulk lifecycle and infrastructure exceptions remain listed for their later phases.

### Identity-context verification

- Formatting, workspace Clippy with warnings denied, and whitespace checks passed.
- `cargo test --workspace --all-targets` passed: 205 core, 31 Tauri and 45 web tests; two HTTP integration tests remain ignored.
- `cargo test -p app_core --all-features --lib` passed: 205 tests, two ignored. Both adapters compile with the full feature set; `TEST_POSTGRES_URL` is absent, so runtime PostgreSQL parity remains outstanding.
- All eight moved adapters match their original SQL exactly. No direct calls to their former backend paths remain.
- `cargo audit` still reports the existing `libsqlite3-sys` advisory and five allowed warnings. Dependencies and schemas were not changed.
- Default-feature `cargo check -p app_core` passed without warnings; feature-specific imports are gated. Workspace Clippy passed again after that adjustment.

## Commerce contexts implemented

Commerce is the persistence portion of payments/licensing: the catalog, payment attempts, subscriptions, one-time purchases, and provider delivery receipts. Added feature-gated `data::accounts_products`, `accounts_prices`, `accounts_checkouts`, `accounts_subscriptions`, `accounts_purchases`, `accounts_stripe_events`, and `accounts_app_store_notifications`. These own fourteen private SQLite/PostgreSQL adapters with unchanged SQL.

- Products/prices: concrete list/get/provider-ID lookup and accepted upsert operations. Active/for-sale projections, supported lookup keys, grant rules, price normalization and provider calls remain in workflows. Refresh deactivation is explicit maintenance access; deactivate-and-upsert remains one transaction. Stored inactive catalog rows remain readable for purchase history.
- Checkouts: create accepts a workflow-constructed record; status update accepts a workflow-constructed provider status. Ordinary get requires the account ID and excludes another account's row. Provider-session lookup and pending-session scans are explicit processing access. Customer creation and provider checkout calls keep their existing boundaries.
- Subscriptions: ordinary get/list require the account ID. Provider identity lookups, reconciliation scans and replacement reads use explicit processing access. Accepted upsert is owned by the subscriptions workflow; store-transaction processing now calls its internal `upsert_in` helper rather than writing another resource's adapter. Status/entitlement interpretation, schedule handling and replacement cancellation remain in workflows.
- Purchases: account-scoped list/get, idempotent accepted create, ownership predicate and refund operations. Payment-ID/checkout-ID lookups are explicit processing access. Refunds retain history and still remove the ownership grant through the existing predicate. Store transaction identity and cross-account replay rules are preserved.
- Provider receipts: signature verification and current-state provider fetches stay in Stripe/App Store workflows. Context create accepts receipt metadata constructed there and retains `INSERT ... ON CONFLICT DO NOTHING`/affected-row semantics. The claim and resulting resource changes still share one transaction; a failed transaction leaves no processed receipt. App Store non-database refusals retain their existing deliberate record-and-log behavior.
- Store transactions, reconciliation jobs and account-deletion payment helpers now use the contexts or owning resource helpers. No direct SQL macro calls to the seven former adapter paths remain. Licensing projections and HTTP/Tauri contracts are unchanged; no new licensing product features were added.

Account, store and provider IDs remain bound parameters. This extraction adds no network calls, receipt payload storage, secret logging, dependency changes or migrations. These server-only records do not become sync events. Existing cross-resource SQL joins remain permitted for projections.

Added behavioral tests proving a rolled-back Stripe receipt/catalog transaction can be retried and then deduplicated, and proving a license purchase/refund follows transaction commit while retaining refund history. Existing tests continue covering catalog refresh/deactivation, price rules, account ownership, repeatable credits versus one-time licenses, refunds, subscription changes/replacements, webhook signatures and duplicates, App Store ownership checks, reconciliation, and account deletion.

The infrastructure and maintenance increment below completes storage extraction, reviews the bulk lifecycle exceptions and removes the shared adapter bypass.

### Commerce-context verification

- Formatting, workspace Clippy with warnings denied, default-feature `cargo check -p app_core`, and whitespace checks passed.
- Workspace tests passed: 207 core, 31 Tauri and 45 web tests; two HTTP integration tests remain ignored.
- Full-feature core tests passed: 207 tests, two ignored. Both database adapters compile; runtime PostgreSQL parity remains outstanding because `TEST_POSTGRES_URL` is absent.
- All fourteen moved commerce adapters retain their original SQL exactly. No calls to their former backend paths remain.
- `cargo audit` still reports the existing `libsqlite3-sys` advisory and five allowed warnings. Dependencies and schemas were not changed.

## Infrastructure, maintenance and boundary enforcement implemented

Added `data::events`, `jobs`, `purges`, `sync_state`, `sync_backfills` and `device_settings`. Also moved local-user/trial bulk adapters beneath explicit `data::local_users` and `data::trial_users` operations so workflows no longer bypass the context boundary for these reviewed exceptions. All sixteen moved adapters retain their original SQL.

- Events: typed status/table list scopes, accepted record creation, pending checks, counts and deletions. Retry/conflict/error transitions are explicit processing functions. Attempt increments still use one SQL `UPDATE ... RETURNING`; refusal thresholds, conflict assembly, trial-write limits and local-user queue suppression remain in the events workflow. Missing-business-row cleanup is explicit maintenance.
- Jobs: get plus explicit claim/finish processing. Conditional claims remain atomic SQL with affected-row semantics. Role/setup/interval policy, execution and completion recording retain their existing transactions; no lock or read-then-write replacement was introduced.
- Purges: accepted run records, horizon lookup and explicit retention deletion. A typed table allowlist preserves the six business tables, children-first ordering, SQL predicates and report keys. The purge workflow still combines tombstone cleanup, event cleanup, token expiry, unconfirmed-account cleanup and horizon recording in one transaction.
- Sync state/backfills: concrete cursor get/upsert and idempotent backfill create/list/delete. Device reset is explicit maintenance with the existing pending/conflicted-row preservation predicates and foreign-key behavior. Applied rows, new backfills and page cursor still share the caller's transaction.
- Device settings: optional raw JSON get/upsert under the context. Defaults, typed serialization errors, missing-key behavior and upgrade policy remain in `device_settings.rs`; existing shared transactions are preserved.
- Local/trial lifecycle: read counts and trial-visibility predicates are explicit operations; ownership transfer, promotion visibility updates and destructive cleanup use maintenance access. Existing role, account and trial policy stays in workflows.

The shared `database::sql!` macro was removed after its final callers migrated. Every record adapter is now a private child of its context. `core/src/sqlite/mod.rs` and `postgres/mod.rs` contain only migration runners. README, crate documentation and AGENTS.md now describe application entry points owning transactions and internal contexts borrowing them. AGENTS.md requires future application workflows to use the contexts.

### Reviewed semantic exceptions

These operations pass through explicit context entry points, but remain exceptions to the rule that a write changes only its resource. Replacing their atomic/bulk SQL with loops of ordinary CRUD would change semantics or cascade behavior.

| Context/adapters | Exception and rationale |
| --- | --- |
| `data/local_users` | Multi-table ownership transfer, local tombstone cleanup and converted-user deletion in the account-conversion transaction |
| `data/trial_users` | Trial isolation queries, promotion visibility timestamps, whole-trial deletion and expiry cleanup across related resources |
| `data/app_sync_state` maintenance | Device resync clears multiple tables while retaining pending/refused-file state; page application and reset keep their existing boundaries |
| `data/app_purges` maintenance | Retention hard deletion across an explicit business-table allowlist, preserving version/time cutoffs and children-first order |
| `data/app_events` maintenance | Event cleanup for missing rows reads multiple business tables |
| `data/app_users` and `data/accounts_users` maintenance | Unconfirmed/orphan-account cleanup and foreign-key cascades to credentials, tokens and staff |
| `data/app_posts` and other resource maintenance | Physical removal/pruning on devices can cascade into association rows |
| Resource adapters' read joins | Visibility predicates, authentication joins and purchase/subscription projections may read related resources without loading them individually |
| Database/migration setup and test fixtures | Connection/migration execution and deliberate test-state setup are outside production record operations |

SQL cascades and these bulk operations do not invoke a Rust per-row hook. Hooks needing complete logical-change coverage must define their bulk semantics explicitly. Exactly-once external publication is still outside this refactor; post-commit/durable-delivery policy remains a separate design decision.

Compiler privacy prevents workflows from calling the private adapters, and removal of the shared dispatcher closes the old access path. The current production core source has no record queries outside the contexts. Rust still exposes SQLx and database types, so a future direct query must be caught during review; this work does not claim a type-level prohibition on every possible bypass. No custom linter/framework was added.

Added behavioral tests for event refusal/retry rollback and metadata reset, rollback/reclaim of an atomic job claim, and atomic application of sync rows, a membership-triggered backfill, the cursor and a device setting. Existing sync, purge, trial cleanup, local conversion and job tests remain the wider regression coverage.

Storage extraction and the boundary review are complete. Remaining follow-up is runtime PostgreSQL verification with a configured database, the separately recorded dependency remediation, and any future design for telemetry or durable publication. Those are not silently included as product features or schema changes here.

### Infrastructure and final boundary verification

- Format, workspace Clippy with warnings denied, default-feature `cargo check -p app_core`, and whitespace checks passed.
- Workspace tests passed: 210 core, 31 Tauri and 45 web tests; two HTTP integration tests remain ignored.
- Full-feature core tests passed: 210 tests, two ignored. Both database adapters compile; runtime PostgreSQL parity remains outstanding because `TEST_POSTGRES_URL` is absent.
- All sixteen adapters moved in this increment retain their original SQL exactly. All backend root resource exports and the shared dispatcher are gone. A production-source audit found no record queries outside `core/src/data/`; remaining direct SQL is in deliberate test fixtures/schema setup and migration runners.
- `cargo audit` still reports the existing `libsqlite3-sys` advisory and five allowed warnings. No dependencies or schemas were changed.


## Dependency follow-up

A temporary vendored SQLx compatibility patch resolved the SQLite advisory during verification, but the project owner subsequently chose to remove that patch and wait for a published upstream release while the app is in development. The known advisory and five upstream warnings remain visible. Current status and the upstream fix reference are in [dependency-security.md](dependency-security.md).


## App workflow source organization

App resource workflows moved into `core/src/app/`, declared by `core/src/app.rs`: avatars, communities, conflicts, current community, events, files, local users, memberships, pending changes, posts, post-community associations, purge, sync and user profiles. Their source contents are unchanged. Root re-exports preserve `crate::posts` / `app_core::posts` and the corresponding resource paths while also exposing `app_core::app::posts`.

Shared database, deployment, settings, scheduling, storage and validation infrastructure remains at the root, as do account helpers such as credentials, tokens and mail. The `data`, `sqlite` and `postgres` directories retain their locations. This is source organization only; there are no schema, transport or behavior changes.

App-directory verification: all fourteen moved files were checked byte-for-byte against their pre-move contents. Formatting, workspace Clippy with warnings denied, whitespace checks and workspace tests passed (211 core, 31 Tauri, 45 web; two HTTP tests ignored). Audit passed with zero vulnerabilities and the same five documented upstream warnings. No call-site namespace changes were needed because the root re-exports preserve the existing module paths.
