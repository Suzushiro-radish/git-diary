use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Local};
use std::fmt::Display;
use std::sync::Arc;

#[cfg(test)]
use mockall::{automock, predicate::*};

// Core domain types
#[derive(Debug, Clone)]
pub struct Commit {
    pub message: String,
    time: i64,
}

impl Commit {
    pub fn new(message: String, time: i64) -> Self {
        Self { message, time }
    }

    pub fn datetime(&self) -> Option<String> {
        let datetime = DateTime::from_timestamp(self.time, 0);
        datetime.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
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

#[derive(Debug)]
pub struct DiaryContent {
    pub commits: Vec<Commit>,
    pub summary: String,
    pub start_date: String,
    pub end_date: String,
}

// Trait definitions for external dependencies
#[cfg_attr(test, automock)]
#[async_trait]
pub trait GitRepository: Send + Sync {
    fn get_commits_since(&self, timestamp: i64) -> Result<Vec<Commit>>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait AISummarizer: Send + Sync {
    async fn summarize_commits(&self, commits: &[Commit]) -> Result<String>;
}

#[cfg_attr(test, automock)]
pub trait DiaryStorage: Send + Sync {
    fn save_diary(&self, content: &DiaryContent) -> Result<String>;
    fn generate_file_name(&self, content: &DiaryContent) -> String;
    fn format_markdown_content(&self, content: &DiaryContent) -> String;
}

#[cfg_attr(test, automock)]
pub trait DateTimeProvider: Send + Sync {
    fn now(&self) -> DateTime<Local>;
    fn days_ago(&self, days: i64) -> DateTime<Local>;
}

// DiaryGenerator implementation
pub struct DiaryGenerator<G, A, S, D>
where
    G: GitRepository,
    A: AISummarizer,
    S: DiaryStorage,
    D: DateTimeProvider,
{
    git_repo: Arc<G>,
    ai_summarizer: Arc<A>,
    storage: Arc<S>,
    datetime_provider: Arc<D>,
    days_to_include: i64,
}

impl<G, A, S, D> DiaryGenerator<G, A, S, D>
where
    G: GitRepository,
    A: AISummarizer,
    S: DiaryStorage,
    D: DateTimeProvider,
{
    pub fn new(
        git_repo: Arc<G>,
        ai_summarizer: Arc<A>,
        storage: Arc<S>,
        datetime_provider: Arc<D>,
        days_to_include: i64,
    ) -> Self {
        Self {
            git_repo,
            ai_summarizer,
            storage,
            datetime_provider,
            days_to_include,
        }
    }

    pub fn format_commit_logs(&self, commits: &[Commit]) -> String {
        let mut logs = String::new();
        logs.push_str(&format!("Last {} days commits:\n", self.days_to_include));

        for commit in commits.iter().rev() {
            logs.push_str(&format!("{}\n", commit));
        }

        logs
    }

    pub async fn generate_diary(&self) -> Result<String> {
        let now = self.datetime_provider.now();
        let days_ago = self.datetime_provider.days_ago(self.days_to_include);

        let start_date = days_ago.format("%Y-%m-%d").to_string();
        let end_date = now.format("%Y-%m-%d").to_string();

        // Get commits from git repository
        let commits = self.git_repo.get_commits_since(days_ago.timestamp())?;

        // Format commit logs
        let commit_logs = self.format_commit_logs(&commits);

        // Print the commit logs
        println!("{}", commit_logs);

        // Get summary from AI
        let summary = self.ai_summarizer.summarize_commits(&commits).await?;

        // Print the summary
        println!("Summary:");
        println!("------------------------------------");
        println!("{}", summary);

        // Create diary content
        let content = DiaryContent {
            commits,
            summary,
            start_date,
            end_date,
        };

        // Save diary to storage
        let file_path = self.storage.save_diary(&content)?;

        Ok(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use chrono::{Duration, TimeZone};

    // Test helper functions
    fn create_test_commit(message: &str, time: i64) -> Commit {
        Commit::new(message.to_string(), time)
    }

    fn create_test_commits() -> Vec<Commit> {
        vec![
            create_test_commit("First commit", 1704067200), // 2024-01-01
            create_test_commit("Second commit", 1704153600), // 2024-01-02
            create_test_commit("Third commit with special chars: !@#$%^&*()", 1704240000), // 2024-01-03
            create_test_commit(&"A".repeat(500), 1704326400), // Very long commit message (2024-01-04)
        ]
    }

    struct TestDateTimeProvider {
        now: DateTime<Local>,
    }

    impl TestDateTimeProvider {
        fn new(now: DateTime<Local>) -> Self {
            Self { now }
        }
    }

    impl DateTimeProvider for TestDateTimeProvider {
        fn now(&self) -> DateTime<Local> {
            self.now
        }

        fn days_ago(&self, days: i64) -> DateTime<Local> {
            self.now - Duration::days(days)
        }
    }

    // Basic commit and display tests
    #[test]
    fn test_commit_datetime_formatting() {
        let commit = create_test_commit("Test commit", 1714989869);

        assert!(commit.datetime().is_some());
        assert!(commit.to_string().contains("Test commit"));
    }

    // DiaryGenerator tests
    #[tokio::test]
    async fn test_diary_generator_success() {
        // Setup mocks
        let mut mock_git_repo = MockGitRepository::new();
        let mut mock_ai_summarizer = MockAISummarizer::new();
        let mut mock_storage = MockDiaryStorage::new();
        let now = Local.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let datetime_provider = Arc::new(TestDateTimeProvider::new(now));

        let test_commits = create_test_commits();
        let expected_file_path = "diaries/test-diary.md".to_string();

        let expected_file_path_2 = expected_file_path.clone();

        // Set mock expectations
        mock_git_repo
            .expect_get_commits_since()
            .returning(move |_| Ok(test_commits.clone()));

        mock_ai_summarizer
            .expect_summarize_commits()
            .returning(|_| Ok("This is a test summary".to_string()));

        mock_storage
            .expect_save_diary()
            .returning(move |_| Ok(expected_file_path.clone()));

        let generator = DiaryGenerator::new(
            Arc::new(mock_git_repo),
            Arc::new(mock_ai_summarizer),
            Arc::new(mock_storage),
            datetime_provider,
            7,
        );

        // Execute
        let result = generator.generate_diary().await;

        // Verify
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_file_path_2);
    }

    #[tokio::test]
    async fn test_diary_generator_git_error() {
        // Setup mocks
        let mut mock_git_repo = MockGitRepository::new();
        let mock_ai_summarizer = MockAISummarizer::new();
        let mock_storage = MockDiaryStorage::new();
        let now = Local.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let datetime_provider = Arc::new(TestDateTimeProvider::new(now));

        // Set mock expectations - simulate Git error
        mock_git_repo
            .expect_get_commits_since()
            .returning(|_| Err(anyhow!("Git repository error").into()));

        let generator = DiaryGenerator::new(
            Arc::new(mock_git_repo),
            Arc::new(mock_ai_summarizer),
            Arc::new(mock_storage),
            datetime_provider,
            7,
        );

        // Execute
        let result = generator.generate_diary().await;

        // Verify
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Git repository error"));
    }

    #[tokio::test]
    async fn test_diary_generator_ai_error() {
        // Setup mocks
        let mut mock_git_repo = MockGitRepository::new();
        let mut mock_ai_summarizer = MockAISummarizer::new();
        let mock_storage = MockDiaryStorage::new();
        let now = Local.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let datetime_provider = Arc::new(TestDateTimeProvider::new(now));

        let test_commits = create_test_commits();

        // Set mock expectations - Git success but AI error
        mock_git_repo
            .expect_get_commits_since()
            .returning(move |_| Ok(test_commits.clone()));

        mock_ai_summarizer
            .expect_summarize_commits()
            .returning(|_| Err(anyhow!("AI service error").into()));

        let generator = DiaryGenerator::new(
            Arc::new(mock_git_repo),
            Arc::new(mock_ai_summarizer),
            Arc::new(mock_storage),
            datetime_provider,
            7,
        );

        // Execute
        let result = generator.generate_diary().await;

        // Verify
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("AI service error"));
    }

    #[tokio::test]
    async fn test_diary_generator_storage_error() {
        // Setup mocks
        let mut mock_git_repo = MockGitRepository::new();
        let mut mock_ai_summarizer = MockAISummarizer::new();
        let mut mock_storage = MockDiaryStorage::new();
        let now = Local.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let datetime_provider = Arc::new(TestDateTimeProvider::new(now));

        let test_commits = create_test_commits();

        // Set mock expectations - Git and AI success, storage error
        mock_git_repo
            .expect_get_commits_since()
            .returning(move |_| Ok(test_commits.clone()));

        mock_ai_summarizer
            .expect_summarize_commits()
            .returning(|_| Ok("This is a test summary".to_string()));

        mock_storage
            .expect_save_diary()
            .returning(|_| Err(anyhow!("Storage error").into()));

        let generator = DiaryGenerator::new(
            Arc::new(mock_git_repo),
            Arc::new(mock_ai_summarizer),
            Arc::new(mock_storage),
            datetime_provider,
            7,
        );

        // Execute
        let result = generator.generate_diary().await;

        // Verify
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Storage error"));
    }

    #[tokio::test]
    async fn test_diary_generator_empty_commits() {
        // Setup mocks
        let mut mock_git_repo = MockGitRepository::new();
        let mut mock_ai_summarizer = MockAISummarizer::new();
        let mut mock_storage = MockDiaryStorage::new();
        let now = Local.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let datetime_provider = Arc::new(TestDateTimeProvider::new(now));

        // Set mock expectations - return empty commit list
        mock_git_repo
            .expect_get_commits_since()
            .returning(|_| Ok(Vec::new()));

        // AI should still be called even with empty commits
        mock_ai_summarizer
            .expect_summarize_commits()
            .returning(|commits| {
                assert!(commits.is_empty());
                Ok("No activity in the last 7 days".to_string())
            });

        mock_storage.expect_save_diary().returning(|content| {
            assert!(content.commits.is_empty());
            assert_eq!(content.summary, "No activity in the last 7 days");
            Ok("diaries/empty-diary.md".to_string())
        });

        let generator = DiaryGenerator::new(
            Arc::new(mock_git_repo),
            Arc::new(mock_ai_summarizer),
            Arc::new(mock_storage),
            datetime_provider,
            7,
        );

        // Execute
        let result = generator.generate_diary().await;

        // Verify
        assert!(result.is_ok());
    }
}
