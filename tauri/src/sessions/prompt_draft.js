// A session's unsent prompt, kept in this client's localStorage so leaving
// the page doesn't lose it. Never sent anywhere; one draft per session.

const KEY_PREFIX = "session-prompt-draft:";

export function loadPromptDraft(sessionName) {
  try {
    return window.localStorage.getItem(KEY_PREFIX + sessionName) ?? "";
  } catch {
    return "";
  }
}

/** Keeps `prompt`, or forgets the draft when it is empty. */
export function savePromptDraft(sessionName, prompt) {
  try {
    if (prompt === "") window.localStorage.removeItem(KEY_PREFIX + sessionName);
    else window.localStorage.setItem(KEY_PREFIX + sessionName, prompt);
  } catch {
    // Keeping the draft is a convenience only.
  }
}
