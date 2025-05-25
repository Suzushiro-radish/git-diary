use anyhow::Result;
use async_openai::Client;
use chrono::{DateTime, Duration, Local};
use clap::Parser;
use std::sync::Arc;

// Declare modules
mod ai;
mod domain;
mod git;
mod storage;

// Import necessary types from modules
use ai::AISummarizerImpl;
use domain::{DateTimeProvider, DiaryGenerator};
use git::GitRepositoryImpl;
use storage::DiaryStorageImpl;

/// Generate a diary from Git commits using AI summarization
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of days to look back for commits
    #[arg(short, long, default_value_t = 1)]
    days: i64,
}

// Simple DateTime provider implementation
struct LocalDateTimeProvider;

impl LocalDateTimeProvider {
    fn new() -> Self {
        Self
    }
}

impl DateTimeProvider for LocalDateTimeProvider {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }

    fn days_ago(&self, days: i64) -> DateTime<Local> {
        Local::now() - Duration::days(days)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Get current directory as repo path
    let repo_path = std::env::current_dir()?.to_string_lossy().to_string();

    // Create dependencies
    let git_repo = Arc::new(GitRepositoryImpl::new(repo_path));
    let ai_summarizer = Arc::new(AISummarizerImpl::new(
        Client::new(),
        "gpt-4".to_string(),
        1000,
    ));
    let storage = Arc::new(DiaryStorageImpl::new("diaries".to_string()));
    let datetime_provider = Arc::new(LocalDateTimeProvider::new());

    // Create diary generator
    let generator = DiaryGenerator::new(
        git_repo,
        ai_summarizer,
        storage,
        datetime_provider,
        args.days, // Use the parsed number of days
    );

    // Generate diary
    match generator.generate_diary().await {
        Ok(file_path) => {
            println!("✨ Successfully generated diary!");
            println!("📝 File saved to: {}", file_path);
        }
        Err(e) => {
            eprintln!("❌ Error generating diary: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
