// A file PUT straight to object storage, at the signed URL the server
// answered, never through the server. XMLHttpRequest, since fetch reports
// no upload progress. The URL was signed for the file's Content-Type, and
// for a file's checksum, so the bucket refuses anything else.
import { t } from "ttag";

/**
 * @param {{upload_url: string, content_type: string, checksum_sha256?: string}} upload
 * @param {File} file
 * @param {(fraction: number) => void} [onProgress] from 0 to 1
 * @returns {Promise<void>}
 */
export function putFile(upload, file, onProgress) {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest();
    request.open("PUT", upload.upload_url);
    request.setRequestHeader("Content-Type", upload.content_type);
    if (upload.checksum_sha256) request.setRequestHeader("x-amz-checksum-sha256", upload.checksum_sha256);
    request.upload.onprogress = (event) => {
      if (event.lengthComputable && onProgress) onProgress(event.loaded / event.total);
    };
    request.onload = () => {
      if (request.status >= 200 && request.status < 300) resolve();
      else reject(new Error(t`The upload failed with ${request.status}; try again`));
    };
    request.onerror = () => reject(new Error(t`The upload could not reach the storage; try again`));
    request.send(file);
  });
}
