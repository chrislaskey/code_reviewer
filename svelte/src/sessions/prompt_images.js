// Images pasted, dropped or picked for a prompt: found in what the browser
// hands over, and read as base64 to send to the backend.

/** The image files in a paste or drop's `DataTransfer`. */
export function imageFilesIn(dataTransfer) {
  const files = Array.from(dataTransfer?.files ?? []);
  if (files.length === 0) {
    for (const item of Array.from(dataTransfer?.items ?? [])) {
      if (item.kind !== "file") continue;
      const file = item.getAsFile();
      if (file) files.push(file);
    }
  }
  return files.flatMap((file) => {
    if (file.type.startsWith("image/")) return [file];
    if (file.type !== "") return [];
    const extension = file.name.split(".").pop()?.toLowerCase();
    const type = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp" }[extension];
    return type ? [new File([file], file.name, { type, lastModified: file.lastModified })] : [];
  });
}

/** A file's bytes as base64, without the `data:` URL prefix. */
export function readFileAsBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",", 2)[1] ?? "");
    reader.onerror = () => reject(reader.error);
    reader.onabort = () => reject(new Error(`Reading ${file.name} was cancelled`));
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
