#![feature(iter_intersperse)]

use std::{
    env::current_dir,
    io::{stdout, Write},
};

use git2::{BranchType, Oid, Repository, Status, StatusOptions, StatusShow};

use crate::color::{bold_str, colored_str};

mod color;

#[derive(Debug)]
pub enum HeadInfo {
    /// This exists in empty repo, the branch name is there, but there is no commit
    EmptyBranch { name: String },
    /// Checking out local branch, this branch can optionally track a remote branch
    Branch {
        name: String,
        upstream: Option<String>,
    },
    /// Checking out a remote branch
    RemoteBranch { name: String },
    /// Checking out a tag
    Tag { name: String },
    /// None of the above, fallback to commit hash
    Commit { hash: String },
}

#[derive(Debug, Default)]
pub struct CommitStat {
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Debug, Default)]
pub struct StagingStat {
    pub modified: usize,
    pub staged: usize,
    pub conflict: usize,
}

#[derive(Debug)]
pub struct PromptData {
    pub head: HeadInfo,
    pub commit_stat: CommitStat,
    pub staging_stat: StagingStat,
    pub stash: usize,
    // TODO: Repo status: rebasing, cherry-picking, bisect, etc.
}

fn quit() -> ! {
    std::process::exit(1);
}

fn find_repo_using_current_dir() -> Repository {
    Repository::discover(&current_dir().unwrap()).unwrap_or_else(|_| quit())
}

fn find_tag(repo: &Repository, head: Oid) -> Option<String> {
    let mut tag = None;
    repo.tag_foreach(|oid, name| {
        if oid == head {
            tag = Some(
                repo.find_reference(std::str::from_utf8(name).unwrap())
                    .unwrap()
                    .shorthand()
                    .unwrap()
                    .to_string(),
            );
        }
        true
    })
    .unwrap();
    tag
}

fn prepare_head_info(repo: &Repository) -> HeadInfo {
    let repo_head = repo.head().unwrap();
    let head_name = repo_head.shorthand().unwrap().to_string();
    if repo_head.is_branch() {
        let branch = repo.find_branch(&head_name, BranchType::Local).unwrap();
        let upstream = branch
            .upstream()
            .ok()
            .map(|u| u.name().unwrap().unwrap().to_string());
        HeadInfo::Branch {
            upstream,
            name: head_name,
        }
    } else if let Some(tag) = find_tag(repo, repo_head.peel_to_commit().unwrap().id()) {
        HeadInfo::Tag { name: tag }
    } else if repo_head.is_remote() {
        HeadInfo::RemoteBranch { name: head_name }
    } else {
        HeadInfo::Commit {
            hash: repo_head.target().unwrap().to_string(),
        }
    }
}

