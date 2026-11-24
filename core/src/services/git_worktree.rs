//! The git worktrees sessions work in: `<repository>/.worktrees/<name>` on a
//! branch named `<name>`, added with the user's git, found again in the
//! repositories under the watched directories after the tmux sessions are
//! gone, and finished by merging the branch or closing the worktree, as
//! `bin/worktree` does.

use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::services::git_status::base_branch_config_key;

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
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
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

/// The checkout `directory` is in and its repository's main checkout, the
/// same directory when `directory` isn't in a linked worktree.
#[derive(Clone, Debug, PartialEq)]
pub struct Checkout {
    pub path: PathBuf,
    pub main_checkout: PathBuf,
}

/// The main checkout of the repository `directory` is in, waiting for git
/// on this thread; `None` outside a repository.
pub fn find_main_checkout_blocking(directory: &Path) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let common_directory = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    common_directory.parent().map(Path::to_path_buf)
}

/// The checkout `directory` is in, and its repository's main checkout.
pub async fn find_checkout(directory: &Path) -> Result<Checkout, String> {
    let output = run_git(
        directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ],
    )
    .await?
    .map_err(|_| format!("{} is not in a git repository", directory.display()))?;
    let mut lines = output.lines();
    let (Some(path), Some(common_directory)) = (lines.next(), lines.next()) else {
        return Err(format!(
            "git didn't say which checkout {} is in",
            directory.display()
        ));
    };
    let main_checkout = Path::new(common_directory)
        .parent()
        .ok_or_else(|| format!("{common_directory} has no main checkout"))?;
    Ok(Checkout {
        path: PathBuf::from(path),
        main_checkout: main_checkout.to_path_buf(),
    })
}

/// Where `add_worktree` puts the worktree named `name` of the repository at
/// `root`.
pub fn worktree_path(root: &Path, name: &str) -> PathBuf {
    root.join(WORKTREES_DIRECTORY).join(name)
}

/// Adds the worktree `<root>/.worktrees/<name>` on the branch `name`,
/// created from `base_branch` unless it already exists. A created branch
/// records `base_branch` in git config, for Merge and Review to find.
pub async fn add_worktree(
    root: &Path,
    name: &str,
    base_branch: &str,
) -> Result<AddedWorktree, String> {
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
    if !branch_exists {
        let base_commit = format!("{base_branch}^{{commit}}");
        let base_found = run_git(root, &["rev-parse", "--verify", "--quiet", &base_commit])
            .await?
            .is_ok();
        if base_branch.starts_with('-') || !base_found {
            return Err(format!(
                "{} has no branch named {base_branch} to start from",
                root.display()
            ));
        }
    }
    let args: Vec<&str> = if branch_exists {
        vec!["worktree", "add", "--", &path_text, name]
    } else {
        vec!["worktree", "add", "-b", name, "--", &path_text, base_branch]
    };
    run_git(root, &args)
        .await?
        .map_err(|stderr| format!("git worktree add failed: {stderr}"))?;
    if !branch_exists {
        record_base_branch(root, name, base_branch).await;
    }
    exclude_worktrees_directory(root).await;

    Ok(AddedWorktree {
        path,
        branch: name.to_string(),
        branch_created: !branch_exists,
    })
}

/// Merges the worktree's branch into `into_branch`, a fast-forward only so
/// `into_branch` never gets a merge commit or a conflict, then removes the
/// worktree and deletes the branch. Where `into_branch` is checked out, in
/// the main checkout or another worktree, the merge runs there; elsewhere
/// the branch is moved. Refuses a worktree with uncommitted changes, which
/// would be lost.
pub async fn merge_worktree(worktree: &Worktree, into_branch: &str) -> Result<(), String> {
    let branch = refuse_unless_finishable(worktree, into_branch).await?;
    let repository = Path::new(&worktree.repository);
    let into_reference = format!("refs/heads/{into_branch}");
    let into_commit = run_git(
        repository,
        &["rev-parse", "--verify", "--quiet", &into_reference],
    )
    .await?
    .map(|stdout| stdout.trim().to_string())
    .map_err(|_| format!("{into_branch} is not a local branch to merge into"))?;

    let into_is_ancestor = run_git(
        repository,
        &["merge-base", "--is-ancestor", &into_commit, &branch],
    )
    .await?
    .is_ok();
    if !into_is_ancestor {
        return Err(format!(
            "{branch} cannot fast-forward {into_branch}, run `git rebase {into_branch}` in the worktree first"
        ));
    }

    let checkouts = list_checkouts(repository).await?;
    let into_checkout = checkouts
        .iter()
        .find(|checkout| checkout.branch.as_deref() == Some(into_branch));
    match into_checkout {
        Some(checkout) => run_git(
            Path::new(&checkout.path),
            &["merge", "--ff-only", "--", &branch],
        )
        .await?
        .map(|_| ())
        .map_err(|stderr| format!("Merging {branch} into {into_branch} failed: {stderr}"))?,
        None => run_git(
            repository,
            &[
                "update-ref",
                "-m",
                &format!("merge {branch}: Fast-forward"),
                &into_reference,
                &branch,
                &into_commit,
            ],
        )
        .await?
        .map(|_| ())
        .map_err(|stderr| format!("Moving {into_branch} to {branch} failed: {stderr}"))?,
    }
    remove_worktree(worktree).await?;
    // `-D` since `-d` checks the main checkout's HEAD, which may be another
    // branch; `into_branch` has every commit of `branch` now.
    run_git(repository, &["branch", "-D", "--", &branch])
        .await?
        .map(|_| ())
        .map_err(|stderr| format!("git branch -D failed: {stderr}"))
}

