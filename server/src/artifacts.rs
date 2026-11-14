//! Artifacts: files made while working in a checkout, such as screenshots
//! and page previews, kept in the app's main checkout under
//! `artifacts/<project>/<worktree>/` and served by `bin/dev` (see
//! `artifacts/README.md`). `bin/dev` tells the app where they are, and only
//! when it runs in the main checkout, so without it there are none.

use std::path::{Path, PathBuf};

use crate::git_worktree::find_checkout;

/// The variable `bin/dev` sets to the artifacts directory.
const DIRECTORY_VARIABLE: &str = "CODE_REVIEWER_ARTIFACTS_DIRECTORY";
/// The variable `bin/dev` sets to the port its dev server serves them on.
const PORT_VARIABLE: &str = "CODE_REVIEWER_ARTIFACTS_PORT";
/// The folder of the work done in a main checkout, not a linked worktree.
const MAIN_CHECKOUT_FOLDER: &str = "main";
/// The folder in a checkout's artifacts that images pasted into a prompt go in.
const PROMPT_IMAGES_FOLDER: &str = "prompt-images";
/// How many names a new prompt image tries before giving up.
const MAX_PROMPT_IMAGE_NAME_ATTEMPTS: u32 = 100;

/// Where the artifacts are kept and served.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactsServer {
    pub directory: PathBuf,
    pub port: u16,
}

impl ArtifactsServer {
    /// The artifacts `bin/dev` set up, or `None` when it didn't start the app.
    pub fn from_environment() -> Option<Self> {
        let directory = std::env::var_os(DIRECTORY_VARIABLE)?;
        let port = std::env::var(PORT_VARIABLE).ok()?.parse().ok()?;
        Some(Self {
            directory: PathBuf::from(directory),
            port,
        })
    }

    /// The URL of a folder's index on this computer.
    pub fn localhost_url(&self, folder: &ArtifactsFolder) -> String {
        format!(
            "http://localhost:{}/artifacts/{}/{}/",
            self.port, folder.project, folder.worktree
        )
    }

    /// Deletes a folder's artifacts, when it has any.
    pub async fn delete_folder(&self, folder: &ArtifactsFolder) -> Result<(), String> {
        let path = self.directory.join(&folder.project).join(&folder.worktree);
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not delete {}: {error}", path.display())),
        }
    }

    /// Saves an image pasted into a prompt in a folder's `prompt-images/`,
    /// under a new name, and returns its path.
    pub async fn save_prompt_image(
        &self,
        folder: &ArtifactsFolder,
        media_type: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, String> {
        let extension = prompt_image_extension(media_type)?;
        let directory = self
            .directory
            .join(&folder.project)
            .join(&folder.worktree)
            .join(PROMPT_IMAGES_FOLDER);
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
        let stem = format!("image-{}", chrono::Local::now().format("%Y%m%d-%H%M%S-%3f"));
        for attempt in 1..=MAX_PROMPT_IMAGE_NAME_ATTEMPTS {
            let file_name = match attempt {
                1 => format!("{stem}.{extension}"),
                _ => format!("{stem}-{attempt}.{extension}"),
            };
            let path = directory.join(file_name);
            match write_new_file(&path, bytes).await {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("Could not save {}: {error}", path.display()));
                }
            }
        }
        Err(format!(
            "Could not find a free name for an image in {}",
            directory.display()
        ))
    }
}

/// The file extension of an image type an agent can read.
fn prompt_image_extension(media_type: &str) -> Result<&'static str, String> {
    match media_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        "image/webp" => Ok("webp"),
        _ => Err(format!(
            "Can't attach a {media_type} image: only PNG, JPEG, GIF and WebP are supported"
        )),
    }
}

async fn write_new_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(bytes).await?;
    file.flush().await
}

/// The folder of one checkout's artifacts: its repository's main checkout's
/// name, then the worktree's name, or `main` for the main checkout.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactsFolder {
    pub project: String,
    pub worktree: String,
}

impl ArtifactsFolder {
    /// The folder of the worktree named `worktree` of the repository whose
    /// main checkout is at `main_checkout`.
    pub fn new(main_checkout: &Path, worktree: &str) -> Result<Self, String> {
        let project = main_checkout
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        validate_folder_name(&project)?;
        validate_folder_name(worktree)?;
        Ok(Self {
            project,
            worktree: worktree.to_string(),
        })
    }

