# Purchasing on phones

**Status: iOS built 2026-11-12, not yet tried by hand in the simulator
(see "As built" in `purchasing-ios-build.md`); Android in a later
effort.** Step 8 of `purchasing.md`: plans and one-time purchases
bought in the app on iOS through the App Store and on Android through
Google Play. Read `purchasing.md` first; this doc only adds what
differs on a phone. The build plan for iOS, with deleting an account,
is `purchasing-ios-build.md`.

## Why the phones need their own store

- **The rules.** Apple's guideline 3.1.1 and Google Play's payments
  policy ask that digital goods bought in the app are bought through
  the store. Apple's 3.1.3(b), multiplatform services, lets an app
  unlock what was bought on the web only if the same items can also
  be bought in the app. So an app that shows plans but cannot sell them
  risks rejection; one that sells them through the store does not.
- **The US is the exception, for now.** Under the Epic injunctions a
  US app may link to its own web checkout. Checked 2026-11-12: Apple
  charges nothing on those links today, but the Ninth Circuit allows
  some fee and the rate is still before the district court, and Apple
  has asked the Supreme Court to hear it. Google says it means to
  charge a fee and is not charging one yet. Too unsettled to build on;
  see Left out.
- **The store is the merchant of record**, as Managed Payments is on
  the web: it charges the tax and files it. It keeps 15% under Apple's
  Small Business Program and on Google's first $1M a year, 30% above.

## What stays the same

- **One thing at a time, the same pages.** A product's page has Buy;
  the store's own sheet takes the payment instead of Stripe's page.
- **Only a full user buys.** A local user sees Sign up, as on desktop.
  Apple rejects an app that makes people sign up to buy something not
  tied to an account; here every purchase is the account's and follows
  it to the web and the desktop, which is the case Apple accepts. The
  review notes say so.
- **The server is the copy, the store the system of record.** Our
  tables hold what the store says, kept up to date by its server
  notifications and a daily job, as with Stripe.
- **What a user has** is still `plans::current` and `purchases::list`.
  A store's subscription or purchase is a row like a Stripe one, with
  its `store`, so nothing that asks what a user may do changes, and
  the device's kept plan works as it does now.
- **One plan per user.** Someone on a plan, from any store, sees their
  plan in place of Buy on every plan's page.
- **A plan that is not paid is no plan**, in every store.

## The catalog on a phone

- **A store's product id is the lookup key.** In App Store Connect and
  the Play Console each product is made with the id of its row in
  `purchasing.md`'s catalog: `single_user_monthly`,
  `single_user_yearly`, `license`, `credits`. `Grant::from_lookup_key`
  already maps these, so the code learns no new names.
- **On Google Play** a subscription is a product with base plans. Each
  plan's lookup key is a subscription of its own with one base plan,
  `monthly` or `yearly`, so one id still names one price.
- **A store's row points at the Stripe price with the same lookup
  key** for its `product_id` and `price_id`. The catalog stays the one
  copied from Stripe, so `accounts_products` and `accounts_prices` need
  no store columns, and a phone purchase shows on the web with the same
  name. The cost: a fork must have its Stripe catalog synced to sell on
  phones, even in a sandbox.
- **The price shown on a phone is the store's**, in the buyer's
  currency, asked of StoreKit or Play Billing on the device. The store
  sets it per country. A fork may charge more on phones to cover the
  store's share; that is set in each store, not in our code.
- **Kinds map to the stores' kinds**: a plan is an auto-renewable
  subscription (on Apple, all plans in one subscription group); a once
  product is non-consumable on Apple and a one-time product on Google;
  a repeatable one is consumable on Apple and consumed after it is
  recorded on Google.
- **Free days are set in each store**, as an introductory offer on
  Apple and a free-trial offer on Google, by hand, matching
  `Grant::free_days`. The placeholders have none.
- **Apple asks the buy page** for a subscription to show its name,
  length and price, and to link the terms of use and the privacy
  policy. On a phone the product's page adds these.

## User flows

These replace P2 to P4 and P7 to P10 of `purchasing.md` on a phone.
P1, P5 and P6 are the same pages, with the store's price.

**M1. Buy.** Buy opens the store's sheet for that product, with the
user's id attached: Apple's `appAccountToken` and Google's
`obfuscatedAccountId`, both the user's UUID. When the store says paid,
the app sends the store's proof to the server: Apple's signed
transaction, Google's purchase token. The server checks it with the
store, records the purchase or the subscription, and, on Google,
acknowledges it. Only then does the app finish the transaction with
the store. The page goes to the new purchase's or subscription's page.
A store payment that waits, such as Ask to Buy, says Pending and is
recorded when the store's notification comes.

