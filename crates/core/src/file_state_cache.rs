use lru::LruCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// State of a file at the time it was last read.
#[derive(Debug, Clone)]
pub struct FileState {
    /// Hash of the file content at read time.
    pub content_hash: u64,
    /// File modification time at read time.
    pub mtime: SystemTime,
    /// Offset used when reading (None if full read).
    pub offset: Option<usize>,
    /// Limit used when reading (None if full read).
    pub limit: Option<usize>,
    /// Whether this was a system-injected partial view (e.g., CLAUDE.md auto-injection).
    pub is_partial_view: bool,
    /// Wall-clock time when the read occurred.
    pub read_time: Instant,
}

/// LRU cache tracking file read state for staleness detection.
///
/// Used by `FileEditTool` and `FileWriteTool` to detect external modifications
/// before writing, and to reject edits on partial-view files.
#[derive(Debug, Clone)]
pub struct FileStateCache {
    cache: LruCache<PathBuf, FileState>,
}

/// Duration within which we apply the hash-fallback check for same-mtime edits.
const MTIME_EDGE_CASE_WINDOW: Duration = Duration::from_secs(2);

impl FileStateCache {
    /// Create a new cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        FileStateCache {
            cache: LruCache::new(
                NonZeroUsize::new(capacity).expect("capacity must be > 0"),
            ),
        }
    }

    /// Record a file read into the cache.
    ///
    /// `content` is hashed (not stored) to save memory.
    /// `is_partial_view` should be `true` for system-injected content.
    pub fn record_read(
        &mut self,
        path: &Path,
        content: &str,
        offset: Option<usize>,
        limit: Option<usize>,
        is_partial_view: bool,
    ) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let content_hash = hash_content(content);
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let state = FileState {
            content_hash,
            mtime,
            offset,
            limit,
            is_partial_view,
            read_time: Instant::now(),
        };
        self.cache.put(canonical, state);
    }

    /// Check whether a file is stale (modified externally since last read).
    ///
    /// Returns:
    /// - `None` if no cache entry exists for this path
    /// - `Some(false)` if the file has not been modified
    /// - `Some(true)` if the file has been modified since last read
    pub fn is_stale(&mut self, path: &Path) -> Option<bool> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        let state = self.cache.get(&canonical)?;

        let current_mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        if current_mtime != state.mtime {
            return Some(true);
        }

        // Edge case: if mtime is the same but the read happened very recently,
        // the filesystem's mtime granularity might hide a modification.
        // Re-read and compare content hash.
        if state.read_time.elapsed() < MTIME_EDGE_CASE_WINDOW {
            if let Ok(content) = std::fs::read_to_string(&canonical) {
                let current_hash = hash_content(&content);
                if current_hash != state.content_hash {
                    return Some(true);
                }
            }
        }

        Some(false)
    }

    /// Get the read state for a file (for checking `is_partial_view`).
    ///
    /// This does NOT refresh the LRU position (uses `peek`).
    pub fn get_read_state(&self, path: &Path) -> Option<&FileState> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.cache.peek(&canonical)
    }

    /// Update the cache after a successful write operation.
    ///
    /// Records the new content hash and mtime, clears `is_partial_view`.
    pub fn record_write(&mut self, path: &Path, content: &str) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let content_hash = hash_content(content);
        let mtime = std::fs::metadata(&canonical)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let state = FileState {
            content_hash,
            mtime,
            offset: None,
            limit: None,
            is_partial_view: false,
            read_time: Instant::now(),
        };
        self.cache.put(canonical, state);
    }

    /// Number of entries currently in the cache.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.cache.len()
    }
}

