use std::env::current_dir;

use git2::{Repository, Status};

fn main() {
    let repo = Repository::discover(
        &current_dir()
            .expect("Current directory does not exist or insufficient permissions to access it"),
    )
    .expect("Failed opening repository");
    println!("{:?}", repo.state());
    println!(
        "{:?}",
        repo.statuses(None)
            .unwrap()
            .iter()
            .map(|x| x.status())
            .collect::<Vec<Status>>()
    );
    // println!(
    //     "{:?}",
    //     repo.branches(None)
    //         .unwrap()
    //         .into_iter()
    //         .map(|(branch, branch_type)| branch)
    //         .collect::<Vec>()
    // );
}
