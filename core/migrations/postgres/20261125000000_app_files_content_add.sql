-- A file's content on its row, see docs/design/files-content.md: set, the
-- content is here and nothing is in the bucket; empty, the bytes are in
-- the bucket, as before. `size` and `sha256` are the content's.
ALTER TABLE app_files ADD COLUMN content text;
