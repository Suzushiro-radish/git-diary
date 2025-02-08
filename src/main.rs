use anyhow::Result;
use chrono::{Duration, Local};

fn main() -> Result<()> {
    let path = std::env::current_dir()?;
    let repo = git2::Repository::open(path)?;
    let reflogs = repo.reflog("HEAD")?;
    let reflogs = reflogs.iter();

    let now = Local::now();
    let start = (now - Duration::days(7)).timestamp();

    let mut messages: Vec<String> = Vec::new();

    for reflog in reflogs {
        let time = reflog.committer().when();
        if time.seconds() < start {
            break;
        }
        messages.push(reflog.message().unwrap_or("No message").to_string());
    }

    println!("Last 7 days commits:");
    for message in messages.iter().rev() {
        println!("{}", message);
    }

    Ok(())
}