**M2. Restore.** The products page on iOS has Restore purchases, as
Apple asks for non-consumables. The app sends every transaction
StoreKit holds for this Apple ID to the server, which records the ones
it lacks. Android needs no button: the app sends Play Billing's
purchases each time the products page opens.

**M3. Someone else's purchase.** A store transaction that carries
another user's id is refused: "bought while signed in as someone
else". One without an id, such as an Apple offer code, is given to the
signed-in user who sends it first, and refused for anyone after.

**M4. Switch, cancel and resume.** A store's subscription is changed in
that store:
- Switch on a phone buys the other plan through the store. Apple
  decides the timing within a subscription group: an upgrade at once
  with a prorated refund, a downgrade at the renewal, which is our rule
  already. On Google the app picks the replacement mode by
  `subscriptions::switch_waits`: at once and prorated, or deferred.
- Cancel and resume open the store's own subscription page. Neither
  store lets a server cancel for the person.
- The subscription's page says "Changes to … on …" from the store's
  renewal info, as for Stripe, with Manage in the App Store (or Google
  Play) in place of Keep.

**M5. A plan from another store.** A Stripe plan seen on a phone says
"Manage on the web" and links nowhere. An App Store or Google Play plan
seen on the web or desktop says "Manage in the App Store on your
iPhone" (or Google Play on your Android device), with no buttons.
Buying a Stripe plan is refused while a store plan is live, as one
plan per user already does.

**M6. A payment fails.** The store retries. While it does the
subscription is `past_due`: no plan, and the banner says to update the
payment in the App Store or Google Play, opening the store's
subscription page. Apple's billing grace period and Google's grace
period are left off in both consoles, so a failed renewal is no plan,
as on the web.

**M7. Refunds.** The stores refund without us: Apple's `REFUND`
notification and Google's voided purchases mark the purchase refunded,
or end the subscription, through `purchases::refund_in` as on the web.

**M8. Deleting an account.** Apple asks any app that lets people make
an account to let them delete it in the app, guideline 5.1.1(v). This
holds for any iOS release, not only one that sells. A store's
subscription cannot be cancelled by our server, so the delete page
says to cancel it in the store. Designed in `accounts-delete.md`.

## iOS first, Android without a rewrite

Only the App Store is built now. What Android needs later is kept out
of the App Store's code by drawing the line where the stores differ:

- **Core writes rows from one store-neutral shape.**
  `accounts::store_transactions` takes a `StoreTransaction`: the store,
  the user id the store carried, the lookup key, and either a one-time
  purchase (store id, amount, currency, refunded) or a subscription
  (store id, the id it replaced, status, period end, cancelling,
  ended, the scheduled lookup key and when). It applies M3, the lookup
  key to product and price, one plan per user, and writes
  `accounts_purchases` or `accounts_subscriptions`. Nothing in it
  names Apple.
- **A store module only translates.** `accounts::app_store` checks
  Apple's proof and answers `StoreTransaction`s, and
  `accounts::app_store_notifications` handles Apple's notifications as
  `stripe_events` does Stripe's. Google later adds `play_store` and
  `play_store_notifications` beside them, and nothing else in core.
- **One route for the app's proof**, tagged by store:
  `POST /api/accounts/store-transactions` with
  `{"store": "app_store", "signed_transaction": "..."}`, later
  `{"store": "play_store", "product_id": "...", "purchase_token": "..."}`.
  Core's `StoreProof` enum has the App Store's variant now; Google's is
  one more variant.
- **The device talks to a store through one Rust trait**,
  `StoreSheet` in `tauri/src/accounts/store_sheet.rs`: the store's prices for
  some lookup keys, buy a lookup key for a user id (with the plan it
  replaces), the transactions not yet finished, finish one, open the
  store's subscriptions page. The commands are written against it;
  iOS implements it with the plugin, Android answers "not available
  yet" until its effort. If the plugin is the one chosen for both
  stores, Android's implementation is mostly the same calls.
- **The pages read `store`** from what the commands answer, `stripe`,
  `app_store` or `play_store`, to choose Buy's wording and where Manage
  goes. They never test for iOS.
- **Status mapping is per store**, a function in each store module
  onto `SubscriptionStatus`.

## The server

