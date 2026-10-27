//! The git worktrees sessions work in: `<repository>/.worktrees/<name>` on a
//! branch named `<name>`, added with the user's git, found again in the
//! repositories under the watched directories after the tmux sessions are
//! gone, and finished by merging the branch or closing the worktree, as
//! `bin/worktree` does.

use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use serde::Serialize;
use tokio::process::Command;

/// Where the worktrees go, under the repository's root. Added to the
/// repository's local excludes so they never show as untracked.
const WORKTREES_DIRECTORY: &str = ".worktrees";

#[derive(Clone, Debug, PartialEq)]
pub struct AddedWorktree {
    pub path: PathBuf,
    pub branch: String,
    /// Whether the branch was created, as opposed to checked out because it
    /// already existed.
    pub branch_created: bool,
}

/// A worktree as `git worktree list` reports it, other than the main
/// checkout.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Worktree {
    /// The directory's name, which for the app's worktrees is the tmux
    /// session's name too.
    pub name: String,
    pub path: String,
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    /// The repository's main checkout.
    pub repository: String,
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

/// Where `add_worktree` puts the worktree named `name` of the repository at
/// `root`.
pub fn worktree_path(root: &Path, name: &str) -> PathBuf {
    root.join(WORKTREES_DIRECTORY).join(name)
}

