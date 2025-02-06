use anyhow::Result;

fn main() -> Result<()> {
    let path = std::env::current_dir()?;
    let repo = git2::Repository::open(path)?;
    let head = repo.head()?;
    let head = head.peel_to_commit()?;
    let head_id = head.message().unwrap_or("default message");
    println!("HEAD: {}", head_id);

    Ok(())
}
