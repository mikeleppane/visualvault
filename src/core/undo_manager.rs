use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
const MAX_UNDO_HISTORY: usize = 10000;
const UNDO_HISTORY_FILE: &str = "undo_history.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Move {
        source: PathBuf,
        destination: PathBuf,
    },
    Copy {
        source: PathBuf,
        destination: PathBuf,
    },
    Delete {
        path: PathBuf,
        backup_path: Option<PathBuf>,
    },
    BatchMove {
        operations: Vec<MoveOperation>,
    },
    BatchDelete {
        operations: Vec<DeleteOperation>,
    },
    OrganizeFiles {
        operations: Vec<FileOperation>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOperation {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOperation {
    pub path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Move(MoveOperation),
    Copy { source: PathBuf, destination: PathBuf },
    Delete(DeleteOperation),
}

#[derive(Debug, thiserror::Error)]
pub enum VisualVaultError {
    // ...existing variants...
    #[error("Undo operation failed: {message}")]
    UndoError { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoableOperation {
    pub id: String,
    pub operation: OperationType,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub undone: bool,
    pub metadata: Option<serde_json::Value>,
}

impl UndoableOperation {
    pub fn new(operation: OperationType, description: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            operation,
            timestamp: Utc::now(),
            description,
            undone: false,
            metadata: None,
        }
    }
}

#[derive(Debug)]
pub struct UndoManager {
    history: Arc<RwLock<VecDeque<UndoableOperation>>>,
    redo_stack: Arc<RwLock<Vec<UndoableOperation>>>,
    config_dir: PathBuf,
}

impl UndoManager {
    /// Create a new `UndoManager` instance
    ///
    /// # Errors
    ///
    /// This function currently does not return any errors, but returns a `Result`
    /// to maintain consistency with the async constructor and allow for future
    /// error conditions during initialization.
    #[must_use]
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            history: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_UNDO_HISTORY))),
            redo_stack: Arc::new(RwLock::new(Vec::new())),
            config_dir,
        }
    }

    /// Create a new `UndoManager` and load history from disk
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The undo manager cannot be initialized
    /// - The history file cannot be read from disk
    /// - The history file contains invalid JSON data
    /// - File system operations fail during history loading
    pub async fn new_with_history(config_dir: PathBuf) -> Result<Self> {
        let mut manager = Self::new(config_dir);
        manager.load_history().await?;
        Ok(manager)
    }

    /// Record a new operation in the undo history
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The history cannot be saved to disk due to file system errors
    /// - JSON serialization of the history fails
    /// - Directory creation fails when saving the history file
    pub async fn record_operation(&self, operation: UndoableOperation) -> Result<()> {
        let mut history = self.history.write().await;

        // Clear redo stack when new operation is recorded
        self.redo_stack.write().await.clear();

        // Add new operation
        history.push_back(operation);

        // Maintain max history size
        while history.len() > MAX_UNDO_HISTORY {
            history.pop_front();
        }

        // Save to disk
        drop(history);
        self.save_history().await?;

        Ok(())
    }

    /// Record a file move operation
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The history cannot be saved to disk due to file system errors
    /// - JSON serialization of the history fails
    /// - Directory creation fails when saving the history file
    #[allow(dead_code)]
    pub async fn record_move(&self, source: &Path, destination: &Path) -> Result<()> {
        let operation = UndoableOperation::new(
            OperationType::Move {
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
            },
            format!("Moved {} to {}", source.display(), destination.display()),
        );

        self.record_operation(operation).await
    }

    /// Record a batch organization operation
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The history cannot be saved to disk due to file system errors
    /// - JSON serialization of the history fails
    /// - Directory creation fails when saving the history file
    pub async fn record_organize(&self, operations: Vec<FileOperation>) -> Result<()> {
        let count = operations.len();
        let operation = UndoableOperation::new(
            OperationType::OrganizeFiles { operations },
            format!("Organized {count} files"),
        );

        self.record_operation(operation).await
    }

    /// Undo the last operation
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The file system operations fail during the undo process (e.g., file cannot be moved or restored)
    /// - The history cannot be saved to disk after marking the operation as undone
    /// - Required backup files are missing for delete operations
    /// - Directory creation or file operations fail during the undo process
    pub async fn undo(&self) -> Result<Option<String>> {
        let history = self.history.write().await;

        // Find the last non-undone operation
        let position = history.iter().rposition(|op| !op.undone);

        if let Some(pos) = position {
            let mut operation = history[pos].clone();

            // Perform the undo
            drop(history);
            let result = Self::undo_operation(&operation)?;

            // Mark as undone
            let mut history = self.history.write().await;
            history[pos].undone = true;
            operation.undone = true;

            // Add to redo stack
            self.redo_stack.write().await.push(operation);

            // Save history
            drop(history);
            self.save_history().await?;

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Redo the last undone operation
    ///
    /// # Errors
    ///
    /// Returns an error if the file system operations fail during the redo process,
    /// such as when files cannot be moved, copied, or deleted.
    pub async fn redo(&self) -> Result<Option<String>> {
        let operation = self.redo_stack.write().await.pop();

        if let Some(mut op) = operation {
            let result = Self::redo_operation(&op)?;

            // Mark as not undone and add back to history
            op.undone = false;
            let mut history = self.history.write().await;

            // Find and update the operation in history
            if let Some(pos) = history.iter().position(|h| h.id == op.id) {
                history[pos].undone = false;
            }

            drop(history);
            self.save_history().await?;

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::too_many_lines)]
    /// Perform the actual undo operation
    fn undo_operation(operation: &UndoableOperation) -> Result<String> {
        match &operation.operation {
            OperationType::Move { source, destination } => {
                // Undo move by moving back
                if destination.exists() {
                    fs::rename(destination, source)?;
                    Ok(format!("Restored {} to original location", source.display()))
                } else {
                    Err(VisualVaultError::UndoError {
                        message: format!("Cannot undo: {} no longer exists", destination.display()),
                    }
                    .into())
                }
            }

            OperationType::Copy { source: _, destination } => {
                // Undo copy by deleting the copy
                if destination.exists() {
                    fs::remove_file(destination)?;
                    Ok(format!("Removed copy at {}", destination.display()))
                } else {
                    Ok("Copy already removed".to_string())
                }
            }

            OperationType::Delete { path, backup_path } => {
                // Undo delete by restoring from backup
                if let Some(backup) = backup_path {
                    if backup.exists() {
                        fs::rename(backup, path)?;
                        Ok(format!("Restored {} from backup", path.display()))
                    } else {
                        Err(VisualVaultError::UndoError {
                            message: "Backup file not found".to_string(),
                        }
                        .into())
                    }
                } else {
                    Err(VisualVaultError::UndoError {
                        message: "No backup available for deleted file".to_string(),
                    }
                    .into())
                }
            }

            OperationType::BatchMove { operations } => {
                let mut success_count = 0;
                let mut errors = Vec::new();

                for op in operations.iter().rev() {
                    if op.destination.exists() {
                        match fs::rename(&op.destination, &op.source) {
                            Ok(()) => success_count += 1,
                            Err(e) => errors.push(format!("{}: {}", op.source.display(), e)),
                        }
                    }
                }

                if errors.is_empty() {
                    Ok(format!("Restored {success_count} files to original locations"))
                } else {
                    Ok(format!(
                        "Restored {} files ({} errors: {})",
                        success_count,
                        errors.len(),
                        errors.join(", ")
                    ))
                }
            }

            OperationType::BatchDelete { operations } => {
                let mut restored_count = 0;

                for op in operations {
                    if let Some(backup) = &op.backup_path {
                        if backup.exists() {
                            fs::rename(backup, &op.path)?;
                            restored_count += 1;
                        }
                    }
                }

                Ok(format!("Restored {restored_count} deleted files"))
            }

            OperationType::OrganizeFiles { operations } => {
                let mut success_count = 0;
                let mut errors = Vec::new();

                for op in operations.iter().rev() {
                    match op {
                        FileOperation::Move(move_op) => {
                            if move_op.destination.exists() {
                                match fs::rename(&move_op.destination, &move_op.source) {
                                    Ok(()) => success_count += 1,
                                    Err(e) => errors.push(format!("{}: {}", move_op.source.display(), e)),
                                }
                            }
                        }
                        FileOperation::Copy { destination, .. } => {
                            if destination.exists() {
                                match fs::remove_file(destination) {
                                    Ok(()) => success_count += 1,
                                    Err(e) => errors.push(format!("{}: {}", destination.display(), e)),
                                }
                            }
                        }
                        FileOperation::Delete(del_op) => {
                            if let Some(backup) = &del_op.backup_path {
                                if backup.exists() {
                                    match fs::rename(backup, &del_op.path) {
                                        Ok(()) => success_count += 1,
                                        Err(e) => errors.push(format!("{}: {}", del_op.path.display(), e)),
                                    }
                                }
                            }
                        }
                    }
                }

                if errors.is_empty() {
                    Ok(format!("Undid organization of {success_count} files"))
                } else {
                    Ok(format!("Undid {} operations ({} errors)", success_count, errors.len()))
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    /// Perform the redo operation
    fn redo_operation(operation: &UndoableOperation) -> Result<String> {
        match &operation.operation {
            OperationType::Move { source, destination } => {
                if source.exists() {
                    fs::rename(source, destination)?;
                    Ok(format!("Redid move to {}", destination.display()))
                } else {
                    Err(VisualVaultError::UndoError {
                        message: format!("Cannot redo: {} no longer exists", source.display()),
                    }
                    .into())
                }
            }

            OperationType::Copy { source, destination } => {
                if source.exists() {
                    fs::copy(source, destination)?;
                    Ok(format!("Redid copy to {}", destination.display()))
                } else {
                    Err(VisualVaultError::UndoError {
                        message: format!("Cannot redo: {} no longer exists", source.display()),
                    }
                    .into())
                }
            }

            OperationType::Delete { path, backup_path } => {
                // Redo delete by deleting the file again
                if path.exists() {
                    // If there was a backup, create it again
                    if let Some(backup) = backup_path {
                        fs::copy(path, backup)?;
                    }
                    fs::remove_file(path)?;
                    Ok(format!("Redid deletion of {}", path.display()))
                } else {
                    Ok(format!("File {} already deleted", path.display()))
                }
            }

            OperationType::BatchMove { operations } => {
                let mut success_count = 0;
                let mut errors = Vec::new();

                for op in operations {
                    if op.source.exists() {
                        match fs::rename(&op.source, &op.destination) {
                            Ok(()) => success_count += 1,
                            Err(e) => errors.push(format!("{}: {}", op.source.display(), e)),
                        }
                    }
                }

                if errors.is_empty() {
                    Ok(format!("Redid {success_count} file moves"))
                } else {
                    Ok(format!(
                        "Redid {} moves ({} errors: {})",
                        success_count,
                        errors.len(),
                        errors.join(", ")
                    ))
                }
            }

            OperationType::BatchDelete { operations } => {
                let mut deleted_count = 0;

                for op in operations {
                    if op.path.exists() {
                        // If there was a backup, create it again
                        if let Some(backup) = &op.backup_path {
                            fs::copy(&op.path, backup)?;
                        }
                        fs::remove_file(&op.path)?;
                        deleted_count += 1;
                    }
                }

                Ok(format!("Redid deletion of {deleted_count} files"))
            }

            OperationType::OrganizeFiles { operations } => {
                let mut success_count = 0;
                let mut errors = Vec::new();

                // For redo, we need to re-execute the operations in the original order
                for op in operations {
                    match op {
                        FileOperation::Move(move_op) => {
                            if move_op.source.exists() {
                                // Create destination directory if it doesn't exist
                                if let Some(parent) = move_op.destination.parent() {
                                    fs::create_dir_all(parent)?;
                                }

                                match fs::rename(&move_op.source, &move_op.destination) {
                                    Ok(()) => success_count += 1,
                                    Err(e) => errors.push(format!("{}: {}", move_op.source.display(), e)),
                                }
                            }
                        }
                        FileOperation::Copy { source, destination } => {
                            if source.exists() {
                                // Create destination directory if it doesn't exist
                                if let Some(parent) = destination.parent() {
                                    fs::create_dir_all(parent)?;
                                }

                                match fs::copy(source, destination) {
                                    Ok(_) => success_count += 1,
                                    Err(e) => errors.push(format!("{}: {}", source.display(), e)),
                                }
                            }
                        }
                        FileOperation::Delete(del_op) => {
                            if del_op.path.exists() {
                                // If there was a backup, create it again
                                if let Some(backup) = &del_op.backup_path {
                                    fs::copy(&del_op.path, backup)?;
                                }
                                match fs::remove_file(&del_op.path) {
                                    Ok(()) => success_count += 1,
                                    Err(e) => errors.push(format!("{}: {}", del_op.path.display(), e)),
                                }
                            }
                        }
                    }
                }

                if errors.is_empty() {
                    Ok(format!("Redid organization of {success_count} files"))
                } else {
                    Ok(format!("Redid {} operations ({} errors)", success_count, errors.len()))
                }
            }
        }
    }

    /// Get the undo history
    #[allow(dead_code)]
    pub async fn get_history(&self) -> Vec<UndoableOperation> {
        self.history.read().await.iter().cloned().collect()
    }

    /// Get undoable operations (non-undone operations)
    #[allow(dead_code)]
    pub async fn get_undoable_operations(&self) -> Vec<(usize, UndoableOperation)> {
        let history = self.history.read().await;
        history
            .iter()
            .enumerate()
            .filter(|(_, op)| !op.undone)
            .map(|(i, op)| (i, op.clone()))
            .collect()
    }

    /// Save history to disk
    #[allow(clippy::unwrap_used)]
    async fn save_history(&self) -> Result<()> {
        let history_file = self.config_dir.join("visualvault").join(UNDO_HISTORY_FILE);
        if !history_file.exists() {
            fs::create_dir_all(history_file.parent().unwrap_or_else(|| {
                panic!(
                    "SAVE HISTORY FAILURE: could not create path {}",
                    history_file.parent().unwrap().display()
                )
            }))?;
            fs::File::create(&history_file)?;
        }
        let history: Vec<UndoableOperation> = self.history.read().await.iter().cloned().collect();

        let json = serde_json::to_string_pretty(&history)?;
        fs::write(history_file, json)?;

        Ok(())
    }

    /// Load history from disk
    async fn load_history(&mut self) -> Result<()> {
        let history_file = self.config_dir.join("visualvault").join(UNDO_HISTORY_FILE);

        if history_file.exists() {
            let json = fs::read_to_string(history_file)?;
            let operations: Vec<UndoableOperation> = serde_json::from_str(&json)?;

            let mut history = self.history.write().await;
            history.extend(operations);

            // Maintain max size
            while history.len() > MAX_UNDO_HISTORY {
                history.pop_front();
            }
            drop(history);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_manager() -> Result<(UndoManager, TempDir)> {
        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).await?;
        let manager = UndoManager::new(config_dir);
        Ok((manager, temp_dir))
    }

    async fn create_test_file(dir: &Path, name: &str, content: &str) -> Result<PathBuf> {
        let path = dir.join(name);
        fs::write(&path, content).await?;
        Ok(path)
    }

    #[tokio::test]
    async fn test_undo_move_operation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).await?;

        let manager = UndoManager::new(config_dir);

        // Create test files
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        fs::write(&source, "test content").await?;

        // Move file
        fs::rename(&source, &dest).await?;

        // Record the move
        manager.record_move(&source, &dest).await?;

        // Undo the move
        let result = manager.undo().await?;
        assert!(result.is_some());
        assert!(source.exists());
        assert!(!dest.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_redo_operation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).await?;

        let manager = UndoManager::new(config_dir);

        // Create test files
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        fs::write(&source, "test content").await?;

        // Move file
        fs::rename(&source, &dest).await?;
        manager.record_move(&source, &dest).await?;

        // Undo
        manager.undo().await?;
        assert!(source.exists());

        // Redo
        let result = manager.redo().await?;
        assert!(result.is_some());
        assert!(!source.exists());
        assert!(dest.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_new_undo_manager() -> Result<()> {
        let (manager, _temp_dir) = create_test_manager().await?;

        // Verify initial state
        let history = manager.get_history().await;
        assert!(history.is_empty());

        let undoable = manager.get_undoable_operations().await;
        assert!(undoable.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_record_move_operation() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // Record a move operation
        manager.record_move(&source, &dest).await?;

        // Verify history
        let history = manager.get_history().await;
        assert_eq!(history.len(), 1);

        let op = &history[0];
        assert!(!op.undone);
        assert!(op.description.contains("Moved"));

        match &op.operation {
            OperationType::Move {
                source: s,
                destination: d,
            } => {
                assert_eq!(s, &source);
                assert_eq!(d, &dest);
            }
            _ => panic!("Expected Move operation"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_record_organize_operation() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        let operations = vec![
            FileOperation::Move(MoveOperation {
                source: temp_dir.path().join("file1.txt"),
                destination: temp_dir.path().join("organized/file1.txt"),
            }),
            FileOperation::Move(MoveOperation {
                source: temp_dir.path().join("file2.txt"),
                destination: temp_dir.path().join("organized/file2.txt"),
            }),
        ];

        manager.record_organize(operations.clone()).await?;

        let history = manager.get_history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].description, "Organized 2 files");

        match &history[0].operation {
            OperationType::OrganizeFiles { operations: ops } => {
                assert_eq!(ops.len(), 2);
            }
            _ => panic!("Expected OrganizeFiles operation"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_undo_organize_operation() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        // Create files
        let file1 = create_test_file(temp_dir.path(), "file1.txt", "content1").await?;
        let file2 = create_test_file(temp_dir.path(), "file2.txt", "content2").await?;

        // Create destination directory
        let organized_dir = temp_dir.path().join("organized");
        fs::create_dir_all(&organized_dir).await?;

        let dest1 = organized_dir.join("file1.txt");
        let dest2 = organized_dir.join("file2.txt");

        // Move files
        fs::rename(&file1, &dest1).await?;
        fs::rename(&file2, &dest2).await?;

        // Record the organization
        let operations = vec![
            FileOperation::Move(MoveOperation {
                source: file1.clone(),
                destination: dest1.clone(),
            }),
            FileOperation::Move(MoveOperation {
                source: file2.clone(),
                destination: dest2.clone(),
            }),
        ];
        manager.record_organize(operations).await?;

        // Undo the organization
        let result = manager.undo().await?;
        assert!(result.is_some());
        assert!(result.unwrap().contains("Undid organization of 2 files"));

        // Verify files are back
        assert!(file1.exists());
        assert!(file2.exists());
        assert!(!dest1.exists());
        assert!(!dest2.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_redo_stack_cleared_on_new_operation() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // Create a file to move
        fs::write(&source, "test content").await?;

        // Move the file
        fs::rename(&source, &dest).await?;

        // Record and undo an operation
        manager.record_move(&source, &dest).await?;
        manager.undo().await?;

        // Record a new operation
        let another_source = temp_dir.path().join("another.txt");
        let another_dest = temp_dir.path().join("another_dest.txt");
        manager.record_move(&another_source, &another_dest).await?;

        // Try to redo - should return None because redo stack was cleared
        let result = manager.redo().await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_undo_redo() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        // Record multiple operations
        for i in 0..3 {
            let source = temp_dir.path().join(format!("file{i}.txt"));
            let dest = temp_dir.path().join(format!("dest{i}.txt"));

            // Create the source file
            fs::write(&source, format!("content{i}")).await?;

            // Actually move the file
            fs::rename(&source, &dest).await?;

            manager.record_move(&source, &dest).await?;
        }

        // Undo all
        for _ in 0..3 {
            let result = manager.undo().await?;
            assert!(result.is_some());
        }

        // No more to undo
        let result = manager.undo().await?;
        assert!(result.is_none());

        // Redo all
        for _ in 0..3 {
            let result = manager.redo().await?;
            assert!(result.is_some());
        }

        // No more to redo
        let result = manager.redo().await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_max_history_limit() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        // Record more than MAX_UNDO_HISTORY operations
        for i in 0..100 {
            let source = temp_dir.path().join(format!("src{i}.txt"));
            let dest = temp_dir.path().join(format!("dst{i}.txt"));

            manager.record_move(&source, &dest).await?;
        }

        let history = manager.get_history().await;
        assert_eq!(history.len(), 100);

        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_load_history() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).await?;

        // Create manager and record operations
        {
            let manager = UndoManager::new(config_dir.clone());

            for i in 0..5 {
                let source = temp_dir.path().join(format!("src{i}.txt"));
                let dest = temp_dir.path().join(format!("dst{i}.txt"));

                // Create the source file
                fs::write(&source, format!("content{i}")).await?;

                // Actually move the file
                fs::rename(&source, &dest).await?;
                manager.record_move(&source, &dest).await?;
            }

            // Undo some operations
            manager.undo().await?;
            manager.undo().await?;
        }

        // Create new manager with history loading
        let manager = UndoManager::new_with_history(config_dir).await?;

        let history = manager.get_history().await;
        assert_eq!(history.len(), 5);
        assert!(history[3].undone);
        assert!(history[4].undone);
        assert!(!history[2].undone);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_undoable_operations() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        // Record and perform actual operations
        for i in 0..5 {
            let source = temp_dir.path().join(format!("src{i}.txt"));
            let dest = temp_dir.path().join(format!("dst{i}.txt"));

            // Create the source file
            fs::write(&source, format!("content{i}")).await?;

            // Actually move the file
            fs::rename(&source, &dest).await?;

            // Record the move
            manager.record_move(&source, &dest).await?;
        }

        // Undo some operations
        manager.undo().await?;
        manager.undo().await?;

        let undoable = manager.get_undoable_operations().await;
        assert_eq!(undoable.len(), 3); // Only non-undone operations (0, 1, 2 are not undone; 3, 4 are undone)

        Ok(())
    }

    #[tokio::test]
    async fn test_undo_nonexistent_file() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;

        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // Record move but don't actually move the file
        manager.record_move(&source, &dest).await?;

        // Try to undo - should fail gracefully
        let result = manager.undo().await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_operations() -> Result<()> {
        let (manager, temp_dir) = create_test_manager().await?;
        let manager = Arc::new(manager);

        // Spawn multiple tasks recording operations
        let mut handles = vec![];

        for i in 0..10 {
            let manager_clone = manager.clone();
            let temp_path = temp_dir.path().to_path_buf();

            let handle = tokio::spawn(async move {
                let source = temp_path.join(format!("src{i}.txt"));
                let dest = temp_path.join(format!("dst{i}.txt"));
                manager_clone.record_move(&source, &dest).await
            });

            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await??;
        }

        let history = manager.get_history().await;
        assert_eq!(history.len(), 10);

        Ok(())
    }

    #[tokio::test]
    async fn test_operation_metadata() -> Result<()> {
        let (manager, _temp_dir) = create_test_manager().await?;

        let mut operation = UndoableOperation::new(
            OperationType::Move {
                source: PathBuf::from("/src"),
                destination: PathBuf::from("/dst"),
            },
            "Test operation".to_string(),
        );

        // Add metadata
        operation.metadata = Some(serde_json::json!({
            "user": "test_user",
            "reason": "organization"
        }));

        manager.record_operation(operation).await?;

        let history = manager.get_history().await;
        assert!(history[0].metadata.is_some());

        let metadata = history[0].metadata.as_ref().unwrap();
        assert_eq!(metadata["user"], "test_user");
        assert_eq!(metadata["reason"], "organization");

        Ok(())
    }
}
