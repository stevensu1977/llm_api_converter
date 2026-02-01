//! Logging utilities
//!
//! This module provides custom logging utilities including a size-based
//! rolling file writer for tracing.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Default maximum log file size (10MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default maximum number of rotated files to keep
pub const DEFAULT_MAX_FILES: usize = 5;

/// A size-based rolling file writer
///
/// This writer automatically rotates log files when they exceed a specified size.
/// Files are named with a numeric suffix (e.g., app.log, app.log.1, app.log.2, etc.)
#[derive(Debug)]
pub struct SizeBasedRollingWriter {
    inner: Arc<Mutex<RollingWriterInner>>,
}

#[derive(Debug)]
struct RollingWriterInner {
    /// Base path for the log file
    base_path: PathBuf,
    /// Current log file
    file: Option<File>,
    /// Current file size
    current_size: u64,
    /// Maximum file size before rotation
    max_size: u64,
    /// Maximum number of rotated files to keep
    max_files: usize,
}

impl SizeBasedRollingWriter {
    /// Create a new size-based rolling writer
    ///
    /// # Arguments
    /// * `path` - Base path for the log file (e.g., /var/log/app.log)
    /// * `max_size` - Maximum file size in bytes before rotation (default: 10MB)
    /// * `max_files` - Maximum number of rotated files to keep (default: 10)
    pub fn new(path: impl AsRef<Path>, max_size: u64, max_files: usize) -> io::Result<Self> {
        let base_path = path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = base_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Get current file size if file exists
        let current_size = fs::metadata(&base_path).map(|m| m.len()).unwrap_or(0);

        // Open or create the log file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&base_path)?;

        Ok(Self {
            inner: Arc::new(Mutex::new(RollingWriterInner {
                base_path,
                file: Some(file),
                current_size,
                max_size,
                max_files,
            })),
        })
    }

    /// Create a new size-based rolling writer with default settings (10MB, 10 files)
    pub fn with_defaults(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::new(path, DEFAULT_MAX_FILE_SIZE, DEFAULT_MAX_FILES)
    }
}

impl RollingWriterInner {
    /// Rotate log files
    fn rotate(&mut self) -> io::Result<()> {
        // Close current file
        self.file = None;

        // Rotate existing files
        // app.log.9 -> deleted (if max_files is 10)
        // app.log.8 -> app.log.9
        // ...
        // app.log.1 -> app.log.2
        // app.log -> app.log.1
        for i in (1..self.max_files).rev() {
            let from = self.rotated_path(i);
            let to = self.rotated_path(i + 1);
            if from.exists() {
                if i + 1 >= self.max_files {
                    // Delete the oldest file
                    fs::remove_file(&from).ok();
                } else {
                    fs::rename(&from, &to).ok();
                }
            }
        }

        // Rename current log file
        let rotated = self.rotated_path(1);
        if self.base_path.exists() {
            fs::rename(&self.base_path, &rotated)?;
        }

        // Open new log file
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.base_path)?,
        );
        self.current_size = 0;

        Ok(())
    }

    /// Get the path for a rotated file
    fn rotated_path(&self, index: usize) -> PathBuf {
        let mut path = self.base_path.clone();
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        path.set_file_name(format!("{}.{}", filename, index));
        path
    }
}

impl Write for SizeBasedRollingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();

        // Check if rotation is needed
        if inner.current_size + buf.len() as u64 > inner.max_size {
            inner.rotate()?;
        }

        // Write to file
        if let Some(ref mut file) = inner.file {
            let written = file.write(buf)?;
            inner.current_size += written as u64;
            Ok(written)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Log file not open"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(ref mut file) = inner.file {
            file.flush()
        } else {
            Ok(())
        }
    }
}

impl Clone for SizeBasedRollingWriter {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Make the writer usable with tracing-subscriber
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SizeBasedRollingWriter {
    type Writer = SizeBasedRollingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_rolling_writer_creation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let writer = SizeBasedRollingWriter::with_defaults(&path).unwrap();
        assert!(path.exists());
        drop(writer);
    }

    #[test]
    fn test_rolling_writer_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let mut writer = SizeBasedRollingWriter::with_defaults(&path).unwrap();
        writer.write_all(b"Hello, World!\n").unwrap();
        writer.flush().unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Hello, World!"));
    }

    #[test]
    fn test_rolling_writer_rotation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        // Create a writer with a very small max size (100 bytes)
        let mut writer = SizeBasedRollingWriter::new(&path, 100, 3).unwrap();

        // Write enough data to trigger rotation
        for i in 0..10 {
            writeln!(writer, "Line {}: This is a test log message", i).unwrap();
        }
        writer.flush().unwrap();

        // Check that rotation occurred
        let rotated = dir.path().join("test.log.1");
        assert!(rotated.exists(), "Rotated file should exist");
    }
}
