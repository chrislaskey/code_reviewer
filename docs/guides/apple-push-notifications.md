# Setting up Apple push notifications

How to let the Mac's server send notifications to the iOS app, for anyone
who hasn't set up Apple Push Notification service (APNs) before. It's done
once per Apple developer team. See `docs/plans/notifications.md` for why
the phone needs APNs.

The app stays on APNs' **sandbox** for good: development builds installed
with `bin/reinstall` (or run in the simulator) get sandbox tokens, and
no App Store or TestFlight release is needed.

## What you need

- A paid Apple Developer Program membership. Free "Personal Team"
  accounts can't use push.
- Access to [Certificates, Identifiers & Profiles](https://developer.apple.com/account/resources).

## 1. Turn on push for the app's identifier

1. Open **Identifiers** and pick `com.chrislaskey.codereviewer`. If it
   isn't listed, add it with **+** → **App IDs** → **App**, using that
   bundle ID exactly.
2. Under **Capabilities**, tick **Push Notifications** and save.
   Ignore its **Configure** button: that makes certificates, and this app
   uses a key (step 2) instead.

## 2. Create an APNs key

1. Open **Keys** and press **+**.
2. Name it, such as "Code Reviewer push (sandbox)", and tick **Apple Push
   Notifications service (APNs)**.
3. Under **Configure**, choose the **Sandbox** environment and a **Team
   Scoped** key, then save, **Continue** and **Register**.
4. Note the **Key ID** (10 characters) on the confirmation page.
5. Press **Download**. **The `.p8` file can be downloaded only once**; if
   it's lost, revoke the key and make a new one.
6. Note your **Team ID**: it's under **Membership details** in the account,
   and is the same as `APPLE_DEVELOPMENT_TEAM` in `.env`.

## 3. Put the key where the server reads it

The server runs inside the desktop app, whichever checkout or worktree it
was built from, so the key lives in the app's data directory with the
other private files (paired devices, tokens), not in a checkout:

```sh
DATA="$HOME/Library/Application Support/com.chrislaskey.codereviewer"
mv ~/Downloads/AuthKey_<KEY ID>.p8 "$DATA/apple_push_notifications_key.p8"
chmod 600 "$DATA/apple_push_notifications_key.p8"
```

Then write the key ID and team ID beside it, in
`$DATA/apple_push_notifications.json`:

```json
{ "key_id": "<KEY ID>", "team_id": "<TEAM ID>" }
```

The key signs requests as your team, so treat it like a password: never
commit it, paste it into a chat, or copy it into the repository. The
three values aren't useful apart, but keep the key ID and team ID out of
the repository too.

## 4. The app's entitlement and profile

The iOS app asks for push with the `aps-environment: development`
entitlement in
`tauri/src-tauri/gen/apple/code_reviewer_iOS/code_reviewer_iOS.entitlements`.
Xcode's automatic signing makes a new provisioning profile that includes
push the next time `bin/reinstall` builds. If the app logs "no valid
aps-environment entitlement", delete the old profile in Xcode → Settings →
Accounts → Download Manual Profiles, and build again.

## 5. Check it by hand

Once the app is installed, it logs its device token at launch.

- **On the phone or simulator, through Apple:** open the
  [Push Notifications Console](https://icloud.developer.apple.com/dashboard/notifications),
  pick the `com.chrislaskey.codereviewer` bundle and the **Development**
  environment, paste the device token and send. A notification should
  appear even with the phone locked.
- **In the simulator, without Apple:** this shows how the app displays a
  push, but doesn't check the key or token:

  ```sh
  echo '{"aps":{"alert":{"title":"cr-test","body":"Finished"}}}' > push.apns
  xcrun simctl push booted com.chrislaskey.codereviewer push.apns
  ```

## When it doesn't work

| APNs says | Usually means |
| --- | --- |
| `BadDeviceToken` | A sandbox token sent to the production host, or the other way round |
| `InvalidProviderToken` | The key ID or team ID doesn't match the key |
| `TopicDisallowed` | Push isn't on for the bundle ID, or the key belongs to another team |
| `Unregistered` | The app was deleted; the token is no longer valid |
| `ExpiredProviderToken` | The signed token is over an hour old; the server makes a new one |

Device tokens change when the app is reinstalled, so the app sends its
token every time it starts, not only when it's paired.
