// A file's SHA-256, worked out in the page, which has the bytes: the
// bucket checks an upload against it, and a page asks for the files with
// the same bytes by it.

/**
 * The file's SHA-256 as lowercase hex. Needs a secure context: HTTPS,
 * localhost, or the app.
 * @param {Blob} file
 */
export async function sha256Of(file) {
  const digest = await crypto.subtle.digest("SHA-256", await file.arrayBuffer());
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}