/// Records the branch `branch` was created from, see
/// `base_branch_config_key`. Without it the Git menu only offers the main
/// branch, so a failure is logged.
async fn record_base_branch(root: &Path, branch: &str, base_branch: &str) {
    let key = base_branch_config_key(branch);
    match run_git(root, &["config", &key, base_branch]).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) | Err(error) => {
            tracing::warn!("Could not record {base_branch} as the base of {branch}: {error}");
        }
    }
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
    into_branch: &str,
) -> Result<String, String> {
    let path = Path::new(&worktree.path);
    let Some(branch) = &worktree.branch else {
        return Err(format!("No branch is checked out in {}", worktree.path));
    };
    if branch == into_branch {
        return Err(format!("{} is on {into_branch} itself", worktree.path));
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
            tracing::warn!(
                "Could not list the worktrees of {}: {stderr}",
                repository.display()
            );
            Vec::new()
        }
    }
}

/// A checkout of a repository, its main checkout or a worktree, with the
/// branch it has checked out.
struct ListedCheckout {
    path: String,
    /// `None` when HEAD is detached.
    branch: Option<String>,
}

/// Every checkout of the repository, the main checkout first.
async fn list_checkouts(repository: &Path) -> Result<Vec<ListedCheckout>, String> {
    let stdout = run_git(repository, &["worktree", "list", "--porcelain"])
        .await?
        .map_err(|stderr| format!("git worktree list failed: {stderr}"))?;
    Ok(parse_checkouts(&stdout))
}

/// Parses `git worktree list --porcelain`: one block per checkout, the main
/// checkout first, each with `worktree <path>`, `HEAD <commit>` and either
/// `branch refs/heads/<name>` or `detached`.
fn parse_checkouts(stdout: &str) -> Vec<ListedCheckout> {
    stdout
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .filter_map(|block| {
            let path = worktree_path_of(block)?;
            let branch = block
                .lines()
                .find_map(|line| line.strip_prefix("branch "))
                .map(|reference| {
                    reference
                        .strip_prefix("refs/heads/")
                        .unwrap_or(reference)
                        .to_string()
                });
            Some(ListedCheckout { path, branch })
        })
        .collect()
}

