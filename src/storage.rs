use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::domain::{DiaryContent, DiaryStorage};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Commit;
    use std::fs;
    use tempfile::TempDir;

    // Test helper functions
    fn create_test_commit(message: &str, time: i64) -> Commit {
        Commit::new(message.to_string(), time)
    }

    fn create_test_diary_content() -> DiaryContent {
        DiaryContent {
            commits: vec![
                create_test_commit("First commit", 1704067200), // 2024-01-01
                create_test_commit("Second commit", 1704153600), // 2024-01-02
            ],
            summary: "Test summary".to_string(),
            start_date: "2024-01-01".to_string(),
            end_date: "2024-01-07".to_string(),
        }
    }

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
        let content = create_test_diary_content();

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

        let content = create_test_diary_content();

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

        let content = create_test_diary_content();

        // Save diary should create directories
        let result = storage.save_diary(&content);
        assert!(result.is_ok());

        // Verify directory was created
        assert!(Path::new(&base_dir).exists());

        Ok(())
    }
}

