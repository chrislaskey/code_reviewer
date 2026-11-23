// The ops app's API on the web server, under /api/ops, for staff; to
// anyone else it is a 404. The browser sends the session cookie itself.

/**
 * The signed-in person's staff row.
 * @returns {Promise<{user_id: string, inserted_at: string}>}
 */
export async function opsCurrentStaffGet() {
  const response = await fetch("/api/ops/current-staff");
  if (response.status === 404) throw new Error("Log in again as staff to use ops");
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  return response.json();
}