/// The worktrees in `git worktree list --porcelain`, main checkout left out.
fn parse_worktree_list(stdout: &str) -> Vec<Worktree> {
    let mut checkouts = parse_checkouts(stdout).into_iter();
    let Some(repository) = checkouts.next().map(|checkout| checkout.path) else {
        return Vec::new();
    };

    checkouts
        .filter_map(|checkout| {
            let name = Path::new(&checkout.path)
                .file_name()?
                .to_string_lossy()
                .into_owned();
            Some(Worktree {
                name,
                path: checkout.path,
                branch: checkout.branch,
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
            tracing::warn!(
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
        tracing::warn!("Could not write {}: {error}", exclude_file.display());
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

        let created = add_worktree(&repository, "cr-feature", "main")
            .await
            .expect("the worktree is added");
        let checked_out = add_worktree(&repository, "cr-existing", "main")
            .await
            .expect("the worktree is added");
        let again = add_worktree(&repository, "cr-feature", "main").await;
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
    async fn adds_a_worktree_on_a_new_branch_from_the_base_branch() {
        let directory = TestDirectory::create("base");
        let repository = directory.init_repository("repo").await;
        git(&repository, &["checkout", "-q", "-b", "release"]).await;
        git(
            &repository,
            &["commit", "-q", "--allow-empty", "-m", "Release commit"],
        )
        .await;
        git(&repository, &["checkout", "-q", "main"]).await;

        let added = add_worktree(&repository, "cr-fix", "release")
            .await
            .expect("the worktree is added");
        let subject = git(&added.path, &["log", "-1", "--format=%s"]).await;
        let missing = add_worktree(&repository, "cr-other", "no-such-branch").await;

        let recorded = git(
            &repository,
            &["config", "--get", "branch.cr-fix.codeReviewerBaseBranch"],
        )
        .await;

        assert_eq!(subject.trim(), "Release commit");
        assert_eq!(recorded.trim(), "release");
        assert!(missing.is_err_and(|error| error.contains("no branch named no-such-branch")));
    }

    #[tokio::test]
    async fn merges_into_a_branch_checked_out_nowhere_or_in_another_worktree() {
        let directory = TestDirectory::create("merge-base");
        let repository = directory.init_repository("repo").await;
        git(&repository, &["branch", "release"]).await;
        let parent = add_worktree(&repository, "cr-parent", "main")
            .await
            .expect("the worktree is added");
        let onto_release = add_worktree(&repository, "cr-onto-release", "release")
            .await
            .expect("the worktree is added");
        let onto_parent = add_worktree(&repository, "cr-onto-parent", "cr-parent")
            .await
            .expect("the worktree is added");
        for path in [&onto_release.path, &onto_parent.path] {
            git(path, &["commit", "-q", "--allow-empty", "-m", "Work"]).await;
        }

        merge_worktree(
            &worktree_named(&directory.path, "cr-onto-release").await,
            "release",
        )
        .await
        .expect("release is moved");
        merge_worktree(
            &worktree_named(&directory.path, "cr-onto-parent").await,
            "cr-parent",
        )
        .await
        .expect("the parent worktree is fast-forwarded");
        let release_subject = git(&repository, &["log", "-1", "--format=%s", "release"]).await;
        let parent_subject = git(&parent.path, &["log", "-1", "--format=%s"]).await;
        let parent_status = git(&parent.path, &["status", "--porcelain"]).await;
        let main_subject = git(&repository, &["log", "-1", "--format=%s", "main"]).await;
        let branches = git(&repository, &["branch", "--list", "cr-onto-*"]).await;

        assert_eq!(release_subject.trim(), "Work");
        assert_eq!(parent_subject.trim(), "Work");
        assert_eq!(parent_status, "", "the parent's files moved with it");
        assert_eq!(main_subject.trim(), "Initial commit");
        assert_eq!(branches.trim(), "");
    }

    #[tokio::test]
    async fn refuses_a_repository_without_commits() {
        let directory = TestDirectory::create("empty");
        let root = directory.path.join("repo");
        std::fs::create_dir_all(&root).expect("the directory is created");
        git(&root, &["init", "-q"]).await;

        let result = add_worktree(&root, "cr-feature", "main").await;
        assert!(result.is_err_and(|error| error.contains("no commits")));
    }

    #[tokio::test]
    async fn lists_the_worktrees_of_the_repositories_in_watched_directories() {
        let directory = TestDirectory::create("list");
        let repository = directory.init_repository("repo").await;
        std::fs::create_dir_all(directory.path.join("not-a-repo")).expect("created");
        add_worktree(&repository, "cr-feature", "main")
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
        let added = add_worktree(&repository, "cr-done", "main")
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
        let added = add_worktree(&repository, "cr-dirty", "main")
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

    #[tokio::test]
    async fn merge_reports_gits_error_when_the_main_checkout_has_changes_in_the_way() {
        let directory = TestDirectory::create("blocked");
        let repository = directory.init_repository("repo").await;
        let added = add_worktree(&repository, "cr-blocked", "main")
            .await
            .expect("the worktree is added");
        std::fs::write(added.path.join("README.md"), "from the branch")
            .expect("the file is written");
        git(&added.path, &["commit", "-q", "-am", "Work"]).await;
        std::fs::write(repository.join("README.md"), "uncommitted in main")
            .expect("the file is written");
        let worktree = worktree_named(&directory.path, "cr-blocked").await;

        let merge_blocked = merge_worktree(&worktree, "main").await;

        let error = merge_blocked.expect_err("the merge is refused");
        assert!(!error.contains("cannot fast-forward"), "{error}");
        assert!(error.contains("README.md"), "{error}");
        assert!(added.path.exists(), "the worktree is kept");
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
