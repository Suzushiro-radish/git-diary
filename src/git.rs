use anyhow::Result;
use git2;

use crate::domain::{Commit, GitRepository};

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    // Test helper to create a git repository with test commits
    fn setup_test_repo() -> Result<(TempDir, String)> {
        // Create a temporary directory for the test repository
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path().to_string_lossy().to_string();
        
        // Initialize git repository
        let repo = git2::Repository::init(&repo_path)?;
        let signature = git2::Signature::now("Test User", "test@example.com")?;
        
        // Create a test file
        let test_file_path = Path::new(&repo_path).join("test.txt");
        let mut file = fs::File::create(&test_file_path)?;
        writeln!(file, "Test content")?;
        
        // Add and commit the file
        let mut index = repo.index()?;
        index.add_path(Path::new("test.txt"))?;
        index.write()?;
        
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Test commit",
            &tree,
            &[],
        )?;
        
        Ok((temp_dir, repo_path))
    }

    #[test]
    fn test_get_commits_since() -> Result<()> {
        // Setup test repository
        let (_temp_dir, repo_path) = setup_test_repo()?;
        
        // Create GitRepositoryImpl instance
        let git_repo = GitRepositoryImpl::new(repo_path);
        
        // Get commits
        let timestamp = Local::now().timestamp() - 3600; // 1 hour ago
        let commits = git_repo.get_commits_since(timestamp)?;
        
        // Verify we got the test commit
        assert!(!commits.is_empty());
        assert!(commits[0].message.contains("Test commit"));
        
        Ok(())
    }
    
    #[test]
    fn test_get_commits_since_future_timestamp() -> Result<()> {
        // Setup test repository
        let (_temp_dir, repo_path) = setup_test_repo()?;
        
        // Create GitRepositoryImpl instance
        let git_repo = GitRepositoryImpl::new(repo_path);
        
        // Get commits with a future timestamp
        let timestamp = Local::now().timestamp() + 3600; // 1 hour in the future
        let commits = git_repo.get_commits_since(timestamp)?;
        
        // Verify we got no commits
        assert!(commits.is_empty());
        
        Ok(())
    }
    
    #[test]
    fn test_invalid_repository_path() {
        // Create GitRepositoryImpl with invalid path
        let git_repo = GitRepositoryImpl::new("/path/that/does/not/exist".to_string());
        
        // Attempt to get commits
        let result = git_repo.get_commits_since(0);
        
        // Verify operation failed
        assert!(result.is_err());
    }
}

