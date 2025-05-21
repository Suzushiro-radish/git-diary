use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestSystemMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Local};
use std::fmt::Display;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
use mockall::{automock, predicate::*};

// Core domain types
#[derive(Debug, Clone)]
pub struct Commit {
    message: String,
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

// Implementation of the dependency traits
pub struct GitRepositoryImpl {
    repo_path: String,
}

impl GitRepositoryImpl {
    pub fn new(repo_path: String) -> Self {
        Self { repo_path }
    }
}

#[async_trait::async_trait]
impl GitRepository for GitRepositoryImpl {
    fn get_commits_since(&self, timestamp: i64) -> Result<Vec<Commit>> {
        let repo = git2::Repository::open(&self.repo_path)?;
        let reflogs = repo.reflog("HEAD")?;
        let reflogs = reflogs.iter();

        let mut commits = Vec::new();

        for reflog in reflogs {
            let time = reflog.committer().when();
            if time.seconds() < timestamp {
                break;
            }
            commits.push(Commit::new(
                reflog.message().unwrap_or("No message").to_string(),
                reflog.committer().when().seconds(),
            ));
        }

        Ok(commits)
    }
}

pub struct AISummarizerImpl {
    client: Client<OpenAIConfig>,
    model: String,
    max_tokens: u32,
}

impl AISummarizerImpl {
    pub fn new(model: String, max_tokens: u32) -> Self {
        Self {
            client: Client::new(),
            model,
            max_tokens,
        }
    }
}

#[async_trait::async_trait]
impl AISummarizer for AISummarizerImpl {
    async fn summarize_commits(&self, commits: &[Commit]) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .max_tokens(self.max_tokens)
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

        let response = self.client.chat().create(request).await?;

        let mut summary = String::new();
        for choice in response.choices {
            let content = choice.message.content.unwrap_or("No content".to_string());
            summary.push_str(&content);
        }

        Ok(summary)
    }
}

pub struct DiaryStorageImpl {
    base_dir: String,
}

impl DiaryStorageImpl {
    pub fn new(base_dir: String) -> Self {
        Self { base_dir }
    }
}

impl DiaryStorage for DiaryStorageImpl {
    fn save_diary(&self, content: &DiaryContent) -> Result<String> {
        // Create the diaries directory if it doesn't exist
        let diary_dir = Path::new(&self.base_dir);
        if !diary_dir.exists() {
            fs::create_dir_all(diary_dir).context("Failed to create diary directory")?;
        }

        // Generate the file name
        let file_name = self.generate_file_name(content);

        // Create the file
        let mut file = File::create(&file_name).context("Failed to create diary file")?;

        // Format the content
        let markdown_content = self.format_markdown_content(content);

        // Write to file
        file.write_all(markdown_content.as_bytes())
            .context("Failed to write to diary file")?;

        println!("Diary saved to: {}", file_name);

        Ok(file_name)
    }

    /// Generates a file name based on the diary content's date range
    ///
    /// # Arguments
    ///
    /// * `content` - The DiaryContent containing the start and end dates
    ///
    /// # Returns
    ///
    /// A String containing the file path
    fn generate_file_name(&self, content: &DiaryContent) -> String {
        format!(
            "{}/git-diary-{}-to-{}.md",
            self.base_dir,
            content.start_date.replace("-", ""),
            content.end_date.replace("-", "")
        )
    }

    /// Formats the diary content as Markdown
    ///
    /// # Arguments
    ///
    /// * `content` - The DiaryContent to format
    ///
    /// # Returns
    ///
    /// A String containing the formatted Markdown content
    fn format_markdown_content(&self, content: &DiaryContent) -> String {
        // Format commit logs
        let mut commit_logs = String::new();
        for commit in content.commits.iter().rev() {
            commit_logs.push_str(&format!("- {}\n", commit));
        }

        // Create markdown content
        format!(
            "# Git Diary ({} – {})\n\n## Commit Logs\n\n{}\n\n## AI-generated Summary\n\n{}\n",
            content.start_date, content.end_date, commit_logs, content.summary
        )
    }
}

// DateTimeProvider implementation
pub struct DateTimeProviderImpl;

impl DateTimeProviderImpl {
    pub fn new() -> Self {
        Self
    }
}

