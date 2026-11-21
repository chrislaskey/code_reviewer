# Purchasing

**Status: agreed 2026-11-11; steps 1–7 built 2026-11-11, step 8 agreed in `purchasing-phones.md`, iOS first.** Plans and one-time
purchases in `accounts`, paid through Stripe on the web and desktop and
later through the App Store and Google Play on phones.

## What is agreed

- **One thing at a time, everywhere.** No cart. A product's page has a
  Buy button and buying it is one checkout. The phone stores sell one
  product at a time, so the web and the desktop do too, and every
  platform shows the same pages.
- **Plans and one-time purchases.** A plan is a subscription, monthly
  or yearly. A one-time purchase is paid once. A checkout never mixes a
  monthly and a yearly price, which one-thing-at-a-time gives anyway.
- **A user pays**, never a community.
- **Only a full user buys.** A local user, or a trial user, sees the
  products with Sign up in place of Buy.
- **A plan that is not paid is no plan.** Only a subscription that is
  `active` (or in Stripe's free days, `trialing`) gives its plan. One
  that is `past_due`, `unpaid`, `incomplete` or `canceled` gives
  nothing, the same as never having bought it. Read-only while Stripe
  retries the card is a later choice, see Left out.
- **Stripe is the system of record** for payments, subscriptions and
  invoices. Our tables are a copy kept up to date by its webhooks, used
  to show the pages and to answer what a user has. What a product
  unlocks is decided by our code, not Stripe.
- **`async-stripe`**, the 1.0 line: split into crates, so we take only
  the ones we use, `async-stripe-webhook` in step 1, the client with
  checkout, billing and product from step 2, each pinned exactly. Checked 2026-11-11: maintained (1.0.0-rc.10 on
  2026-10-26, commits this month), no RustSec advisory, not archived.
  The last stable, 0.41.0, is from 2025-04 and gets no new API
  versions, so we pin the release candidate exactly and move to 1.0
  when it ships.
- **Stripe is the merchant of record by default**, Managed Payments,
  see Tax.
- **No feature needs a plan yet.** The grants are placeholders, see
  The catalog. The sync needs none.
- **Once or repeatable** per product, once by default, in the code.
- **Free days** per plan, in the code, none on the placeholders.
- **Switching plans**: an upgrade at once, prorated; a downgrade at the
  end of the period already paid, a Stripe subscription schedule.
- **One subscription per user**, the plan.

## Tax

`STRIPE_TAX`, a setting of the deployment, read when a checkout is
made:

| Value | Who sells | The checkout | Cost | We file |
|---|---|---|---|---|
| `managed_payments`, the default | Stripe, the merchant of record | `managed_payments[enabled]` | about 3.5% on top of the card fee | nothing |
| `stripe_tax` | us | `automatic_tax[enabled]` | about 0.5% where registered | where registered |

- **Why the default:** most forks sell little, and the EU and the UK
  charge VAT on digital sales to consumers from the first sale. With
  Managed Payments a small fork sells everywhere and files nothing. A
  fork that grows switches the setting.
- **A subscription keeps the mode it was bought under.** Switching
  affects only new checkouts.
- **What Managed Payments asks:** each product's tax code is one
  eligible for it, set in Stripe's dashboard; the checkout collects the
  buyer's name and billing address; some of the checkout's parameters
  are Stripe's, not ours; the seller is in a country it supports, and
  not a Connect platform.
- The phone stores are the merchant of record for what they sell.

## Stores

A purchase or a subscription records which store sold it, `store`:

| Store | Where | First build |
|---|---|---|
| `stripe` | web, desktop | yes |
| `app_store` | iOS | step 8, `purchasing-phones.md` |
| `play_store` | Android | step 8, `purchasing-phones.md` |

The pages read our copy, so they look the same whichever store sold
it. What differs is where a subscription is managed: a store's
subscription is cancelled or changed in that store. A plan bought on
the web, seen on a phone, says "Manage on the web" and links nowhere,
as the store's rules ask outside the US.

## User flows

**P1. Browse.** `/accounts/products` lists what is for sale: plans
first, then one-time products, each with its price. A product's page,
`/accounts/products/:id`, says what it gives and shows Buy. If the
user already has it, a one-time product they cannot buy twice, or the
plan they are on, the page says so in place of Buy.

**P2. Buy a one-time product.** Buy makes a checkout on the server,
which makes a Stripe Checkout Session in `payment` mode for that one
price and answers its URL. The page goes there: on the web in the same
tab, on desktop in the system browser, as the sign-in does. Stripe's
page takes the card, Apple or Google Pay, and the 3-D Secure step.

**P3. Come back.** Stripe returns to `/accounts/checkouts/:id`. The
server fetches the session from Stripe there and then, so the page
says Paid at once instead of waiting for the webhook. A payment that
settles later says Processing, and the page refreshes. Cancel on
Stripe's page returns to the product. On desktop the browser page says
to go back to the app, and the app's own checkout page fetches the
same checkout when it gets focus again. Stripe emails the receipt.

**P4. Buy a plan.** The same as P2 with a Checkout Session in
`subscription` mode. Someone already on a plan does not buy another:
the other plans' pages show Switch, P7.

**P5. What I bought.** `/accounts/purchases` lists one-time purchases,
newest first; `/accounts/purchases/:id` shows one, with Stripe's
receipt link.

**P6. My plan.** `/accounts/subscriptions` lists the user's
subscriptions, the current one and the ended ones;
`/accounts/subscriptions/:id` shows one: its plan, its status, the
next renewal and its amount, or the day it ends.

**P7. Switch plans.** On another plan's page, Switch changes the
subscription's price in Stripe and comes back to the subscription's
page. No checkout: the card on file pays. An upgrade is at once,
prorated. A downgrade waits for the end of the period already paid;
until then the subscription's page says "Changes to … on …", with a
way to undo it.

**P8. Cancel and resume.** The subscription's page has Cancel, which
ends it at the end of the period already paid, and until then Resume,
which undoes that. Both are our buttons and our API calls.

**P9. Card and invoices.** The subscription's page has Manage payment,
which opens Stripe's Customer Portal for the card on file and past
invoices. The only page of Stripe's we use after the checkout; our own
can replace it later.

**P10. A payment fails.** Stripe retries and emails the person. The
subscription turns `past_due`, the plan is gone, and every page shows
a banner: the payment failed, update your card, linking to P9. When a
retry succeeds the subscription is `active` again and so is the plan.

**P11. Refunds and disputes.** Staff refund in Stripe's dashboard. The
webhook marks the purchase refunded, and a refunded one-time purchase
gives nothing. Ops shows it later.

**P12. Deleting an account** with a plan cancels it at once in Stripe,
with no refund, and deletes the Stripe customer; built 2026-11-12, see
`accounts-delete.md`. A plan the call missed is ended by
`stripe_reconcile`.

## The copy of Stripe

- **Webhooks repeat and come out of order.** `accounts_stripe_events`
  keeps every event id handled; one seen before is answered 200 and
  skipped. An event is a signal, not the data: the handler fetches the
  object it names from Stripe and writes that, so a late event never
  overwrites a newer state.
- **The return page fetches too**, P3, so a person never waits for a
  webhook.
- **A daily job**, `stripe_reconcile` among the template's jobs in `core/src/jobs.rs`, copies the catalog
  again, asks Stripe for every checkout still open or processing, and
  copies every subscription not ended, for a webhook that never came.
  Every live subscription is fine for a small app; a big one would ask
  for those changed lately.
- **The webhook route** is `POST /api/accounts/stripe-webhooks`, no
  session, checked by its `Stripe-Signature` against the raw body. A
  bad or missing signature, or one more than five minutes old, is a
  400; without `STRIPE_WEBHOOK_SECRET` the route is a 404. Built in
  step 1: `app_core::accounts::stripe_events::create`.
- **Only the event's id and type are read.** `async-stripe-webhook`
  types the event's object as it checks it, and refuses one it cannot
  type, an API version apart from the crate's say. It checks the
  signature first, so such a body was still signed: `create` then reads
  the id and type itself. The object is fetched from Stripe anyway, so
  the endpoint's API version never breaks the webhook.
- **Settings** are the environment's: `STRIPE_SECRET_KEY` and
  `STRIPE_WEBHOOK_SECRET`, held in `secrecy::SecretString`; `STRIPE_TAX`;
  and `SERVER_URL`, the server's address as people reach it, which
  Stripe's return links start with, `http://localhost:4000` when unset.
  Without the key the server takes no payments: a checkout is a 503.
- **The events handled**: `product.*` and `price.*`, the catalog;
  `checkout.session.*`, a checkout and, for a plan, its subscription;
  `customer.subscription.*`, a plan changed anywhere; and
  `charge.refunded`. A failed or paid renewal arrives as
  `customer.subscription.updated`, so the `invoice.*` events are not
  needed. Every other event is recorded and changes nothing.

## The catalog

Products and prices are made in Stripe's dashboard and copied here by
the `product.*` and `price.*` webhooks. Our code knows a price by its
Stripe **lookup key**, never by its id, which differs between test and
live. `accounts::products::Grant` maps each lookup key the code knows to
what it gives. A price in Stripe with a key the code does not know is
not listed.

No feature needs a plan yet, so the grants are placeholders, named for
what they are, for a fork to rename:

| Lookup key | Product | Kind | Grant |
|---|---|---|---|
| `single_user_monthly` | Single user subscription | plan | `Grant::SingleUserSubscription` |
| `single_user_yearly` | Single user subscription | plan | `Grant::SingleUserSubscription` |
| `license` | License | one-time, once | `Grant::License` |
| `credits` | Credits | one-time, repeatable | `Grant::Credits` |

- **Once or repeatable** is the grant's, in the code:
  `Grant::is_repeatable`, false unless the grant says otherwise. Making
  a product buyable again is a one-line change, reviewed and tested
  like any other, not a setting in Stripe's dashboard anyone can flip.
  A once product the user has, and has not had refunded, shows Owned
  in place of Buy, and the server refuses its checkout.
- **Free days** are the grant's too: `Grant::free_days`, for a plan,
  none unless the grant says otherwise. Not a column or a JSON field:
  the code acts on it at every checkout, so it is typed and tested. A
  plan with free days still takes the card up front.
- `credits` is there so the repeatable path has a product to be tested
  with. What a credit buys is a fork's.
- Nothing calls `plans::current` or `purchases::list` to allow or
  refuse anything yet. The pages show what the user has.

## Tables

Server only, `accounts_*`, never synced. Plain writes, not events, like
every `accounts_` table. A device has none of them.

| Table | Holds |
|---|---|
| `accounts_users.stripe_customer_id` | made at the user's first checkout |
| `accounts_products` | id, Stripe id, lookup key, name, description, `kind` (plan, one-time), active |
| `accounts_prices` | id, product, Stripe id, amount, currency, interval (none, month, year), active |
| `accounts_checkouts` | id, user, price, Stripe session id, status (open, paid, processing, expired, failed) |
| `accounts_purchases` | id, user, product, price, `store`, the store's id, amount, `refunded_at` |
| `accounts_subscriptions` | id, user, product, price, `store`, the store's id, status, `current_period_end`, `cancel_at_period_end`, `ended_at` |
| `accounts_stripe_events` | Stripe event id, type, when handled |

## What a user has

`accounts::plans::current(deployment, user)` answers the user's plan,
or none, from their one `active` or `trialing` subscription.
`accounts::purchases::list` answers their one-time purchases that are
not refunded. Everything that asks what a user may do asks these two,
so a phone store later changes neither.

- `GET /api/current-user` adds the plan, so the pages know it.
- A device keeps the plan in `device_settings` from the last time it
  was online, and trusts it offline until `current_period_end`.
- The sync needs no plan, which `accounts-local-and-trial-users.md`
  left to purchasing: a full user syncs, with a plan or without.

## Names

| Layer | Names |
|---|---|
| Core | `core/src/accounts/products.rs`, `prices.rs`, `checkouts.rs`, `purchases.rs`, `subscriptions.rs`, `plans.rs`, `stripe.rs` (the client), `stripe_events.rs` |
| API | `/api/accounts/products`, `/api/accounts/products/:id`, `POST /api/accounts/checkouts`, `/api/accounts/checkouts/:id`, `/api/accounts/purchases[/:id]`, `/api/accounts/subscriptions[/:id]`, `PUT /api/accounts/subscriptions/:id` (switch, cancel, resume), `POST /api/accounts/subscriptions/:id/portal` |
| Tauri | `tauri/src/accounts/`: `accounts_products_list` and the rest, one module per resource, each asking the API with the device's token through `app_core::accounts::online`, step 7 |
| Pages | `AccountsProductsIndexPage`, `AccountsProductsShowPage`, `AccountsCheckoutsShowPage`, `AccountsPurchasesIndexPage`, `AccountsPurchasesShowPage`, `AccountsSubscriptionsIndexPage`, `AccountsSubscriptionsShowPage` |

Stripe's own "trial" is called **free days** here
(`Grant::free_days`): a trial user is someone else.

## Build order

1. **The webhook.** Built: `async-stripe-webhook` behind core's
   `stripe` feature, which only the server enables;
   `STRIPE_WEBHOOK_SECRET`; the route checking signatures;
   `accounts_stripe_events`; tests with signed events. The API client
   and `STRIPE_SECRET_KEY` come with step 2, the first to call Stripe.
2. **The catalog.** Built: the API client (`async-stripe` with rustls),
   `STRIPE_SECRET_KEY`, `accounts_products` and `accounts_prices`,
   `Grant`, the catalog events, `app_web accounts products sync`, and
   `/accounts/products` and `/accounts/products/:id`.
3. **One-time purchases.** Built: `STRIPE_TAX`, `SERVER_URL`, the
   customer, `accounts_checkouts` and `accounts_purchases`, Buy, the
   return page `/accounts/checkouts/:id`, `/accounts/purchases` and
   `/accounts/purchases/:id`, refunds: P2, P3, P5, P11. The receipt is
   Stripe's email; the purchase page says so.
4. **Plans.** Built: `accounts_subscriptions`, `plans::current` and
   `plans::payment_failed`, both on `GET /api/current-user`, the
   past-due banner, one plan at a time, `/accounts/subscriptions` and
   `/accounts/subscriptions/:id`: P4, P6, P10. Until step 5 the banner
   points to Stripe's email for a new card.
5. **Changing a plan.** Built: `PUT /api/accounts/subscriptions/:id`
   with `switch`, `switch_undo`, `cancel` or `resume`, and
   `POST /api/accounts/subscriptions/:id/portal`; `scheduled_price_id`
   and `scheduled_at` on `accounts_subscriptions`; the
   `subscription_schedule.*` events. A switch is at once and prorated
   for a dearer price of the same interval and for monthly to yearly
   (Stripe starts a new period when the interval changes); it waits
   for the end of the period for a cheaper price and for yearly to
   monthly, so a paid year is not cut short, with a subscription
   schedule the page shows as "Changes to … on …". A plan ending with
   its period is resumed before it is switched. The banner's Update
   your card opens the portal: P7–P9.
6. **The reconcile job.** Built: `accounts::stripe_reconcile::run`, on
   a server whose deployment has Stripe, `Deployment::with_stripe`;
   `app_web jobs run stripe_reconcile` runs it now.
7. **Desktop.** Built: `app_core::accounts::online`, the purchasing API
   asked with the device's token through the sync's `Http`, its JSON
   passed to the pages as it came, so the app has none of the Stripe
   crates; the Tauri commands under `tauri/src/accounts/`. Stripe's
   checkout and portal open in the system browser with
   `"return_to": "app"`, and come back to `/accounts/back-to-app`, open
   to anyone, as that browser may not be signed in; the app's pages ask
   again when it has the focus. The plan is kept in the `plan` device
   setting, asked for after a sync when older than 15 minutes and after
   a purchase or a change, and trusted offline until its period ends.
   A phone is refused a checkout or a change until step 8.
8. **Phones.** The App Store and Google Play: `purchasing-phones.md`,
   proposed.

Each step asks before its migration, as AGENTS.md says.

## Setting up Stripe

Once per fork. A sandbox is enough for steps 1–4; Managed Payments
needs Stripe's review before it is on, so apply early.

1. **An account**: sign up at dashboard.stripe.com. A new account opens
   in a sandbox, test mode, with test keys at once; nothing it does
   moves money.
2. **Managed Payments**: in the Dashboard's settings, apply for it
   with the business's details. It needs a business in one of the
   supported countries, see Checked, and the account activated for live
   payments. Until it is on, `STRIPE_TAX=stripe_tax` lets the sandbox
   take checkouts.
3. **The webhook in dev**, with the Stripe CLI:

   ```sh
   brew install stripe/stripe-cli/stripe
   stripe login
   stripe listen --forward-to localhost:4000/api/accounts/stripe-webhooks
   ```

   `listen` prints a signing secret, `whsec_...`. Start the server with
   it, `STRIPE_WEBHOOK_SECRET=whsec_... cargo run -p app_web`, and in
   another terminal send an event, `stripe trigger customer.created`.
   The server logs `stripe event evt_... customer.created: recorded`,
   and the CLI shows a 200. The secret stays the same each time
   `listen` runs on that machine.
4. **The webhook in production**: in the Dashboard's Developers,
   Webhooks, add an endpoint at
   `https://<the server>/api/accounts/stripe-webhooks` with the events
   under The copy of Stripe, and give the server its signing secret as
   `STRIPE_WEBHOOK_SECRET`. A sandbox and the live account each have
   their own endpoint and secret.
5. **The API key**, from step 2 on: Developers, API keys. `sk_test_...`
   in dev, `sk_live_...` in production, as `STRIPE_SECRET_KEY`. Never
   committed, never in a `.env` checked in.
6. **The products**, from step 2 on: in the Dashboard's product
   catalog, a product per row of The catalog, with tax code SaaS,
   `txcd_10103000`, and each price's lookup key set to the key there.
   Then `app_web accounts products sync` copies them to the server;
   the webhook keeps them up to date after. Made again in live mode,
   with the same lookup keys.

## Checked 2026-11-11

From `async-stripe` 1.0.0-rc.10's source and Stripe's Managed Payments
docs:

- **`async-stripe` has it**: `managed_payments.enabled` on
  `CreateCheckoutSession`, and its API version, `2026-09-30.endive`,
  is past the `2025-03-31.basil` Managed Payments needs. Subscription
  updates (`proration_behavior`, `cancel_at_period_end`) and schedules
  are in its billing crate.
- **Both modes**: `payment` for a one-time purchase, `subscription`
  for a plan, on Stripe's hosted page. An existing `customer` id is
  taken, and Stripe writes the buyer's name and billing address to it.
- **Refused on the checkout**: `automatic_tax`, `tax_id_collection`,
  `payment_method_types`, `payment_method_configuration`,
  `customer_update[name]` and `[address]`, `invoice_creation`,
  shipping, `adaptive_pricing` (always on), and in payment mode
  `payment_intent_data.receipt_email` and the statement descriptor.
  The `stripe_tax` mode sets `automatic_tax`; `managed_payments` sets
  none of these.
- **Free days**: supported, with or without a card.
- **Downgrades**: subscription schedules are supported.
- **The card on file** cannot be set through the API; the customer
  changes it in the Customer Portal or on Link. P9 already sends them
  to the portal.
- **Customers manage their orders on Link too**: they can cancel or
  change a subscription and change the card there, outside our pages.
  Our copy already follows the webhooks wherever a change is made;
  the pages must never assume a change came from them.
- **Stripe may refund without us**, within 60 days, if we do not
  answer one of its questions in 48 hours, and it answers disputes.
  `charge.refunded` already handles a refund we did not make.
- **A data deletion request** to Stripe cancels the customer's
  subscriptions: `customer.subscription.deleted`, handled as any end.
- **Receipts and invoices** are Link's, "Sold through Link", with our
  business name. The card statement reads `LINK.COM* <ours>`. In the
  sandbox no receipt is sent unless sent by hand.
- **Digital products only**: software, SaaS, digital media, courses,
  hosting. Not physical goods, and not services with a person in them,
  such as coaching. Each product needs an eligible tax code; the
  placeholders take SaaS, `txcd_10103000`.
- **Who can use it**: a business in one of 39 countries, the US,
  Canada, most of Europe, Australia, Hong Kong, Japan, Singapore,
  after Stripe reviews the account. Not Connect.

Not stated in the docs, to try in the sandbox before step 5:

1. Changing a Managed Payments subscription's price, prorated, and
   setting `cancel_at_period_end`. The docs say subscriptions are
   updated "in the Dashboard or with the API" without listing these.
2. What the Customer Portal offers on a Managed Payments subscription.
3. Whether subscription and invoice webhooks differ from plain
   Stripe's. Only the checkout events are named.

Steps 1–4 depend on none of these.

## Left out

- A cart, by choice.
- Read-only while `past_due`.
- Our own pages for the card and invoices, in place of the portal.
- Coupons and promotion codes; Stripe's page can take them with one
  flag when wanted.
- The public website's pricing page from the catalog.
- Ops: a user's purchases and plan, refunds from there.
