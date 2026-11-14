// Images pasted, dropped or picked for a prompt: found in what the browser
// hands over, and read as base64 to send to the backend.

/** The image files in a paste or drop's `DataTransfer`. */
export function imageFilesIn(dataTransfer) {
  return Array.from(dataTransfer?.files ?? []).filter((file) => file.type.startsWith("image/"));
}

/** A file's bytes as base64, without the `data:` URL prefix. */
export function readFileAsBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",", 2)[1] ?? "");
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** `prompt` with `text` put in at `position`, set apart by spaces, and the position after it. */
export function insertIntoPrompt(prompt, position, text) {
  const before = prompt.slice(0, position);
  const after = prompt.slice(position);
  const spaceBefore = before === "" || /\s$/.test(before) ? "" : " ";
  const spaceAfter = /^\s/.test(after) ? "" : " ";
  const inserted = spaceBefore + text + spaceAfter;
  return { prompt: before + inserted + after, position: before.length + inserted.length };
}
