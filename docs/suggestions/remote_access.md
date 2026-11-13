# Remote access: suggestions

Left out of decision 25 on purpose, or found while building it.

- **Keep the phone's pairing code in the iOS Keychain** rather than a file
  in the app's data directory.
- **Renew the tailnet certificate while the app runs.** It is fetched when
  the listener starts; Tailscale's certificates last 90 days, so an app left
  running that long would serve an expired one.
- **Accept tailnet connections concurrently.** The listener checks each
  connection (`whois`, TLS handshake) before accepting the next, so one slow
  device delays the others.
- **Open the Remote page first on an unpaired phone.** On a phone the Remote
  tab is under More in the bottom bar, and every other page fails until
  pairing.
- **Hide what a phone can't do** (Open in terminal app, Open in browser),
  which the tailnet listener doesn't serve, instead of letting it fail.
- **A standalone server binary** (decision 25's background program): a
  `main.rs` calling `code_reviewer_server::start`, a launchd agent, and the
  desktop app connecting to a running server instead of starting one.