| Store | Proof the app sends | Checked with | Notifications |
|---|---|---|---|
| App Store | the signed transaction, JWS | App Store Server API, `Get Transaction Info` and `Get All Subscription Statuses` | App Store Server Notifications V2 |
| Google Play | the purchase token | Play Developer API, `purchases.subscriptionsv2.get` and `purchases.products.get` | real-time developer notifications, Pub/Sub push |

- **The same rules as the Stripe copy.** A notification is a signal:
  its id is recorded in the transaction that handles it, a repeat is
  skipped, and the handler asks the store for the current state rather
  than writing the payload, so a late one never overwrites a newer
  state.
- **Who is who**: the app's token, then the store's account id, which
  must be the signed-in user's. A notification finds its user by the
  row it already has, or by that id.
- **What the rows hold**: a subscription's `store_id` is Apple's
  `originalTransactionId`, which stays across renewals and switches,
  and Google's purchase token, which changes on a switch, so the new
  token's `linkedPurchaseToken` finds the old row. A purchase's
  `store_id` is Apple's `transactionId` and Google's purchase token.
  The amount and currency are the store's.
- **Status**: Apple's and Google's states map onto
  `SubscriptionStatus`: active, and Apple's or Google's free trial, as
  `active` or `trialing`; billing retry, Google's on hold, as
  `past_due`; expired, revoked, as `canceled` with `ended_at`.
- **Google's three days.** A purchase not acknowledged within three
  days is refunded by Google (five minutes for a license tester). The
  server acknowledges as it records, and the daily job acknowledges
  any it missed.
- **The daily job** gains `app_store_reconcile` and
  `play_store_reconcile` beside `stripe_reconcile`: every live store
  subscription asked again, and on Google the voided purchases of the
  last days, since refunds can be missed.
- **Fakes for tests**, `AppStore::Memory` and `PlayStore::Memory`, as
  `Stripe::Memory`.
- **Feature-gated** like `stripe`: `app-store` and `play-store`
  features in core, enabled only by `axum`. The app compiles neither.

### Routes

| Route | Does |
|---|---|
| `POST /api/accounts/store-transactions` | the app's proof, any store, M1 to M3; answers the purchase or subscription |
| `POST /api/accounts/app-store-notifications` | Apple's notifications, no session, checked by Apple's signature against its root certificates; a 404 unless configured |
| `POST /api/accounts/play-store-notifications` | Pub/Sub push, no session, checked by its OIDC bearer token; a 404 unless configured |

### Settings

All the environment's, the keys held in `secrecy::SecretString`, never
committed:

- Apple: `APP_STORE_BUNDLE_ID`, `APP_STORE_APP_ID` (needed to check
  production data), `APP_STORE_ISSUER_ID`, `APP_STORE_KEY_ID`,
  `APP_STORE_PRIVATE_KEY`. Apple's root certificates are public and
  checked into the repo.