/// Adds the worktree `<root>/.worktrees/<name>` on the branch `name`,
/// created from the repository's current HEAD unless it already exists.
pub async fn add_worktree(root: &Path, name: &str) -> Result<AddedWorktree, String> {
    if run_git(root, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .await?
        .is_err()
    {
        return Err(format!(
            "{} has no commits yet, so there is nothing to start a branch from",
            root.display()
        ));
    }

    let path = worktree_path(root, name);
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

    Ok(AddedWorktree {
        path,
        branch: name.to_string(),
        branch_created: !branch_exists,
    })
}

/// Merges the worktree's branch into `main_branch` in the main checkout, a
/// fast-forward only so `main_branch` never gets a merge commit or a
/// conflict, then removes the worktree and deletes the branch. Refuses a
/// worktree with uncommitted changes, which would be lost, and a main
/// checkout that doesn't have `main_branch` checked out.
pub async fn merge_worktree(worktree: &Worktree, main_branch: &str) -> Result<(), String> {
    let branch = refuse_unless_finishable(worktree, main_branch).await?;
    let repository = Path::new(&worktree.repository);
    let checked_out = run_git(repository, &["branch", "--show-current"])
        .await?
        .map(|stdout| stdout.trim().to_string())
        .unwrap_or_default();
    if checked_out != main_branch {
        return Err(format!(
            "{} must have {main_branch} checked out, it has {}",
            worktree.repository,
            if checked_out.is_empty() {
                "a detached HEAD"
            } else {
                &checked_out
            }
        ));
    }

    run_git(repository, &["merge", "--ff-only", "--", &branch])
        .await?
        .map_err(|_| {
            format!(
                "{branch} cannot fast-forward {main_branch}, run `git rebase {main_branch}` in the worktree first"
            )
        })?;
    remove_worktree(worktree).await?;
    run_git(repository, &["branch", "-d", "--", &branch])
        .await?
        .map(|_| ())
        .map_err(|stderr| format!("git branch -d failed: {stderr}"))
}

/// Removes the worktree, keeping its branch for `git switch` in the main
/// checkout later. Refuses a worktree with uncommitted changes.
pub async fn close_worktree(worktree: &Worktree, main_branch: &str) -> Result<(), String> {
    refuse_unless_finishable(worktree, main_branch).await?;
    remove_worktree(worktree).await
}

/// The worktree's branch, when it is a clean worktree on a branch of its
/// own, as `bin/worktree` requires.
async fn refuse_unless_finishable(
    worktree: &Worktree,
    main_branch: &str,
) -> Result<String, String> {
    let path = Path::new(&worktree.path);
    let Some(branch) = &worktree.branch else {
        return Err(format!("No branch is checked out in {}", worktree.path));
    };
    if branch == main_branch {
        return Err(format!("{} is on {main_branch} itself", worktree.path));
    }
    let status = run_git(path, &["status", "--porcelain"])
        .await?
        .map_err(|stderr| format!("git status failed: {stderr}"))?;
    if !status.trim().is_empty() {
        return Err(format!(
            "{} has uncommitted changes, commit or remove them first",
            worktree.path
        ));
    }
    Ok(branch.clone())
}

/// Ignored files in the worktree, like build output, go with it.
async fn remove_worktree(worktree: &Worktree) -> Result<(), String> {
    run_git(
        Path::new(&worktree.repository),
        &["worktree", "remove", "--", &worktree.path],
    )
    .await?
    .map(|_| ())
    .map_err(|stderr| format!("git worktree remove failed: {stderr}"))
}

/// The worktrees of the repositories at `directories` and in their
/// first-level subdirectories, main checkouts left out. Directories that
/// can't be read are skipped.
pub async fn list_worktrees(directories: &[PathBuf]) -> Vec<Worktree> {
    let repositories = find_repositories(directories).await;
    let listed = join_all(
        repositories
            .iter()
            .map(|repository| list_repository_worktrees(repository)),
    )
    .await;

    let mut worktrees: Vec<Worktree> = listed.into_iter().flatten().collect();
    worktrees.sort_by(|a, b| a.path.cmp(&b.path));
    worktrees.dedup_by(|a, b| a.path == b.path);
    worktrees
}

/// `directories` and their first-level subdirectories that have a `.git`
/// entry (a directory, or a file in worktrees). Hidden subdirectories are
/// skipped.
async fn find_repositories(directories: &[PathBuf]) -> Vec<PathBuf> {
    let mut repositories = Vec::new();
    for directory in directories {
        if directory.join(".git").exists() {
            repositories.push(directory.clone());
        }
        let Ok(mut entries) = tokio::fs::read_dir(directory).await else {
            continue;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if !hidden && path.join(".git").exists() {
                repositories.push(path);
            }
        }
    }
    repositories
}

async fn list_repository_worktrees(repository: &Path) -> Vec<Worktree> {
    match run_git(repository, &["worktree", "list", "--porcelain"]).await {
        Ok(Ok(stdout)) => parse_worktree_list(&stdout),
        Ok(Err(stderr)) | Err(stderr) => {
            log::warn!(
                "Could not list the worktrees of {}: {stderr}",
                repository.display()
            );
            Vec::new()
        }
    }
}

/// Parses `git worktree list --porcelain`: one block per worktree, the main
/// checkout first, each with `worktree <path>`, `HEAD <commit>` and either
/// `branch refs/heads/<name>` or `detached`.
fn parse_worktree_list(stdout: &str) -> Vec<Worktree> {
    let mut blocks = stdout
        .split("\n\n")
        .filter(|block| !block.trim().is_empty());
    let Some(repository) = blocks.next().and_then(worktree_path_of) else {
        return Vec::new();
    };

    blocks
        .filter_map(|block| {
            let path = worktree_path_of(block)?;
            let name = Path::new(&path).file_name()?.to_string_lossy().into_owned();
            let branch = block
                .lines()
                .find_map(|line| line.strip_prefix("branch "))
                .map(|reference| {
                    reference
                        .strip_prefix("refs/heads/")
                        .unwrap_or(reference)
                        .to_string()
                });
            Some(Worktree {
                name,
                path,
                branch,
                repository: repository.clone(),
            })
        })
        .collect()
}

fn worktree_path_of(block: &str) -> Option<String> {
    block
        .lines()
        .find_map(|line| line.strip_prefix("worktree "))
        .map(str::to_string)
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

    /// A directory in the temporary directory, removed when the test ends.
    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn create(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "code-reviewer-worktree-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("the directory is created");
            Self { path }
        }

        /// A repository with one commit in `name` under this directory.
        async fn init_repository(&self, name: &str) -> PathBuf {
            let root = self.path.join(name);
            std::fs::create_dir_all(&root).expect("the directory is created");
            git(&root, &["init", "-q", "-b", "main"]).await;
            git(&root, &["config", "user.email", "test@example.com"]).await;
            git(&root, &["config", "user.name", "Test"]).await;
            std::fs::write(root.join("README.md"), "").expect("the file is written");
            git(&root, &["add", "README.md"]).await;
            git(&root, &["commit", "-q", "-m", "Initial commit"]).await;
            root
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
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
        let directory = TestDirectory::create("root");
        let repository = directory.init_repository("repo").await;
        let nested = repository.join("a/b");
        std::fs::create_dir_all(&nested).expect("the directories are created");

        let root = find_repository_root(&nested)
            .await
            .expect("the root is found");
        let outside = find_repository_root(&directory.path).await;

        assert_eq!(root.canonicalize().ok(), repository.canonicalize().ok());
        assert!(outside.is_err_and(|error| error.contains("not in a git repository")));
    }

    #[tokio::test]
    async fn adds_a_worktree_on_a_new_or_existing_branch() {
        let directory = TestDirectory::create("add");
        let repository = directory.init_repository("repo").await;
        git(&repository, &["branch", "cr-existing"]).await;

        let created = add_worktree(&repository, "cr-feature")
            .await
            .expect("the worktree is added");
        let checked_out = add_worktree(&repository, "cr-existing")
            .await
            .expect("the worktree is added");
        let again = add_worktree(&repository, "cr-feature").await;
        let worktrees = git(&repository, &["worktree", "list"]).await;
        let status = git(&repository, &["status", "--porcelain"]).await;
        let exclude = std::fs::read_to_string(repository.join(".git/info/exclude"))
            .expect("the exclude file exists");

        assert_eq!(created.path, repository.join(".worktrees/cr-feature"));
        assert!(created.branch_created);
        assert!(!checked_out.branch_created);
        assert!(created.path.join("README.md").exists());
        assert!(worktrees.contains("[cr-feature]") && worktrees.contains("[cr-existing]"));
        assert!(again.is_err_and(|error| error.contains("already exists")));
        assert_eq!(status, "", "the worktrees directory is ignored");
        assert!(exclude.ends_with("/.worktrees/\n"));
    }

    #[tokio::test]
    async fn refuses_a_repository_without_commits() {
        let directory = TestDirectory::create("empty");
        let root = directory.path.join("repo");
        std::fs::create_dir_all(&root).expect("the directory is created");
        git(&root, &["init", "-q"]).await;

        let result = add_worktree(&root, "cr-feature").await;
        assert!(result.is_err_and(|error| error.contains("no commits")));
    }

    #[tokio::test]
    async fn lists_the_worktrees_of_the_repositories_in_watched_directories() {
        let directory = TestDirectory::create("list");
        let repository = directory.init_repository("repo").await;
        std::fs::create_dir_all(directory.path.join("not-a-repo")).expect("created");
        add_worktree(&repository, "cr-feature")
            .await
            .expect("the worktree is added");

        let worktrees = list_worktrees(std::slice::from_ref(&directory.path)).await;

        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].name, "cr-feature");
        assert_eq!(worktrees[0].branch.as_deref(), Some("cr-feature"));
        assert_eq!(
            Path::new(&worktrees[0].path).canonicalize().ok(),
            repository.join(".worktrees/cr-feature").canonicalize().ok()
        );
        assert_eq!(
            Path::new(&worktrees[0].repository).canonicalize().ok(),
            repository.canonicalize().ok()
        );
    }

    async fn worktree_named(repository: &Path, name: &str) -> Worktree {
        list_worktrees(std::slice::from_ref(&repository.to_path_buf()))
            .await
            .into_iter()
            .find(|worktree| worktree.name == name)
            .unwrap_or_else(|| panic!("{name} is listed"))
    }

    #[tokio::test]
    async fn merges_a_worktrees_branch_and_removes_it() {
        let directory = TestDirectory::create("merge");
        let repository = directory.init_repository("repo").await;
        let added = add_worktree(&repository, "cr-done")
            .await
            .expect("the worktree is added");
        std::fs::write(added.path.join("work.txt"), "done").expect("the file is written");
        git(&added.path, &["add", "work.txt"]).await;
        git(&added.path, &["commit", "-q", "-m", "Work"]).await;
        let worktree = worktree_named(&directory.path, "cr-done").await;

        merge_worktree(&worktree, "main")
            .await
            .expect("the branch is merged");
        let latest_subject = git(&repository, &["log", "-1", "--format=%s", "main"]).await;
        let branches = git(&repository, &["branch", "--list", "cr-done"]).await;

        assert_eq!(latest_subject.trim(), "Work");
        assert!(repository.join("work.txt").exists());
        assert!(!added.path.exists());
        assert_eq!(branches.trim(), "");
    }

    #[tokio::test]
    async fn refuses_to_finish_a_worktree_with_changes_or_behind_main() {
        let directory = TestDirectory::create("refuse");
        let repository = directory.init_repository("repo").await;
        let added = add_worktree(&repository, "cr-dirty")
            .await
            .expect("the worktree is added");
        std::fs::write(added.path.join("new.txt"), "").expect("the file is written");
        let dirty = worktree_named(&directory.path, "cr-dirty").await;

        let close_dirty = close_worktree(&dirty, "main").await;
        std::fs::remove_file(added.path.join("new.txt")).expect("the file is removed");
        std::fs::write(repository.join("ahead.txt"), "").expect("the file is written");
        git(&repository, &["add", "ahead.txt"]).await;
        git(&repository, &["commit", "-q", "-m", "Main moved on"]).await;
        git(
            &added.path,
            &["commit", "-q", "--allow-empty", "-m", "Work"],
        )
        .await;
        let merge_diverged = merge_worktree(&dirty, "main").await;
        let close_clean = close_worktree(&dirty, "main").await;
        let branches = git(&repository, &["branch", "--list", "cr-dirty"]).await;

        assert!(close_dirty.is_err_and(|error| error.contains("uncommitted changes")));
        assert!(merge_diverged.is_err_and(|error| error.contains("cannot fast-forward")));
        assert_eq!(close_clean, Ok(()));
        assert!(!added.path.exists());
        assert!(branches.contains("cr-dirty"), "the branch is kept");
    }

    #[test]
    fn parses_worktree_list_output() {
        let output = "worktree /code/repo\nHEAD abc\nbranch refs/heads/main\n\n\
            worktree /code/repo/.worktrees/cr-feature\nHEAD def\nbranch refs/heads/cr-feature\n\n\
            worktree /code/repo/.worktrees/cr-pinned\nHEAD 123\ndetached\n\n";
        let worktrees = parse_worktree_list(output);

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].name, "cr-feature");
        assert_eq!(worktrees[0].branch.as_deref(), Some("cr-feature"));
        assert_eq!(worktrees[0].repository, "/code/repo");
        assert_eq!(worktrees[1].name, "cr-pinned");
        assert_eq!(worktrees[1].branch, None);
        assert_eq!(parse_worktree_list(""), Vec::new());
    }
}
