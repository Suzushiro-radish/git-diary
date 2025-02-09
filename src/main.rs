use std::fmt::Display;

use anyhow::Result;
use chrono::{DateTime, Duration, Local};

struct Commit {
    message: String,
    time: i64,
}

impl Commit {
    fn new(message: String, time: i64) -> Self {
        Self { message, time }
    }

    pub fn datetime(&self) -> Option<String> {
        let datetime = DateTime::from_timestamp(self.time, 0);
        datetime.map(|dt| dt.to_string())
    }
}

impl Display for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.datetime().unwrap_or("Invalid Date".to_string()), self.message)
    }
}

fn main() -> Result<()> {
    let path = std::env::current_dir()?;
    let repo = git2::Repository::open(path)?;
    let reflogs = repo.reflog("HEAD")?;
    let reflogs = reflogs.iter();

    let now = Local::now();
    let start = (now - Duration::days(7)).timestamp();

    let mut commits: Vec<Commit> = Vec::new();

    for reflog in reflogs {
        let time = reflog.committer().when();
        if time.seconds() < start {
            break;
        }
        commits.push(Commit::new(
            reflog.message().unwrap_or("No message").to_string(),
            reflog.committer().when().seconds(),
        ));
    }

    println!("Last 7 days commits:");
    for message in commits.iter().rev() {
        println!("{}", message);
    }

    Ok(())
}
