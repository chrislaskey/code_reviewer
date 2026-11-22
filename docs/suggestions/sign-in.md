# Sign-in: what comes next

Written 2026-11-20, after the desktop's loopback and the iPhone's
sign-in sheet were built (`docs/design/accounts-sign-in.md`, "Since
2026-11-20"). In the order suggested; each is its own session, planned
before it is built. Anything run with `fly` is asked for first.

## Still to check

- **The desktop's loopback in a real browser**, Safari and Chrome,
  against `bin/dev` and against `starter-pack.fly.dev`. Chrome's Local
  Network Access rules should leave a top-level navigation to
  `127.0.0.1` alone; that is the part to watch. The iPhone's sheet was
  tried on a phone and works.

## 1. Mail from a domain of our own

The blocker for anyone but the developer: Resend's test sender,
`onboarding@resend.dev`, delivers only to the Resend account's own
address, so nobody else receives a sign-in code.

- Pick the domain the app's mail comes from, a subdomain of the
  personal domain to start, such as `mail.<domain>` or the domain the
  app will run on.
- Add the domain in Resend; it lists the DNS records (SPF, DKIM, and
  a return-path `MX`). Add them in Fastmail, the domain's DNS; the
  GitHub Pages records on the apex are untouched.
- Once Resend says verified, set `MAIL_FROM`, `Starter Pack
  <sign-in@<domain>>`, in `fly.toml`, and deploy.
- Worth deciding at the same time: whether this template moves to the
  personal domain too (a `CNAME` in Fastmail to
  `starter-pack.fly.dev`, `fly certs add`, `SERVER_URL` in `fly.toml`,
  the apps' `SYNC_SERVER_URL`). Not needed for anything below; real
  apps start on their own domain, since an iPhone app's associated
  domains are compiled in.

## 2. Passkeys

A returning sign-in as Face ID in the sheet, in place of waiting for a
code.

- Works on `starter-pack.fly.dev`: the server serves
  `/.well-known/apple-app-site-association` with `webcredentials` and
  the Apple team id, and the app has the Associated Domains
  entitlement `webcredentials:starter-pack.fly.dev`. A move to another
  domain later means an app update.
- In the sheet, `ASWebAuthenticationSession`, the web page's WebAuthn
  call works as in Safari, so the pages do it and the apps need
  nothing beyond the entitlement.
- Server: a WebAuthn library, `webauthn-rs` the likely one, to ask
  about before choosing. A table of each user's passkeys, server only,
  not synced, a migration to ask about.
- Where it sits: a way to sign in beside the emailed code, the email
  first and then "Use a passkey" when the account has one, and made
  from the account page after a sign-in. Whether it also counts as a
  second factor, `accounts::SecondFactor`, to decide.

## 3. Sign in with Apple

One tap on an iPhone. The design left the way ready: a provider proves
the user and calls the same private finish, with the device's
challenge carried in the OAuth `state`, so the apps change nothing.

- Needs, in the Apple developer account: a Services ID, its return
  URL on the server, and a key for the client secret, a secret for
  `fly secrets`.
- A `users_identities` table, provider and the provider's user id per
  user; Apple may hand over a relay email, and whether to link an
  Apple sign-in to an account with the same email is to decide.
- Apple's rule: an iPhone app that offers another provider, Google,
  must offer Sign in with Apple too, so it comes first.

## 4. Magic links

A link in the email beside the code, the same pending sign-in with the
pending token and the code in the URL. With the sheet the code is one
step inside the app already, so this matters less. On a phone, a link
tapped in Mail can open the app through universal links, which
`starter-pack.fly.dev` can serve (`applinks:` in the same association
file as passkeys).

## 5. Android's sign-in sheet

A Custom Tab, as the iPhone's sheet, returning to the app's scheme;
Android pastes the code until then. Whenever Android builds matter.

## Later: choosing a name after signing up

Since 2026-11-20 a sign-up's name is the email's part before the @,
numbered when another user has it (`hello`, `hello1`, ...), and the
person changes it on their profile. Still to plan:

- A step right after the first sign-in that offers to change it, as
  social sites do; it needs a way to tell a first sign-in apart.
- Whether names should be unique everywhere: the profile's name edit
  takes any name today, so two users can still pick the same one.
- `CodeMatched.new_account` now tells a first sign-in apart, at the
  last step, which such a step could use.

## Later: accepting new terms

Since 2026-11-22 the sign-in's last step says "By continuing, you
accept the Terms of Service and Privacy Policy" to everyone, new or
not, and records the version accepted and when on `accounts_users`
(`accounts::TERMS_VERSION`, `terms_version`, `terms_accepted_at`).
Still to plan:

- when an account's version is older than `TERMS_VERSION`, the
  signed-in pages show a blocking "We updated our terms" with an
  explicit Accept, so someone who stays signed in for months is asked
  too;
- a log of acceptances, every version with its time, for a business
  that needs to show who agreed to what and when; the columns keep only
  the latest.

## Later: a trial signing in to an account it already has

A trial user who types the address of an account they already have
signs in to that account (since 2026-11-22), and what they made on the
trial stays with the trial, gone in two days. Moving it into the
account, or asking first, is still to plan.
