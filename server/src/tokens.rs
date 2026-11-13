//! The tokens clients send in `Authorization: Bearer <token>`. Each lives
//! in a file only the user can read, so reading the file is as hard as
//! running code as the user.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

use crate::start::ServerError;

const TOKEN_BYTE_COUNT: usize = 32;

/// The token in the file at `path`, written there first with a new random
/// token when there is no such file.
pub fn read_or_create_token_file(path: &Path) -> Result<SecretString, ServerError> {
    match create_token_file(path) {
        Ok(token) => Ok(token),
        Err(ServerError::Io { source, .. }) if source.kind() == ErrorKind::AlreadyExists => {
            read_token_file(path)
        }
        Err(error) => Err(error),
    }
}

/// Whether `sent` is `expected`, compared in constant time.
pub fn tokens_match(sent: &str, expected: &SecretString) -> bool {
    sent.as_bytes()
        .ct_eq(expected.expose_secret().as_bytes())
        .into()
}

fn create_token_file(path: &Path) -> Result<SecretString, ServerError> {
    if let Some(directory) = path.parent() {
        create_private_directory(directory)?;
    }
    let token = create_token()?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(path)
        .map_err(|source| ServerError::io(path, source))?;
    file.write_all(token.expose_secret().as_bytes())
        .map_err(|source| ServerError::io(path, source))?;
    Ok(token)
}

fn read_token_file(path: &Path) -> Result<SecretString, ServerError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|source| ServerError::io(path, source))?;
    }
    let token = fs::read_to_string(path).map_err(|source| ServerError::io(path, source))?;
    let token = token.trim();
    if token.len() != TOKEN_BYTE_COUNT * 2 {
        return Err(ServerError::MalformedTokenFile(path.to_path_buf()));
    }
    Ok(SecretString::from(token))
}

fn create_private_directory(directory: &Path) -> Result<(), ServerError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(directory)
        .map_err(|source| ServerError::io(directory, source))
}

/// A new random token: 32 bytes, as 64 hex digits.
pub(crate) fn create_token() -> Result<SecretString, ServerError> {
    let mut bytes = [0u8; TOKEN_BYTE_COUNT];
    getrandom::fill(&mut bytes).map_err(|error| ServerError::Random(error.to_string()))?;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(SecretString::from(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_token_file_is_created_once_and_read_after() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested").join("desktop_token");

        let created = read_or_create_token_file(&path).unwrap();
        let read = read_or_create_token_file(&path).unwrap();

        assert_eq!(created.expose_secret().len(), TOKEN_BYTE_COUNT * 2);
        assert_eq!(created.expose_secret(), read.expose_secret());
    }

    #[cfg(unix)]
    #[test]
    fn a_token_file_is_readable_only_by_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("desktop_token");

        read_or_create_token_file(&path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn two_new_tokens_differ() {
        let first = create_token().unwrap();
        let second = create_token().unwrap();

        assert_ne!(first.expose_secret(), second.expose_secret());
    }

    #[test]
    fn a_malformed_token_file_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("desktop_token");
        fs::write(&path, "short").unwrap();

        let result = read_or_create_token_file(&path);

        assert!(matches!(result, Err(ServerError::MalformedTokenFile(_))));
    }

    #[test]
    fn tokens_match_only_when_equal() {
        let expected = SecretString::from("abc123");

        assert!(tokens_match("abc123", &expected));
        assert!(!tokens_match("abc124", &expected));
        assert!(!tokens_match("abc12", &expected));
        assert!(!tokens_match("", &expected));
    }
}