fn prepare_commit_stat(repo: &Repository, head: &HeadInfo) -> CommitStat {
    let mut ahead = 0;
    let mut behind = 0;
    if let HeadInfo::Branch { name, upstream } = head {
        // TODO: Handle empty repo, the branch exists but no commit available
        let local_commit = repo
            .find_branch(&name, BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();
        match upstream {
            Some(upstream_name) => {
                let upstream_commit = repo
                    .find_branch(&upstream_name, BranchType::Remote)
                    .unwrap()
                    .get()
                    .peel_to_commit()
                    .unwrap();
                (ahead, behind) = repo
                    .graph_ahead_behind(local_commit.id(), upstream_commit.id())
                    .unwrap();
            }
            None => {
                // No branch to compare, just return total number of commits
                let mut walk = repo.revwalk().unwrap();
                walk.push_head().unwrap();
                ahead = walk.count();
            }
        }
    }
    CommitStat { ahead, behind }
}

fn prepare_staging_stat(repo: &Repository) -> StagingStat {
    let mut modified = 0;
    let mut staged = 0;
    let mut conflict = 0;
    repo.statuses(Some(
        StatusOptions::new()
            .show(StatusShow::IndexAndWorkdir)
            .include_ignored(false)
            .include_untracked(true)
            .include_unmodified(false)
            .recurse_untracked_dirs(true),
    ))
    .unwrap()
    .iter()
    .for_each(|status_entry| {
        // Note: A file can be added to staging area and is modified again, don't assume the flag
        // is exclusive
        let status = status_entry.status();
        if status.contains(Status::CONFLICTED) {
            conflict += 1;
            // Don't count conflict file as modified
            // TODO: But what if the conflict is resolved and added to staging area?
            return;
        }
        // dbg!(status_entry.path());
        // dbg!(status);
        if status.contains(Status::INDEX_MODIFIED)
            || status.contains(Status::INDEX_NEW)
            || status.contains(Status::INDEX_DELETED)
        {
            staged += 1;
        }
        if status.contains(Status::WT_MODIFIED)
            || status.contains(Status::WT_NEW)
            || status.contains(Status::WT_DELETED)
        {
            modified += 1;
        }
    });
    StagingStat {
        modified,
        staged,
        conflict,
    }
}

fn get_current_branch_in_empty_repo(repo: &Repository) -> String {
    let mut path = repo.path().to_path_buf();
    path.push("HEAD");
    std::fs::read_to_string(path)
        .unwrap()
        .trim_start_matches("ref: refs/heads/")
        .trim_end()
        .to_string()
}

fn prepare_prompt_data(repo: &mut Repository) -> PromptData {
    if repo.is_empty().unwrap() {
        return PromptData {
            head: HeadInfo::EmptyBranch {
                name: format!("{}", get_current_branch_in_empty_repo(repo)),
            },
            commit_stat: CommitStat::default(),
            staging_stat: StagingStat::default(),
            stash: 0,
        };
    }
    let head = prepare_head_info(repo);
    let commit_stat = prepare_commit_stat(repo, &head);
    let staging_stat = prepare_staging_stat(repo);
    let mut stash = 0;
    repo.stash_foreach(|_index, _message, _oid| {
        stash += 1;
        true
    })
    .unwrap();
    PromptData {
        head,
        commit_stat,
        staging_stat,
        stash,
    }
}

fn print_prompt(data: &PromptData) {
    let stdout = stdout();
    let mut stdout = stdout.lock();
    // Stash info
    if data.stash > 0 {
        write!(&mut stdout, "🚧{} ", data.stash).unwrap();
    }
    // Staging info (will be mixed in the middle of head info, so we can't print it now)
    let staging_info = if data.staging_stat.modified > 0
        || data.staging_stat.staged > 0
        || data.staging_stat.conflict > 0
    {
        let stat_str = [
            if data.staging_stat.staged > 0 {
                Some(colored_str(
                    &format!("🗸{}", data.staging_stat.staged),
                    "green",
                ))
            } else {
                None
            },
            if data.staging_stat.modified > 0 {
                Some(colored_str(
                    &format!("•{}", data.staging_stat.modified),
                    "yellow",
                ))
            } else {
                None
            },
            if data.staging_stat.conflict > 0 {
                Some(colored_str(
                    &format!("✘{}", data.staging_stat.conflict),
                    "red",
                ))
            } else {
                None
            },
        ]
        .iter()
        .flatten()
        .map(|s| s.as_str())
        .intersperse(", ")
        .collect::<String>();
        format!("({})", &stat_str)
    } else {
        String::new()
    };
    // Head info (include commit count)
    match &data.head {
        HeadInfo::Branch { name, upstream } => {
            write!(
                &mut stdout,
                "{}{} -> {}",
                bold_str(&colored_str(name, "green")),
                staging_info,
                bold_str(&colored_str(
                    upstream.as_ref().map(|s| s.as_str()).unwrap_or("∅"),
                    "red"
                ))
            )
            .unwrap();
            if data.commit_stat.ahead > 0 || data.commit_stat.behind > 0 {
                write!(&mut stdout, " (").unwrap();
                if data.commit_stat.ahead > 0 {
                    write!(&mut stdout, "{}↑", data.commit_stat.ahead).unwrap();
                }
                if data.commit_stat.behind > 0 {
                    if data.commit_stat.ahead > 0 {
                        write!(&mut stdout, ", ").unwrap();
                    }
                    write!(&mut stdout, "{}↓", data.commit_stat.behind).unwrap();
                }
                write!(&mut stdout, ")").unwrap();
            }
        }
        HeadInfo::RemoteBranch { name } => {
            write!(
                &mut stdout,
                "{}{}",
                bold_str(&colored_str(name, "red")),
                staging_info
            )
            .unwrap();
        }
        HeadInfo::Tag { name } => {
            write!(
                &mut stdout,
                "🔖{}{}",
                bold_str(&colored_str(name, "blue")),
                staging_info
            )
            .unwrap();
        }
        HeadInfo::Commit { hash } => {
            write!(
                &mut stdout,
                "Commit {}{}",
                bold_str(&colored_str(&hash[0..=12], "blue")),
                staging_info
            )
            .unwrap();
        }
        HeadInfo::EmptyBranch { name } => {
            write!(
                &mut stdout,
                "{} {}",
                bold_str(&colored_str(&name, "green")),
                colored_str("(empty repo)", "red")
            )
            .unwrap();
        }
    }
}

fn main() {
    let mut repo = find_repo_using_current_dir();
    let prompt_data = prepare_prompt_data(&mut repo);
    print_prompt(&prompt_data);
}