/// Compute a fast hash of file content using DefaultHasher.
fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!("fsc-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_record_read_and_not_stale() {
        let temp = make_temp_dir("not-stale");
        let file = temp.join("test.txt");
        fs::write(&file, "hello world").unwrap();

        let mut cache = FileStateCache::new(100);
        cache.record_read(&file, "hello world", None, None, false);

        assert_eq!(cache.is_stale(&file), Some(false));
    }

    #[test]
    fn test_stale_after_external_modification() {
        let temp = make_temp_dir("stale");
        let file = temp.join("test.txt");
        fs::write(&file, "original").unwrap();

        let mut cache = FileStateCache::new(100);
        cache.record_read(&file, "original", None, None, false);

        // Modify the file externally. The content changes, so either:
        // - mtime changes (most filesystems) → detected via mtime
        // - mtime stays same (sub-second granularity) → detected via hash fallback
        //   within the 2-second window
        thread::sleep(Duration::from_millis(50));
        fs::write(&file, "modified content").unwrap();

        assert_eq!(cache.is_stale(&file), Some(true));
    }

    #[test]
    fn test_no_entry_returns_none() {
        let temp = make_temp_dir("none");
        let file = temp.join("unknown.txt");
        fs::write(&file, "content").unwrap();

        let mut cache = FileStateCache::new(100);
        assert_eq!(cache.is_stale(&file), None);
    }

    #[test]
    fn test_lru_eviction_at_capacity() {
        let temp = make_temp_dir("eviction");
        let mut cache = FileStateCache::new(3);

        for i in 0..4 {
            let file = temp.join(format!("file{}.txt", i));
            fs::write(&file, format!("content {}", i)).unwrap();
            cache.record_read(&file, &format!("content {}", i), None, None, false);
        }

        // Cache capacity is 3, so file0 should be evicted
        assert_eq!(cache.len(), 3);
        let file0 = temp.join("file0.txt");
        assert!(cache.get_read_state(&file0).is_none());

        // file1, file2, file3 should still be cached
        let file3 = temp.join("file3.txt");
        assert!(cache.get_read_state(&file3).is_some());
    }

    #[test]
    fn test_partial_view_flag() {
        let temp = make_temp_dir("partial");
        let file = temp.join("CLAUDE.md");
        fs::write(&file, "# Instructions\nBe helpful").unwrap();

        let mut cache = FileStateCache::new(100);
        cache.record_read(&file, "# Instructions\nBe helpful", None, None, true);

        let state = cache.get_read_state(&file).unwrap();
        assert!(state.is_partial_view);

        // User-initiated read clears partial view
        cache.record_read(&file, "# Instructions\nBe helpful", None, None, false);
        let state = cache.get_read_state(&file).unwrap();
        assert!(!state.is_partial_view);
    }

    #[test]
    fn test_record_write_updates_entry() {
        let temp = make_temp_dir("write-update");
        let file = temp.join("test.txt");
        fs::write(&file, "original").unwrap();

        let mut cache = FileStateCache::new(100);
        cache.record_read(&file, "original", Some(0), Some(10), true);

        let state = cache.get_read_state(&file).unwrap();
        assert!(state.is_partial_view);
        assert_eq!(state.offset, Some(0));

        // After write, the entry should be updated
        fs::write(&file, "new content").unwrap();
        cache.record_write(&file, "new content");

        let state = cache.get_read_state(&file).unwrap();
        assert!(!state.is_partial_view);
        assert_eq!(state.offset, None);
        assert_eq!(state.limit, None);
    }

    #[test]
    fn test_record_read_with_offset_and_limit() {
        let temp = make_temp_dir("offset-limit");
        let file = temp.join("test.txt");
        fs::write(&file, "line1\nline2\nline3\n").unwrap();

        let mut cache = FileStateCache::new(100);
        cache.record_read(&file, "line2\nline3", Some(1), Some(2), false);

        let state = cache.get_read_state(&file).unwrap();
        assert_eq!(state.offset, Some(1));
        assert_eq!(state.limit, Some(2));
        assert!(!state.is_partial_view);
    }

    #[test]
    fn test_hash_content_deterministic() {
        let hash1 = hash_content("hello world");
        let hash2 = hash_content("hello world");
        let hash3 = hash_content("hello world!");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