    /// The folder of the checkout `directory` is in.
    pub async fn of_directory(directory: &Path) -> Result<Self, String> {
        let checkout = find_checkout(directory).await?;
        if checkout.path == checkout.main_checkout {
            return Self::new(&checkout.main_checkout, MAIN_CHECKOUT_FOLDER);
        }
        let worktree = checkout
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self::new(&checkout.main_checkout, &worktree)
    }
}

/// A name safe as one segment of a path and of a URL.
fn validate_folder_name(name: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if name.is_empty() || name.starts_with('.') || !name.chars().all(allowed) {
        return Err(format!(
            "'{name}' can't name an artifacts folder: only letters, digits, -, _ and . are allowed"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(directory: &Path) -> ArtifactsServer {
        ArtifactsServer {
            directory: directory.to_path_buf(),
            port: 1420,
        }
    }

    #[test]
    fn names_the_folder_after_the_project_and_the_worktree() {
        let folder =
            ArtifactsFolder::new(Path::new("/code/code_reviewer"), "cr-mobile-ux").unwrap();

        assert_eq!(folder.project, "code_reviewer");
        assert_eq!(folder.worktree, "cr-mobile-ux");
        assert_eq!(
            server(Path::new("/a")).localhost_url(&folder),
            "http://localhost:1420/artifacts/code_reviewer/cr-mobile-ux/"
        );
    }

    #[test]
    fn refuses_names_that_arent_one_plain_path_segment() {
        let main_checkout = Path::new("/code/code_reviewer");

        assert!(ArtifactsFolder::new(main_checkout, "..").is_err());
        assert!(ArtifactsFolder::new(main_checkout, "a/b").is_err());
        assert!(ArtifactsFolder::new(main_checkout, ".hidden").is_err());
        assert!(ArtifactsFolder::new(main_checkout, "").is_err());
        assert!(ArtifactsFolder::new(Path::new("/my project"), "main").is_err());
    }

    #[tokio::test]
    async fn deletes_only_the_folders_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let kept = directory.path().join("code_reviewer/cr-other");
        let deleted = directory.path().join("code_reviewer/cr-done");
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::create_dir_all(&deleted).unwrap();
        std::fs::write(deleted.join("shot.png"), "png").unwrap();
        let folder = ArtifactsFolder::new(Path::new("/code/code_reviewer"), "cr-done").unwrap();

        server(directory.path())
            .delete_folder(&folder)
            .await
            .unwrap();
        server(directory.path())
            .delete_folder(&folder)
            .await
            .unwrap();

        assert!(!deleted.exists());
        assert!(kept.exists());
    }

    #[tokio::test]
    async fn saves_prompt_images_under_new_names() {
        let directory = tempfile::tempdir().unwrap();
        let folder = ArtifactsFolder::new(Path::new("/code/code_reviewer"), "cr-x").unwrap();

        let first = server(directory.path())
            .save_prompt_image(&folder, "image/png", b"one")
            .await
            .unwrap();
        let second = server(directory.path())
            .save_prompt_image(&folder, "image/png", b"two")
            .await
            .unwrap();

        assert_ne!(first, second);
        assert!(first.starts_with(directory.path().join("code_reviewer/cr-x/prompt-images")));
        assert_eq!(first.extension().unwrap(), "png");
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
    }

    #[tokio::test]
    async fn refuses_prompt_images_of_other_types() {
        let directory = tempfile::tempdir().unwrap();
        let folder = ArtifactsFolder::new(Path::new("/code/code_reviewer"), "cr-x").unwrap();

        let saved = server(directory.path())
            .save_prompt_image(&folder, "image/svg+xml", b"<svg/>")
            .await;

        assert!(saved.is_err());
        assert!(!directory.path().join("code_reviewer").exists());
    }

    #[tokio::test]
    async fn the_main_checkout_has_the_main_folder() {
        let directory = tempfile::tempdir().unwrap();
        let repository = directory.path().join("project");
        std::fs::create_dir_all(&repository).unwrap();
        let initialized = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repository)
            .status()
            .unwrap();
        assert!(initialized.success());

        let folder = ArtifactsFolder::of_directory(&repository).await.unwrap();

        assert_eq!(folder.project, "project");
        assert_eq!(folder.worktree, "main");
    }
}