impl DateTimeProvider for DateTimeProviderImpl {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }

    fn days_ago(&self, days: i64) -> DateTime<Local> {
        Local::now() - Duration::days(days)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get current directory as repo path
    let repo_path = std::env::current_dir()?.to_string_lossy().to_string();

    // Create dependencies
    let git_repo = Arc::new(GitRepositoryImpl::new(repo_path));
    let ai_summarizer = Arc::new(AISummarizerImpl::new("gpt-4o".to_string(), 1000));
    let storage = Arc::new(DiaryStorageImpl::new("diaries".to_string()));
    let datetime_provider = Arc::new(DateTimeProviderImpl::new());

    // Create diary generator
    let generator = DiaryGenerator::new(
        git_repo,
        ai_summarizer,
        storage,
        datetime_provider,
        7, // Last 7 days
    );

    // Generate diary
    let file_path = generator.generate_diary().await?;

    println!("Diary generated successfully and saved to: {}", file_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use chrono::TimeZone;
    use std::fs;
    use tempfile::TempDir;

    // Test helper functions
    fn create_test_commit(message: &str, time: i64) -> Commit {
        Commit::new(message.to_string(), time)
    }

    fn create_test_diary_content(commits: Vec<Commit>) -> DiaryContent {
        DiaryContent {
            commits,
            summary: "Test summary".to_string(),
            start_date: "2024-01-01".to_string(),
            end_date: "2024-01-07".to_string(),
        }
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

    #[tokio::test]
    async fn test_diary_generator_format_logs() {
        // Setup
        let mock_git_repo = Arc::new(MockGitRepository::new());
        let mock_ai_summarizer = Arc::new(MockAISummarizer::new());
        let mock_storage = Arc::new(MockDiaryStorage::new());
        let now = Local.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let datetime_provider = Arc::new(TestDateTimeProvider::new(now));

        let generator = DiaryGenerator::new(
            mock_git_repo,
            mock_ai_summarizer,
            mock_storage,
            datetime_provider,
            7,
        );

        // Test empty commits
        let empty_logs = generator.format_commit_logs(&[]);
        assert!(empty_logs.contains("Last 7 days commits"));

        // Test normal commits
        let commits = create_test_commits();
        let logs = generator.format_commit_logs(&commits);

        assert!(logs.contains("Last 7 days commits"));
        assert!(logs.contains("First commit"));
        assert!(logs.contains("Second commit"));
        assert!(logs.contains("Third commit with special chars"));
        assert!(logs.contains(&"A".repeat(20))); // Part of the long message
    }

    // DiaryStorage tests
    #[test]
    fn test_diary_storage_file_name() {
        let storage = DiaryStorageImpl::new("test_dir".to_string());

        // Test with normal dates
        let content = DiaryContent {
            commits: vec![],
            summary: "Test".to_string(),
            start_date: "2024-01-01".to_string(),
            end_date: "2024-01-07".to_string(),
        };

        let file_name = storage.generate_file_name(&content);
        assert_eq!(file_name, "test_dir/git-diary-20240101-to-20240107.md");

        // Test with different formats
        let content_2 = DiaryContent {
            commits: vec![],
            summary: "Test".to_string(),
            start_date: "2024/01/01".to_string(),
            end_date: "2024/01/07".to_string(),
        };

        let file_name_2 = storage.generate_file_name(&content_2);
        assert_eq!(
            file_name_2,
            "test_dir/git-diary-2024/01/01-to-2024/01/07.md"
        );
    }

    #[test]
    fn test_diary_storage_markdown_format() {
        let storage = DiaryStorageImpl::new("test".to_string());
        let content = create_test_diary_content(create_test_commits());

        let markdown = storage.format_markdown_content(&content);

        // Check markdown structure
        assert!(markdown.starts_with("# Git Diary"));
        assert!(markdown.contains("## Commit Logs"));
        assert!(markdown.contains("## AI-generated Summary"));

        // Check content formatting
        assert!(markdown.contains("- "));
        assert!(markdown.contains("First commit"));
        assert!(markdown.contains("Test summary"));

        // Check date range
        assert!(markdown.contains("2024-01-01 – 2024-01-07"));
    }

    #[test]
    fn test_diary_storage_save_diary() -> Result<()> {
        // Test actual file writing using tempfile
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir.path().to_string_lossy().to_string();
        let storage = DiaryStorageImpl::new(base_dir.clone());

        let content = create_test_diary_content(create_test_commits());

        // Save the diary
        let result = storage.save_diary(&content);
        assert!(result.is_ok());

        // Verify file exists and content is correct
        let file_path = result?;
        let file_content = fs::read_to_string(&file_path)?;

        // Check markdown structure
        assert!(file_content.contains("# Git Diary"));
        assert!(file_content.contains("## Commit Logs"));
        assert!(file_content.contains("## AI-generated Summary"));

        // Check content
        assert!(file_content.contains("First commit"));
        assert!(file_content.contains("Test summary"));

        Ok(())
    }

    #[test]
    fn test_diary_storage_create_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_dir = temp_dir
            .path()
            .join("nested/diary/dir")
            .to_string_lossy()
            .to_string();
        let storage = DiaryStorageImpl::new(base_dir.clone());

        let content = create_test_diary_content(create_test_commits());

        // Save diary should create directories
        let result = storage.save_diary(&content);
        assert!(result.is_ok());

        // Verify directory was created
        assert!(Path::new(&base_dir).exists());

        Ok(())
    }
}
