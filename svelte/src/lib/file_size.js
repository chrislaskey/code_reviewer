import { t } from "ttag";

/**
 * A size in bytes as a person reads it: "12 KB", "3.4 MB".
 * @param {number} bytes
 */
export function fileSize(bytes) {
  if (bytes < 1024) return t`${bytes} B`;
  const kilobytes = Math.round(bytes / 1024);
  if (bytes < 1024 * 1024) return t`${kilobytes} KB`;
  const megabytes = (bytes / (1024 * 1024)).toFixed(1);
  return t`${megabytes} MB`;
}
