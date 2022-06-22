use std::env::{args, current_dir};

use git2::{BranchType, Repository, RepositoryState, Status};
use log::error;

#[derive(Debug, Clone)]
pub struct RepoBranch {
    pub name: String,
    pub branch_type: Option<BranchType>,
    pub remote: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub state: RepositoryState,
    pub branches: Vec<RepoBranch>,
}

impl RepoStatus {
    pub fn new() -> Self {
        Self {
            state: RepositoryState::Clean,
            branches: Vec::new(),
        }
    }
}

fn quit_with_error() -> ! {
    std::process::exit(1);
}

fn find_repo_using_current_dir() -> Repository {
    let pwd = current_dir().unwrap_or_else(|err| {
        error!(
            "Current directory does not exist or insufficient permissions to access it\n{:?}",
            err
        );
        quit_with_error();
    });
    Repository::discover(&pwd).unwrap_or_else(|err| {
        error!("No repo found\n{:?}", err);
        quit_with_error();
    })
}

fn main() {
    if let Some(arg) = args().skip(1).next() {
        if arg.eq("--verbose") {
            env_logger::builder()
                .filter_level(log::LevelFilter::Info)
                .init();
        }
    }

    let repo = find_repo_using_current_dir();
    let mut repo_status = RepoStatus::new();
    repo_status.state = repo.state();
    let branches = repo.branches(None).unwrap_or_else(|err| {
        error!("Failed reading branches in repo\n{:?}", err);
        quit_with_error();
    });
    branches.into_iter().for_each(|item| match item {
        Ok((b, t)) => {
            log::info!("{:?} {:?} {:?}", b.name(), b.upstream().map(|_| "TBD"), t);
        }
        Err(err) => error!("Unexpected error while reading branches info: {:?}", err),
    });
    log::info!(
        "{:?}",
        repo.statuses(None)
            .unwrap()
            .iter()
            .map(|x| x.status())
            .collect::<Vec<Status>>()
    );
}
