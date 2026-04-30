use std::path::{Path, PathBuf};

use memmap2::Mmap;
use xxhash_rust::xxh64::xxh64;

use crate::model::{DefContent, DefKind};

// ---------------------------------------------------------------------------
// Binary format constants
// ---------------------------------------------------------------------------

const VERSION: u32 = 4;
const HEADER_SIZE: usize = 8; // version(4) + entry_count(4)
const INDEX_ENTRY_SIZE: usize = 32; // path_hash(8) + mtime(8) + size(8) + offset(4) + len(4)

// Field offsets within an IndexEntry (32 bytes)
const OFF_PATH_HASH: usize = 0;
const OFF_MTIME: usize = 8;
const OFF_FILE_SIZE: usize = 16;
const OFF_DATA_OFFSET: usize = 24;
const OFF_DATA_LEN: usize = 28;

const MAX_SIG_LEN: usize = 1024; // UTF-8 bytes (u16 prefix, max 65535 but we cap)

// ---------------------------------------------------------------------------
// IndexEntry — read from &[u8], zero-allocation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct IndexEntry {
    path_hash: u64,
    mtime_millis: u64,
    file_size: u64,
    data_offset: u32,
    data_len: u32,
}

impl IndexEntry {
    /// Read an IndexEntry from the given byte slice at the specified index position.
    /// Returns None if the slice is too short.
    fn read_from(data: &[u8], index: usize) -> Option<Self> // L41-74

    /// Read only the path_hash from the given byte slice at the specified index position.
    /// Used by binary search for zero-allocation comparison.
    fn read_path_hash(data: &[u8], index: usize) -> Option<u64> // L78-85

    /// Return the path_hash for this entry.
    pub(crate) fn path_hash(&self) -> u64 // L88-90
}

// ---------------------------------------------------------------------------
// CacheOutcome
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)] // Stale variant used for cache statistics, not directly accessed in pipeline
pub(crate) enum CacheOutcome {
    Hit(IndexEntry),
    Stale(IndexEntry),
    NotFound,
}

// ---------------------------------------------------------------------------
// CacheIndex — mmap-backed read-only cache
// ---------------------------------------------------------------------------

pub(crate) struct CacheIndex {
    mapped: Mmap,
    entry_count: u32,
}

impl CacheIndex {
    /// Load a cache file: mmap read-only mapping + header validation.
    /// Returns None if file does not exist or version does not match.
    pub(crate) fn load(path: &Path) -> Option<Self> // L117-162

    /// Binary search for a path_hash in the index section.
    /// Returns CacheOutcome::Hit if found with matching mtime+size,
    /// CacheOutcome::Stale if found but mtime/size mismatch,
    /// CacheOutcome::NotFound if not present.
    pub(crate) fn lookup(&self, path_hash: u64, mtime_millis: u64, file_size: u64) -> CacheOutcome // L168-190

    /// Return the underlying mapped bytes for decode_data usage.
    pub(crate) fn mapped_bytes(&self) -> &[u8] // L193-195
}

// ---------------------------------------------------------------------------
// DATA encoding / decoding
// ---------------------------------------------------------------------------

/// Encode a slice of DefContent into DATA section bytes.
/// Format per entry: kind(u8) + start_line(u32) + end_line(u32) +
///                   sig_len(u16) + sig_bytes + scope_len(u16) + scope_bytes
/// Prefixed by def_count(u32).
pub(crate) fn encode_defs(defs: &[DefContent]) -> Vec<u8> // L206-229

/// Decode DATA section bytes into Vec<DefContent>.
/// Returns None on any format error.
pub(crate) fn decode_data(data: &[u8], entry: &IndexEntry) -> Option<Vec<DefContent>> // L233-241

/// Decode a DATA slice (starting with def_count) into Vec<DefContent>.
fn decode_defs_from_slice(slice: &[u8]) -> Option<Vec<DefContent>> // L244-267

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Truncate definitions' signatures and scopes to the cache format limits.
/// This ensures that non-cache paths (AST parse) return the same truncated
/// values as cache hit paths (decoded from cache), satisfying the equivalence
/// guarantee: "cache hit results are equivalent to AST parse results".
pub(crate) fn truncate_defs(defs: &mut [DefContent]) // L277-283

/// Compute xxh64 hash of a relative path with separator normalization.
/// Path separators are normalized to '/' for cross-platform consistency.
pub(crate) fn path_hash(rel_path: &Path) -> u64 // L287-294

/// Extract mtime in milliseconds since UNIX_EPOCH from file metadata.
pub(crate) fn mtime_millis(metadata: &std::fs::Metadata) -> Option<u64> // L297-306

// ---------------------------------------------------------------------------
// write_cache — merge old entries + new updates, sort, atomic write
// ---------------------------------------------------------------------------

/// Convenience wrapper: build the merged buffer then write atomically.
#[allow(dead_code)]
pub(crate) fn write_cache(
    path: &Path,
    old: Option<&CacheIndex>,
    updates: &std::collections::HashMap<u64, CacheEvent>,
) -> std::io::Result<()> // L314-321

/// Event produced by Phase 4 for each candidate file.
#[derive(Debug, Clone)]
#[allow(dead_code)] // IndexEntry field read in pipeline.rs
pub(crate) enum CacheEvent {
    Hit(IndexEntry),
    Miss {
        path_hash: u64,
        mtime: u64,
        size: u64,
        defs: Vec<DefContent>,
    },
} // L326-334

/// Build the cache binary buffer by merging old entries with new updates.
/// Returns the complete v4 binary buffer ready for atomic write.
/// The caller should drop the old CacheIndex (mmap) before calling `write_cache_atomic`,
/// because Windows locks memory-mapped files and prevents rename.
pub(crate) fn build_cache_buffer(
    old: Option<&CacheIndex>,
    updates: &std::collections::HashMap<u64, CacheEvent>,
) -> std::io::Result<Vec<u8>> // L340-487

/// Write a pre-built cache buffer atomically (temp file + rename).
/// Safe to call after dropping the mmap — no file lock on Windows.
pub(crate) fn write_cache_atomic(path: &Path, buf: &[u8]) -> std::io::Result<()> // L491-502

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Return the largest index <= `max` that falls on a valid UTF-8 char boundary.
/// This prevents splitting a multi-byte character during byte-level truncation.
fn floor_char_boundary(s: &str, mut max: usize) -> usize // L510-518

fn read_u32_from(data: &[u8], pos: &mut usize) -> Option<u32> // L520-527

fn read_u16_string_from(data: &[u8], pos: &mut usize, max_len: usize) -> Option<String> // L529-546

// ---------------------------------------------------------------------------
// Project root detection — shared with pipeline.rs
// ---------------------------------------------------------------------------

/// Find project root by walking up from `start` looking for `.git`.
pub(crate) fn find_project_root(start: &Path) -> Option<PathBuf> // L553-567

// #[cfg(test)] mod tests { ... } // L573-1819 (test module)
