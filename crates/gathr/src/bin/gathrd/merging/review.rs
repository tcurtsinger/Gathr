//! A completed review may carry over clean, canonical base-branch merges (G165).
use super::{ancestor, git};
use gathr::platform::env::Vars;
use std::path::Path;

/// The PR's own chain must contain only ordinary two-parent base merges. Compare each whole
/// tree with Git's clean automatic merge: a hand-edited or conflicted resolution needs review.
/// Imported base commits are not walked as author commits; their ancestry is checked instead.
pub(super) async fn base_updates_only(
    repo: &Path,
    reviewed: &str,
    head: &str,
    base: &str,
    vars: &Vars,
) -> Result<bool, String> {
    let commits = git(
        repo,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            &format!("{reviewed}..{head}"),
        ],
        vars,
    )
    .await?;
    let mut previous = reviewed.to_string();
    for commit in commits.lines() {
        let parents = git(repo, &["show", "--no-patch", "--format=%P", commit], vars).await?;
        let parents: Vec<_> = parents.split_whitespace().collect();
        if parents.len() != 2
            || parents[0] != previous
            || !ancestor(repo, parents[1], base, vars).await
        {
            return Ok(false);
        }
        let merged = git(
            repo,
            &["merge-tree", "--write-tree", parents[0], parents[1]],
            vars,
        )
        .await?;
        let tree = git(repo, &["rev-parse", &format!("{commit}^{{tree}}")], vars).await?;
        if merged.lines().next().map(str::trim) != Some(tree.trim()) {
            return Ok(false);
        }
        previous = commit.to_string();
    }
    Ok(previous == head)
}
