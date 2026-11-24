#!/usr/bin/env bash
# Two devices syncing over the HTTP endpoints, with curl: device A
# writes a post, device B pulls it, both edit it from the same version, and
# the edit that reaches the server first stays while the other comes back
# as a conflict. See "Event-sourced resources" in docs/design/offline-sync.md.
#
#   SYNC_URL=http://localhost:4000 SYNC_TOKEN=... SYNC_USER_ID=... \
#   bin/sync-two-devices.sh
#
# The server must run with API_SOCKET=true. The token is an API token of
# the user, see App.Data.Accounts.generate_user_api_token/1.
set -euo pipefail

: "${SYNC_URL:=http://localhost:4000}"
: "${SYNC_TOKEN:?set SYNC_TOKEN to an API token of the user}"
: "${SYNC_USER_ID:?set SYNC_USER_ID to the id of the user}"

sync() {
  curl -sS -X POST "$SYNC_URL/sync/$1" \
    -H "Authorization: Bearer $SYNC_TOKEN" -H "Content-Type: application/json" -d "$2"
}

uuid() { uuidgen | tr '[:upper:]' '[:lower:]'; }

# event <post id> <type> <base version> <base body or empty> <new body>
event() {
  local base='{}'
  if [ -n "$4" ]; then base="{\"body\":\"$4\"}"; fi
  printf '{"events":[{"id":"%s","table":"app_posts","row_id":"%s","type":"%s","base_version":%s,"base":%s,"changes":{"body":"%s"}}]}' \
    "$(uuid)" "$1" "$2" "$3" "$base" "$5"
}

id=$(uuid)

echo "device A writes a post"
sync push "$(event "$id" create 0 "" "from device A")" | grep -o '"status":"applied"' || { echo "the post was not created"; exit 1; }

echo "device B pulls it, at version 1"
sync pull '{}' | grep -o "\"body\":\"from device A\"" || { echo "device B did not receive the post"; exit 1; }

echo "device B edits it from version 1 and syncs first"
sync push "$(event "$id" update 1 "from device A" "edited on B")" | grep -o '"status":"applied"' || { echo "B's edit did not apply"; exit 1; }

echo "device A edits it from version 1 too, and gets a conflict with B's body"
sync push "$(event "$id" update 1 "from device A" "edited on A")" |
  grep -o '"conflicts":{"body":{"server":"edited on B","yours":"edited on A"}}' || { echo "A's edit was not a conflict"; exit 1; }

echo "both pull and match the server"
sync pull '{}' | grep -o "\"body\":\"edited on B\"" >/dev/null && echo "ok: both devices end with 'edited on B', and A holds its edit as a conflict"
