use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestSystemMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use async_trait::async_trait;

use crate::domain::{AISummarizer, Commit};

pub struct AISummarizerImpl {
    client: Client<OpenAIConfig>,
    model: String,
    max_tokens: u32,
}

impl AISummarizerImpl {
    pub fn new(client: Client<OpenAIConfig>, model: String, max_tokens: u32) -> Self {
        Self {
            client,
            model,
            max_tokens,
        }
    }
}

#[async_trait]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_summarize_commits_empty() {
        let summarizer = AISummarizerImpl::new(Client::new(), "gpt-4o".to_string(), 1000);

        // Skip the test if no API key is available
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!("Skipping test_summarize_commits_empty: No API key available");
            return;
        }

        let commits = Vec::new();
        let result = summarizer.summarize_commits(&commits).await;

        assert!(result.is_ok());
        // We just verify we get some response back
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_summarize_commits() {
        let summarizer = AISummarizerImpl::new(Client::new(), "gpt-4o".to_string(), 1000);

        // Skip the test if no API key is available
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!("Skipping test_summarize_commits: No API key available");
            return;
        }

        let commits = vec![
            Commit::new("Initial commit".to_string(), 1704067200),
            Commit::new("Add README.md".to_string(), 1704153600),
            Commit::new("Implement core functionality".to_string(), 1704240000),
            Commit::new("Fix bug in error handling".to_string(), 1704326400),
        ];

        let result = summarizer.summarize_commits(&commits).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(!summary.is_empty());
        // We expect the summary to include something about the commits
        assert!(
            summary.contains("commit")
                || summary.contains("functionality")
                || summary.contains("README")
                || summary.contains("bug")
        );
    }

    // This test verifies behavior when the API returns an error
    // Note: This can't actually call the API since we don't want tests to fail
    // due to API connectivity issues
    #[tokio::test]
    async fn test_summarize_commits_error() {
        // Test API error by using an invalid model name
        let summarizer = AISummarizerImpl::new(Client::new(), "invalid-model".to_string(), 1000);

        // Skip the test if no API key is available
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!("Skipping test_summarize_commits_error: No API key available");
            return;
        }

        let commits = vec![Commit::new("Test commit".to_string(), 1704067200)];

        let result = summarizer.summarize_commits(&commits).await;

        // The request should fail due to invalid model
        assert!(result.is_err());
    }
}
