use anyhow::Result;
use async_openai::{
    types::{ChatCompletionRequestSystemMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use chrono::{DateTime, Duration, Local};
use std::fmt::Display;

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
        write!(
            f,
            "{}: {}",
            self.datetime().unwrap_or("Invalid Date".to_string()),
            self.message
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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

    let openai_client = Client::new();

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4o")
        .max_tokens(1000_u32)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You are a software developer who has been working on a project for the last 7 days. You have been making commits to a git repository. You want to summarize the commits you have made in the last 7 days.")
                .build()?
                .into(),
            ChatCompletionRequestSystemMessageArgs::default()
                .content(commits.iter().map(|commit| commit.to_string()).collect::<Vec<String>>().join("\n"))
                .build()?
                .into(),
        ])
        .build()?;

    let response = openai_client.chat().create(request).await?;

    println!("Summary:");

    for choice in response.choices {
        println!("------------------------------------");
        println!(
            "{}",
            choice.message.content.unwrap_or("No content".to_string())
        );
    }

    Ok(())
}