- Google: `PLAY_STORE_PACKAGE_NAME`, `PLAY_STORE_SERVICE_ACCOUNT_KEY`
  (the JSON key, for the Developer API),
  `PLAY_STORE_PUSH_SERVICE_ACCOUNT` and `PLAY_STORE_PUSH_AUDIENCE`
  (what the push's token must name).

### Tables

New, server only, plain writes like every `accounts_` table:

| Table | Holds |
|---|---|
| `accounts_app_store_notifications` | Apple's `notificationUUID`, type, when handled |
| `accounts_play_store_notifications` | the Pub/Sub message id, type, when handled |

`accounts_purchases` and `accounts_subscriptions` take store rows as
they are. `amount` and `currency` stay required if both stores report
them for a one-time purchase, see To check.

## The app

- **A plugin for the store sheets.** StoreKit 2 and Play Billing are
  Swift and Kotlin; Tauri reaches them through a mobile plugin. The
  Tauri commands call it, so the pages still call only
  `svelte/src/lib/core/`.
- **The same commands where they fit.** On a phone the products
  commands add the store's price to each price, and a new
  `accounts_store_transactions_create(price_id)` runs M1 and answers the
  new purchase or subscription; `accounts_store_transactions_restore` runs
  M2. `refused_on_a_phone` goes, and the checkout and portal commands
  answer that a phone uses its store.
- **A transaction is finished only after the server has it.** A
  transaction the app could not send, offline or the app killed, stays
  unfinished in the store and is sent again when the app starts, so
  nothing paid is lost.
- **Offline**, the kept plan is trusted until its period ends, as now.

## Decided

Agreed 2026-11-12, "this makes sense", with iOS first: 1, 2, 6, 7 and
8 as recommended. 3 and 4 are approved if they pass step 0's checks
in `purchasing-ios-build.md`; 5 waits for Android; 9 is approved for
the App Store's table.

**Built against fakes.** This repo is public and holds no Apple
credentials: the server's App Store is tested with its `Memory` fake,
and the app is tried in the simulator with an Xcode StoreKit
configuration file. A fork sets its own App Store Connect settings and
tries Apple's sandbox.

1. **Sell on phones through the stores, both of them.** Recommended:
   the only way that is allowed everywhere. The alternative, phones
   that only show what was bought on the web, risks Apple's 3.1.3(b).
2. **iOS first, then Android.** Recommended: `tauri/gen/apple` exists;
   Android needs `tauri android init` first, a deployment change.
3. **The device plugin.** Recommended: check `tauri-plugin-iap`
   (Choochmeque; StoreKit 2 and Play Billing 8, `appAccountToken`,
   `obfuscatedAccountId`, Android replacement modes) before building
   on it, as `async-stripe` was checked: maintained, its license, no
   advisory, that it gives the app Apple's signed transaction and
   Google's token, and that Rust can call it. It is pre-1.0 (0.8).
   The fallback is a small plugin of our own, asked first.
4. **The Apple server library.** Recommended: check
   `app-store-server-library` (community, 4.x) the same way. Its
   signature check calls OCSP blocking, so it runs in
   `spawn_blocking`.
5. **The Google server calls.** Recommended: a small client of our own
   for the five calls we make (subscriptions v2 get, products get,
   acknowledge, consume, voided purchases), with a crate for the
   service account's token such as `gcp_auth`, over the generated
   `google-androidpublisher3`, which is large. Asked here, as AGENTS.md
   says before writing our own.
6. **The catalog by lookup key**, no store columns on products and
   prices, as above.
7. **Grace periods off** in both stores, keeping "not paid is no plan".
8. **A transaction without a user id** goes to the first full user who
   sends it, M3.
9. **The migration** for the two notification tables, when its step
   comes.

## Checked 2026-11-12

Step 0 of `purchasing-ios-build.md`, from each crate's published source
and its repository.

**`app-store-server-library` 6.0.2** (namecare, MIT; 4.x is no longer
the newest, 6.0 came out 2026-08-13). **Passes.**

- Maintained: releases 2026-08-13, 09-28 and 11-04; 1.5M downloads.
- No RustSec advisory on the crate. Its `x509-validator` had
  RUSTSEC-2026-0312, fixed in 0.3.1; 6.0.2 takes 0.3.2.
- `SignedDataVerifier::new(root_certificates, environment, bundle_id,
  app_apple_id, ..)` checks the chain against the roots we give it,
  then the bundle id and the environment.
  `verify_and_decode_signed_transaction` and
  `verify_and_decode_notification` are there.
- `AppStoreServerApiClient::get_transaction_info` and
  `get_all_subscription_statuses` are there, async.
- **No OCSP**: the chain is checked offline, with no revocation check
  (its docs say so). So verifying does not block and needs no
  `spawn_blocking`; the build doc's note on OCSP no longer holds.
- TLS: feature `api-client-reqwest` takes `reqwest` 0.13 with
  rustls, which the workspace already has. The signature check needs
  one crypto feature: `ring`, already in `Cargo.lock`.

**`tauri-plugin-iap`** (Choochmeque, MIT): 0.9.1 is the newest
release, 0.10.0-rc.10 the newest candidate. **Fails one check.**

- Maintained: commits weekly, the last 2026-11-08; no RustSec advisory.
- Rust can call it: `app.iap().purchase(..)` through `IapExt`.
- On iOS it gives Apple's signed transaction, `jws_representation`,
  after a purchase and from `restore_purchases` (StoreKit's current
  entitlements). It takes `app_account_token` (a UUID).
- **It finishes every transaction itself**: `purchase` calls
  `transaction.finish()` before returning, and its `Transaction.updates`
  listener finishes each update and only tells JavaScript. Its
  `acknowledge_purchase` and `consume_purchase` do nothing on iOS.
  Pull request #267, which would have stopped this on macOS, was
  closed unmerged. So a paid transaction is finished before our server
  has it, which "A transaction is finished only after the server has
  it" rules out; a credits purchase lost that way is gone for good.
- No call for the unfinished transactions (`Transaction.unfinished`).
- No call that opens the subscriptions page (`showManageSubscriptions`).

Decided 2026-11-12: `app-store-server-library` 6.0.2, and a small iOS
plugin of our own on StoreKit 2 in place of `tauri-plugin-iap`.

## Build order

iOS, with deleting an account first: `purchasing-ios-build.md`.
Android, a later effort: Google Play's server module and notifications
route, then `tauri android init`, its `StoreSheet`, tried with license
testers.

## Setting up the stores

Once per fork, written out with step 1 and step 3: in App Store Connect
the agreements, the Small Business Program, the subscription group and
products with the catalog's ids, an In-App Purchase key, and the
notification URLs for production and sandbox; in the Play Console the
merchant account, the products, a Google Cloud project with a Pub/Sub
topic that Google Play's service account may publish to, a push
subscription with an OIDC token to our route, and a service account
with access to the app's orders.

## Setting up the App Store

Once per fork, as "Setting up Stripe" in `purchasing.md`. Nothing here
is ever committed; the repo holds only the placeholders.

1. **Agreements**: in App Store Connect, Business, sign the Paid Apps
   Agreement and fill in the bank and tax forms. Nothing is sold until
   it is active.
2. **The Small Business Program**: apply at
   developer.apple.com/app-store/small-business-program, for Apple's
   15% on the first $1M a year.
3. **The app**: an app record with the bundle id of
   `tauri/tauri.conf.json`'s `identifier`; its Apple ID, a number on
   the App Information page, is `APP_STORE_APP_ID`.
4. **The products**, under Monetization, each with the catalog's id as
   its Product ID, a name, a price and a review screenshot:
   - a subscription group, "Single user subscription", with two
     auto-renewable subscriptions, `single_user_yearly` at level 1 and
     `single_user_monthly` at level 2, so yearly is the upgrade;
   - `license`, non-consumable;
   - `credits`, consumable.

   Free days, if a fork wants them, are an introductory offer on each
   subscription, matching `Grant::free_days`.
5. **Grace period off**: the subscription group's Billing Grace Period
   stays off, so a failed renewal is no plan, as on the web.
6. **The In-App Purchase key**: Users and Access, Integrations,
   In-App Purchase, Generate. Download the `.p8` once; its Key ID is
   `APP_STORE_KEY_ID`, and the Issuer ID above the list is
   `APP_STORE_ISSUER_ID`. The file's contents are
   `APP_STORE_PRIVATE_KEY`.
7. **Notifications**: App Information, App Store Server Notifications,
   Version 2, the same URL for production and sandbox:
   `https://<the server>/api/accounts/app-store-notifications`. Then
   from the server, Request a Test Notification (or the API's) and see
   `app store notification ... TEST: handled` in its log.
8. **A sandbox tester**: Users and Access, Sandbox, Test Accounts. On
   the iPhone, Settings, App Store, Sandbox Account, sign in with it;
   a TestFlight or development build then buys in the sandbox, free,
   with renewals every few minutes.
9. **The server's settings**, from the environment:

   ```sh
   APP_STORE_BUNDLE_ID=com.example.starterpack
   APP_STORE_APP_ID=1234567890        # omit to take the sandbox only
   APP_STORE_ISSUER_ID=...
   APP_STORE_KEY_ID=...
   APP_STORE_PRIVATE_KEY="-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----"
   ```

   None set, the server sells nothing in the iPhone app and its
   notifications route answers 404. The server must also have Stripe's
   catalog synced, since a store's row points at the Stripe price with
   the same lookup key, see The catalog on a phone.
10. **Review notes**: say that purchases belong to the account and
    follow it to the web and the desktop, that a test account is
    given, and where Restore purchases and Delete my account are.

**Trying it without App Store Connect.** `tauri/gen/apple/StarterPack.storekit`
holds the four products and the scheme uses it when the app is run
from Xcode (`cargo tauri ios dev --open`, then Run). StoreKit's sheet,
Restore and the subscriptions page work in the simulator; the server
cannot check what the simulator signs, since Xcode signs it rather
than Apple, so the purchase is refused when sent. Trying a purchase end
to end needs Apple's sandbox, steps 3 to 9.

## To check, before building

- Whether Google's API gives the amount and currency of a one-time
  purchase; Apple's signed transaction has both.
- The App Store's view of an account-required purchase in review, with
  the notes above.
- Whether a fork that never sets up Stripe should still sell on
  phones; this design says no, see The catalog.

## Left out

- Linking to the web checkout from a US app, while the fees are in
  court. A suggestion for when they are settled.
- Family Sharing, offer codes, promotional and win-back offers.
- The Mac App Store and Microsoft Store: the desktop is sold outside
  them, through Stripe.
- Selling on a phone to a local user before sign-up.
