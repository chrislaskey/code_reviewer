//! Creates the git worktree a session works in: `<repository>/.worktrees/<name>`
//! on a branch named `<name>`, with the user's git.

use std::path::{Path, PathBuf};

use tokio::process::Command;

/// Where the worktrees go, under the repository's root. Added to the
/// repository's local excludes so they never show as untracked.
const WORKTREES_DIRECTORY: &str = ".worktrees";

#[derive(Clone, Debug, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    /// Whether the branch was created, as opposed to checked out because it
    /// already existed.
    pub branch_created: bool,
}

/// The root of the git repository `directory` is in.
pub async fn find_repository_root(directory: &Path) -> Result<PathBuf, String> {
    let output = run_git(directory, &["rev-parse", "--show-toplevel"]).await?;
    match output {
        Ok(stdout) => Ok(PathBuf::from(stdout.trim())),
        Err(_) => Err(format!(
            "{} is not in a git repository",
            directory.display()
        )),
    }
}

/// Adds the worktree `<root>/.worktrees/<name>` on the branch `name`,
/// created from the repository's current HEAD unless it already exists.
pub async fn add_worktree(root: &Path, name: &str) -> Result<Worktree, String> {
    if run_git(root, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .await?
        .is_err()
    {
        return Err(format!(
            "{} has no commits yet, so there is nothing to start a branch from",
            root.display()
        ));
    }

    let path = root.join(WORKTREES_DIRECTORY).join(name);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let path_text = path.display().to_string();

    let branch_exists = run_git(
        root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ],
    )
    .await?
    .is_ok();
    let args: Vec<&str> = if branch_exists {
        vec!["worktree", "add", "--", &path_text, name]
    } else {
        vec!["worktree", "add", "-b", name, "--", &path_text]
    };
    run_git(root, &args)
        .await?
        .map_err(|stderr| format!("git worktree add failed: {stderr}"))?;
    exclude_worktrees_directory(root).await;

    Ok(Worktree {
        path,
        branch: name.to_string(),
        branch_created: !branch_exists,
    })
}

/// Appends `/.worktrees/` to the repository's `.git/info/exclude` unless git
/// already ignores it. Local to the clone, so nothing to commit. Failing
/// here only means the directory shows as untracked, so errors are logged.
async fn exclude_worktrees_directory(root: &Path) {
    let ignored = run_git(root, &["check-ignore", "--quiet", WORKTREES_DIRECTORY])
        .await
        .is_ok_and(|result| result.is_ok());
    if ignored {
        return;
    }
    let exclude_file = match run_git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await
    {
        Ok(Ok(common_dir)) => Path::new(common_dir.trim()).join("info/exclude"),
        Ok(Err(error)) | Err(error) => {
            log::warn!(
                "Could not find the git directory of {}: {error}",
                root.display()
            );
            return;
        }
    };

    let existing = tokio::fs::read_to_string(&exclude_file)
        .await
        .unwrap_or_default();
    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let line = format!("{separator}/{WORKTREES_DIRECTORY}/\n");
    if let Some(parent) = exclude_file.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut contents = existing;
    contents.push_str(&line);
    if let Err(error) = tokio::fs::write(&exclude_file, contents).await {
        log::warn!("Could not write {}: {error}", exclude_file.display());
    }
}

/// Runs git in `directory`. The outer error is git not running at all; the
/// inner result is its stdout on success or its stderr on failure.
async fn run_git(directory: &Path, args: &[&str]) -> Result<Result<String, String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .await
        .map_err(|error| format!("Could not run git: {error}"))?;
    if output.status.success() {
        Ok(Ok(String::from_utf8_lossy(&output.stdout).into_owned()))
    } else {
        Ok(Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository in a temporary directory, removed when the test ends.
    struct TestRepository {
        root: PathBuf,
    }

    impl TestRepository {
        async fn init(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "code-reviewer-worktree-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("the directory is created");
            git(&root, &["init", "-q", "-b", "main"]).await;
            git(&root, &["config", "user.email", "test@example.com"]).await;
            git(&root, &["config", "user.name", "Test"]).await;
            Self { root }
        }

        async fn commit(&self) {
            std::fs::write(self.root.join("README.md"), "").expect("the file is written");
            git(&self.root, &["add", "README.md"]).await;
            git(&self.root, &["commit", "-q", "-m", "Initial commit"]).await;
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    async fn git(root: &Path, args: &[&str]) -> String {
        run_git(root, args)
            .await
            .expect("git runs")
            .unwrap_or_else(|stderr| panic!("git {args:?} failed: {stderr}"))
    }

    #[tokio::test]
    async fn finds_the_repository_root() {
        let repository = TestRepository::init("root").await;
        let nested = repository.root.join("a/b");
        std::fs::create_dir_all(&nested).expect("the directories are created");

        let root = find_repository_root(&nested)
            .await
            .expect("the root is found");
        let outside = find_repository_root(&std::env::temp_dir()).await;

        assert_eq!(
            root.canonicalize().ok(),
            repository.root.canonicalize().ok()
        );
        assert!(outside.is_err_and(|error| error.contains("not in a git repository")));
    }

    #[tokio::test]
    async fn adds_a_worktree_on_a_new_or_existing_branch() {
        let repository = TestRepository::init("add").await;
        repository.commit().await;
        git(&repository.root, &["branch", "existing"]).await;

        let created = add_worktree(&repository.root, "feature")
            .await
            .expect("the worktree is added");
        let checked_out = add_worktree(&repository.root, "existing")
            .await
            .expect("the worktree is added");
        let again = add_worktree(&repository.root, "feature").await;
        let worktrees = git(&repository.root, &["worktree", "list"]).await;
        let status = git(&repository.root, &["status", "--porcelain"]).await;
        let exclude = std::fs::read_to_string(repository.root.join(".git/info/exclude"))
            .expect("the exclude file exists");

        assert_eq!(created.path, repository.root.join(".worktrees/feature"));
        assert!(created.branch_created);
        assert!(!checked_out.branch_created);
        assert!(created.path.join("README.md").exists());
        assert!(worktrees.contains("[feature]") && worktrees.contains("[existing]"));
        assert!(again.is_err_and(|error| error.contains("already exists")));
        assert_eq!(status, "", "the worktrees directory is ignored");
        assert!(exclude.ends_with("/.worktrees/\n"));
    }

    #[tokio::test]
    async fn refuses_a_repository_without_commits() {
        let repository = TestRepository::init("empty").await;
        let result = add_worktree(&repository.root, "feature").await;
        assert!(result.is_err_and(|error| error.contains("no commits")));
    }
}
