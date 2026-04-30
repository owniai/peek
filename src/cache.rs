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
    fn read_from(data: &[u8], index: usize) -> Option<Self> {
        let start = HEADER_SIZE + index * INDEX_ENTRY_SIZE;
        let end = start + INDEX_ENTRY_SIZE;
        if data.len() < end {
            return None;
        }
        Some(IndexEntry {
            path_hash: u64::from_le_bytes(
                data[start + OFF_PATH_HASH..start + OFF_PATH_HASH + 8]
                    .try_into()
                    .ok()?,
            ),
            mtime_millis: u64::from_le_bytes(
                data[start + OFF_MTIME..start + OFF_MTIME + 8]
                    .try_into()
                    .ok()?,
            ),
            file_size: u64::from_le_bytes(
                data[start + OFF_FILE_SIZE..start + OFF_FILE_SIZE + 8]
                    .try_into()
                    .ok()?,
            ),
            data_offset: u32::from_le_bytes(
                data[start + OFF_DATA_OFFSET..start + OFF_DATA_OFFSET + 4]
                    .try_into()
                    .ok()?,
            ),
            data_len: u32::from_le_bytes(
                data[start + OFF_DATA_LEN..start + OFF_DATA_LEN + 4]
                    .try_into()
                    .ok()?,
            ),
        })
    }

    /// Read only the path_hash from the given byte slice at the specified index position.
    /// Used by binary search for zero-allocation comparison.
    fn read_path_hash(data: &[u8], index: usize) -> Option<u64> {
        let start = HEADER_SIZE + index * INDEX_ENTRY_SIZE;
        let end = start + 8;
        if data.len() < end {
            return None;
        }
        Some(u64::from_le_bytes(data[start..start + 8].try_into().ok()?))
    }

    /// Return the path_hash for this entry.
    pub(crate) fn path_hash(&self) -> u64 {
        self.path_hash
    }
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
    pub(crate) fn load(path: &Path) -> Option<Self> {
        let file = std::fs::File::open(path).ok()?;
        let metadata = file.metadata().ok()?;
        if metadata.len() < HEADER_SIZE as u64 {
            return None;
        }
        let mapped = unsafe { Mmap::map(&file).ok()? };

        // Validate version
        let version = u32::from_le_bytes(mapped[0..4].try_into().ok()?);
        if version != VERSION {
            return None;
        }

        let entry_count = u32::from_le_bytes(mapped[4..8].try_into().ok()?);

        // Validate index section fits within mapped file
        let expected_min_size = HEADER_SIZE + entry_count as usize * INDEX_ENTRY_SIZE;
        if mapped.len() < expected_min_size {
            return None;
        }

        // Validate DATA section: each entry's data_offset + data_len must be within mapped bytes.
        for i in 0..entry_count as usize {
            let base = HEADER_SIZE + i * INDEX_ENTRY_SIZE;
            let data_offset = u32::from_le_bytes(
                mapped[base + OFF_DATA_OFFSET..base + OFF_DATA_OFFSET + 4]
                    .try_into()
                    .ok()?,
            ) as usize;
            let data_len = u32::from_le_bytes(
                mapped[base + OFF_DATA_LEN..base + OFF_DATA_LEN + 4]
                    .try_into()
                    .ok()?,
            ) as usize;
            let end = data_offset.saturating_add(data_len);
            if end > mapped.len() {
                return None;
            }
        }

        Some(CacheIndex {
            mapped,
            entry_count,
        })
    }

    /// Binary search for a path_hash in the index section.
    /// Returns CacheOutcome::Hit if found with matching mtime+size,
    /// CacheOutcome::Stale if found but mtime/size mismatch,
    /// CacheOutcome::NotFound if not present.
    pub(crate) fn lookup(&self, path_hash: u64, mtime_millis: u64, file_size: u64) -> CacheOutcome {
        let mut lo: usize = 0;
        let mut hi: usize = self.entry_count as usize;

        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match IndexEntry::read_path_hash(&self.mapped, mid) {
                Some(h) if h < path_hash => lo = mid + 1,
                Some(h) if h > path_hash => hi = mid,
                Some(_) => {
                    // Found matching path_hash
                    let entry = IndexEntry::read_from(&self.mapped, mid).unwrap();
                    if entry.mtime_millis == mtime_millis && entry.file_size == file_size {
                        return CacheOutcome::Hit(entry);
                    } else {
                        return CacheOutcome::Stale(entry);
                    }
                }
                None => return CacheOutcome::NotFound,
            }
        }
        CacheOutcome::NotFound
    }

    /// Return the underlying mapped bytes for decode_data usage.
    pub(crate) fn mapped_bytes(&self) -> &[u8] {
        &self.mapped
    }
}

// ---------------------------------------------------------------------------
// DATA encoding / decoding
// ---------------------------------------------------------------------------

/// Encode a slice of DefContent into DATA section bytes.
/// Format per entry: kind(u8) + start_line(u32) + end_line(u32) +
///                   sig_len(u16) + sig_bytes + scope_len(u16) + scope_bytes
/// Prefixed by def_count(u32).
pub(crate) fn encode_defs(defs: &[DefContent]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + defs.len() * 40);
    buf.extend_from_slice(&(defs.len() as u32).to_le_bytes());
    for def in defs {
        buf.push(def.kind.to_u8());
        buf.extend_from_slice(&def.lines[0].to_le_bytes());
        buf.extend_from_slice(&def.lines[1].to_le_bytes());

        // Truncate to MAX_SIG_LEN at a valid UTF-8 char boundary.
        // Raw byte truncation may split a multi-byte character, producing
        // invalid UTF-8 that decode cannot read.
        let sig_end = def.signature.len().min(MAX_SIG_LEN);
        let sig_end = floor_char_boundary(&def.signature, sig_end);
        let sig_trunc = &def.signature.as_bytes()[..sig_end];
        buf.extend_from_slice(&(sig_trunc.len() as u16).to_le_bytes());
        buf.extend_from_slice(sig_trunc);

        // Scope is written in full (no truncation).
        let scope_bytes = def.scope.as_bytes();
        buf.extend_from_slice(&(scope_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(scope_bytes);
    }
    buf
}

/// Decode DATA section bytes into Vec<DefContent>.
/// Returns None on any format error.
pub(crate) fn decode_data(data: &[u8], entry: &IndexEntry) -> Option<Vec<DefContent>> {
    let start = entry.data_offset as usize;
    let end = start + entry.data_len as usize;
    if data.len() < end || start > end {
        return None;
    }
    let slice = &data[start..end];
    decode_defs_from_slice(slice)
}

/// Decode a DATA slice (starting with def_count) into Vec<DefContent>.
fn decode_defs_from_slice(slice: &[u8]) -> Option<Vec<DefContent>> {
    let mut pos = 0;
    let count = read_u32_from(slice, &mut pos)? as usize;

    let mut defs = Vec::with_capacity(count);
    for _ in 0..count {
        let kind_byte = *slice.get(pos)?;
        pos += 1;
        let kind = DefKind::from_u8(kind_byte)?;
        let start_line = read_u32_from(slice, &mut pos)?;
        let end_line = read_u32_from(slice, &mut pos)?;

        let sig = read_u16_string_from(slice, &mut pos, MAX_SIG_LEN)?;
        let scope = read_u16_string_from(slice, &mut pos, u16::MAX as usize)?;

        defs.push(DefContent {
            kind,
            lines: [start_line, end_line],
            signature: sig,
            scope,
        });
    }
    Some(defs)
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Truncate definitions' signatures and scopes to the cache format limits.
/// This ensures that non-cache paths (AST parse) return the same truncated
/// values as cache hit paths (decoded from cache), satisfying the equivalence
/// guarantee: "cache hit results are equivalent to AST parse results".
pub(crate) fn truncate_defs(defs: &mut [DefContent]) {
    for def in defs.iter_mut() {
        let sig_end = def.signature.len().min(MAX_SIG_LEN);
        let sig_end = floor_char_boundary(&def.signature, sig_end);
        def.signature.truncate(sig_end);
    }
}

/// Compute xxh64 hash of a relative path with separator normalization.
/// Path separators are normalized to '/' for cross-platform consistency.
pub(crate) fn path_hash(rel_path: &Path) -> u64 {
    let lossy = rel_path.to_string_lossy();
    #[cfg(windows)]
    let normalized = lossy.replace('\\', "/");
    #[cfg(not(windows))]
    let normalized = lossy;
    xxh64(normalized.as_bytes(), 0)
}

/// Extract mtime in milliseconds since UNIX_EPOCH from file metadata.
pub(crate) fn mtime_millis(metadata: &std::fs::Metadata) -> Option<u64> {
    Some(
        metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64,
    )
}

// ---------------------------------------------------------------------------
// write_cache — merge old entries + new updates, sort, atomic write
// ---------------------------------------------------------------------------

/// Convenience wrapper: build the merged buffer then write atomically.
#[allow(dead_code)]
pub(crate) fn write_cache(
    path: &Path,
    old: Option<&CacheIndex>,
    updates: &std::collections::HashMap<u64, CacheEvent>,
) -> std::io::Result<()> {
    let buf = build_cache_buffer(old, updates)?;
    write_cache_atomic(path, &buf)
}

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
}

/// Build the cache binary buffer by merging old entries with new updates.
/// Returns the complete v4 binary buffer ready for atomic write.
/// The caller should drop the old CacheIndex (mmap) before calling `write_cache_atomic`,
/// because Windows locks memory-mapped files and prevents rename.
pub(crate) fn build_cache_buffer(
    old: Option<&CacheIndex>,
    updates: &std::collections::HashMap<u64, CacheEvent>,
) -> std::io::Result<Vec<u8>> {
    struct NewEntry {
        path_hash: u64,
        mtime_millis: u64,
        file_size: u64,
        data: Vec<u8>,
    }

    let mut new_entries: Vec<NewEntry> = Vec::new();
    let mut processed_hashes: std::collections::HashSet<u64> = std::collections::HashSet::new();

    // Process entries from old index
    if let Some(old_idx) = old {
        for i in 0..old_idx.entry_count {
            let entry =
                IndexEntry::read_from(old_idx.mapped_bytes(), i as usize).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid index entry")
                })?;

            if let Some(event) = updates.get(&entry.path_hash) {
                processed_hashes.insert(entry.path_hash);
                match event {
                    CacheEvent::Hit(_) => {
                        let start = entry.data_offset as usize;
                        let end = start + entry.data_len as usize;
                        let mapped = old_idx.mapped_bytes();
                        if end > mapped.len() {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "cache entry data exceeds mapped region",
                            ));
                        }
                        new_entries.push(NewEntry {
                            path_hash: entry.path_hash,
                            mtime_millis: entry.mtime_millis,
                            file_size: entry.file_size,
                            data: mapped[start..end].to_vec(),
                        });
                    }
                    CacheEvent::Miss {
                        path_hash,
                        mtime,
                        size,
                        defs,
                    } => {
                        new_entries.push(NewEntry {
                            path_hash: *path_hash,
                            mtime_millis: *mtime,
                            file_size: *size,
                            data: encode_defs(defs),
                        });
                    }
                }
            } else {
                let start = entry.data_offset as usize;
                let end = start + entry.data_len as usize;
                let mapped = old_idx.mapped_bytes();
                if end > mapped.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cache entry data exceeds mapped region",
                    ));
                }
                new_entries.push(NewEntry {
                    path_hash: entry.path_hash,
                    mtime_millis: entry.mtime_millis,
                    file_size: entry.file_size,
                    data: mapped[start..end].to_vec(),
                });
            }
        }
    }

    // Add new entries (in updates but not in old index)
    for (hash, event) in updates {
        if processed_hashes.contains(hash) {
            continue;
        }
        if let CacheEvent::Miss {
            path_hash,
            mtime,
            size,
            defs,
        } = event
        {
            new_entries.push(NewEntry {
                path_hash: *path_hash,
                mtime_millis: *mtime,
                file_size: *size,
                data: encode_defs(defs),
            });
        }
    }

    new_entries.sort_by_key(|e| e.path_hash);

    // Build the output buffer
    let data_section_offset = HEADER_SIZE + new_entries.len() * INDEX_ENTRY_SIZE;
    let mut buf = Vec::with_capacity(
        data_section_offset + new_entries.iter().map(|e| e.data.len()).sum::<usize>(),
    );

    buf.resize(data_section_offset, 0);

    let mut current_data_offset = data_section_offset as u64;
    let mut index_entries: Vec<(u64, u64, u64, u32, u32)> = Vec::with_capacity(new_entries.len());
    for entry in &new_entries {
        let data_len = entry.data.len() as u32;
        let offset_u32 = if current_data_offset > u32::MAX as u64 {
            return Err(std::io::Error::other(
                "cache file exceeds 4 GB (data_offset overflow)",
            ));
        } else {
            current_data_offset as u32
        };
        index_entries.push((
            entry.path_hash,
            entry.mtime_millis,
            entry.file_size,
            offset_u32,
            data_len,
        ));
        buf.extend_from_slice(&entry.data);
        current_data_offset += data_len as u64;
    }

    for (i, (path_hash, mtime, file_size, data_offset, data_len)) in
        index_entries.iter().enumerate()
    {
        let base = HEADER_SIZE + i * INDEX_ENTRY_SIZE;
        buf[base + OFF_PATH_HASH..base + OFF_PATH_HASH + 8]
            .copy_from_slice(&path_hash.to_le_bytes());
        buf[base + OFF_MTIME..base + OFF_MTIME + 8].copy_from_slice(&mtime.to_le_bytes());
        buf[base + OFF_FILE_SIZE..base + OFF_FILE_SIZE + 8]
            .copy_from_slice(&file_size.to_le_bytes());
        buf[base + OFF_DATA_OFFSET..base + OFF_DATA_OFFSET + 4]
            .copy_from_slice(&data_offset.to_le_bytes());
        buf[base + OFF_DATA_LEN..base + OFF_DATA_LEN + 4].copy_from_slice(&data_len.to_le_bytes());
    }

    buf[0..4].copy_from_slice(&VERSION.to_le_bytes());
    buf[4..8].copy_from_slice(&(new_entries.len() as u32).to_le_bytes());

    Ok(buf)
}

/// Write a pre-built cache buffer atomically (temp file + rename).
/// Safe to call after dropping the mmap — no file lock on Windows.
pub(crate) fn write_cache_atomic(path: &Path, buf: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, buf)?;
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Return the largest index <= `max` that falls on a valid UTF-8 char boundary.
/// This prevents splitting a multi-byte character during byte-level truncation.
fn floor_char_boundary(s: &str, mut max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(max) {
        max -= 1;
    }
    max
}

fn read_u32_from(data: &[u8], pos: &mut usize) -> Option<u32> {
    if data.len() < *pos + 4 {
        return None;
    }
    let v = u32::from_le_bytes(data[*pos..*pos + 4].try_into().ok()?);
    *pos += 4;
    Some(v)
}

fn read_u16_string_from(data: &[u8], pos: &mut usize, max_len: usize) -> Option<String> {
    if data.len() < *pos + 2 {
        return None;
    }
    let len = u16::from_le_bytes(data[*pos..*pos + 2].try_into().ok()?) as usize;
    *pos += 2;
    if len > max_len {
        return None;
    }
    if data.len() < *pos + len {
        return None;
    }
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .ok()?
        .to_string();
    *pos += len;
    Some(s)
}

// ---------------------------------------------------------------------------
// Project root detection — shared with pipeline.rs
// ---------------------------------------------------------------------------

/// Find project root by walking up from `start` looking for `.git`.
pub(crate) fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(start)
    };

    loop {
        let git_path = current.join(".git");
        if git_path.exists() {
            return Some(current);
        }
        current = current.parent()?.to_path_buf();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

    // =======================================================================
    // Task 2: Binary format constants & IndexEntry
    // =======================================================================

    #[test]
    fn header_size_is_8_bytes() {
        assert_eq!(HEADER_SIZE, 8);
    }

    #[test]
    fn index_entry_size_is_32_bytes() {
        assert_eq!(INDEX_ENTRY_SIZE, 32);
    }

    #[test]
    fn version_is_4() {
        assert_eq!(VERSION, 4);
    }

    #[test]
    fn index_entry_read_from_valid_data() {
        let mut data = vec![0u8; HEADER_SIZE + INDEX_ENTRY_SIZE];
        // Write version + entry_count in header
        data[0..4].copy_from_slice(&VERSION.to_le_bytes());
        data[4..8].copy_from_slice(&1u32.to_le_bytes());
        // Write index entry at position 0
        let base = HEADER_SIZE;
        data[base + OFF_PATH_HASH..base + OFF_PATH_HASH + 8]
            .copy_from_slice(&0xABCDEF0123456789u64.to_le_bytes());
        data[base + OFF_MTIME..base + OFF_MTIME + 8].copy_from_slice(&1000u64.to_le_bytes());
        data[base + OFF_FILE_SIZE..base + OFF_FILE_SIZE + 8].copy_from_slice(&200u64.to_le_bytes());
        data[base + OFF_DATA_OFFSET..base + OFF_DATA_OFFSET + 4]
            .copy_from_slice(&300u32.to_le_bytes());
        data[base + OFF_DATA_LEN..base + OFF_DATA_LEN + 4].copy_from_slice(&50u32.to_le_bytes());

        let entry = IndexEntry::read_from(&data, 0).unwrap();
        assert_eq!(entry.path_hash, 0xABCDEF0123456789u64);
        assert_eq!(entry.mtime_millis, 1000);
        assert_eq!(entry.file_size, 200);
        assert_eq!(entry.data_offset, 300);
        assert_eq!(entry.data_len, 50);
    }

    #[test]
    fn index_entry_read_from_truncated_data_returns_none() {
        let data = vec![0u8; HEADER_SIZE + 16]; // Only 16 bytes, need 32
        assert!(IndexEntry::read_from(&data, 0).is_none());
    }

    #[test]
    fn index_entry_read_path_hash() {
        let mut data = vec![0u8; HEADER_SIZE + INDEX_ENTRY_SIZE];
        let base = HEADER_SIZE;
        data[base..base + 8].copy_from_slice(&0xDEADBEEFu64.to_le_bytes());

        assert_eq!(IndexEntry::read_path_hash(&data, 0), Some(0xDEADBEEFu64));
    }

    // =======================================================================
    // Task 3: DATA encoding
    // =======================================================================

    fn make_defs() -> Vec<DefContent> {
        vec![
            DefContent {
                kind: DefKind::Function,
                lines: [10, 25],
                signature: "fn process(data: &str)".to_string(),
                scope: "handler::process".to_string(),
            },
            DefContent {
                kind: DefKind::Class,
                lines: [5, 15],
                signature: "struct Config".to_string(),
                scope: "Config".to_string(),
            },
        ]
    }

    #[test]
    fn encode_defs_empty() {
        let encoded = encode_defs(&[]);
        // def_count(4) = 0
        assert_eq!(encoded.len(), 4);
        assert_eq!(u32::from_le_bytes(encoded[0..4].try_into().unwrap()), 0);
    }

    #[test]
    fn encode_defs_single() {
        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 10],
            signature: "fn foo()".to_string(),
            scope: "foo".to_string(),
        }];
        let encoded = encode_defs(&defs);

        // Verify def_count
        assert_eq!(u32::from_le_bytes(encoded[0..4].try_into().unwrap()), 1);

        // Verify kind byte
        assert_eq!(encoded[4], DefKind::Function.to_u8());

        // Verify start_line
        assert_eq!(u32::from_le_bytes(encoded[5..9].try_into().unwrap()), 1);

        // Verify end_line
        assert_eq!(u32::from_le_bytes(encoded[9..13].try_into().unwrap()), 10);
    }

    #[test]
    fn encode_defs_multiple_starts_with_count() {
        let defs = make_defs();
        let encoded = encode_defs(&defs);
        let count = u32::from_le_bytes(encoded[0..4].try_into().unwrap());
        assert_eq!(count, 2);
    }

    #[test]
    fn encode_defs_preserves_kind_for_all_variants() {
        for &kind in DefKind::all() {
            let defs = vec![DefContent {
                kind,
                lines: [1, 2],
                signature: "s".to_string(),
                scope: "sc".to_string(),
            }];
            let encoded = encode_defs(&defs);
            assert_eq!(encoded[4], kind.to_u8(), "kind mismatch for {:?}", kind);
        }
    }

    // =======================================================================
    // Task 4: DATA decoding (round-trip)
    // =======================================================================

    #[test]
    fn decode_round_trip_empty() {
        let encoded = encode_defs(&[]);
        // Create a fake IndexEntry pointing to the encoded data
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn decode_round_trip_single_def() {
        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [10, 25],
            signature: "fn process(data: &str)".to_string(),
            scope: "handler::process".to_string(),
        }];
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].kind, DefKind::Function);
        assert_eq!(decoded[0].lines, [10, 25]);
        assert_eq!(decoded[0].signature, "fn process(data: &str)");
        assert_eq!(decoded[0].scope, "handler::process");
    }

    #[test]
    fn decode_round_trip_multiple_defs() {
        let defs = make_defs();
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry).unwrap();
        assert_eq!(decoded.len(), defs.len());
        for (a, b) in defs.iter().zip(decoded.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.lines, b.lines);
            assert_eq!(a.signature, b.signature);
            assert_eq!(a.scope, b.scope);
        }
    }

    #[test]
    fn decode_round_trip_all_def_kinds() {
        for &kind in DefKind::all() {
            let defs = vec![DefContent {
                kind,
                lines: [1, 2],
                signature: "sig".to_string(),
                scope: "scope".to_string(),
            }];
            let encoded = encode_defs(&defs);
            let entry = IndexEntry {
                path_hash: 1,
                mtime_millis: 0,
                file_size: 0,
                data_offset: 0,
                data_len: encoded.len() as u32,
            };
            let decoded = decode_data(&encoded, &entry).unwrap();
            assert_eq!(decoded[0].kind, kind, "round-trip failed for {:?}", kind);
        }
    }

    #[test]
    fn decode_returns_none_on_truncated_data() {
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: 1,
        };
        assert!(decode_data(&[0u8; 1], &entry).is_none());
    }

    #[test]
    fn decode_returns_none_on_invalid_kind() {
        let mut data = vec![];
        data.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        data.push(255); // invalid kind
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: data.len() as u32,
        };
        assert!(decode_data(&data, &entry).is_none());
    }

    #[test]
    fn decode_returns_none_on_truncated_string() {
        let mut data = vec![];
        data.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        data.push(0); // Function kind
        data.extend_from_slice(&1u32.to_le_bytes()); // start_line
        data.extend_from_slice(&2u32.to_le_bytes()); // end_line
        data.extend_from_slice(&100u16.to_le_bytes()); // sig_len = 100 but no bytes follow
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: data.len() as u32,
        };
        assert!(decode_data(&data, &entry).is_none());
    }

    #[test]
    fn decode_returns_none_on_count_exceeds_available_data() {
        // Count claims many definitions but the data is too short to hold them.
        let mut data = vec![];
        data.extend_from_slice(&1000u32.to_le_bytes()); // count = 1000
        // Only provide 1 byte of definition data (need at least 1 byte per def for kind)
        data.push(0);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: data.len() as u32,
        };
        assert!(decode_data(&data, &entry).is_none());
    }

    #[test]
    fn decode_returns_none_on_offset_out_of_bounds() {
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 1000,
            data_len: 10,
        };
        assert!(decode_data(&[0u8; 10], &entry).is_none());
    }

    #[test]
    fn decode_round_trip_empty_strings() {
        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: String::new(),
            scope: String::new(),
        }];
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry).unwrap();
        assert_eq!(decoded[0].signature, "");
        assert_eq!(decoded[0].scope, "");
    }

    #[test]
    fn decode_round_trip_unicode_strings() {
        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "fn 日本語テスト()".to_string(),
            scope: "モジュール::関数".to_string(),
        }];
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry).unwrap();
        assert_eq!(decoded[0].signature, "fn 日本語テスト()");
        assert_eq!(decoded[0].scope, "モジュール::関数");
    }

    #[test]
    fn encode_defs_truncates_at_utf8_boundary() {
        // Regression: truncation must not split a multi-byte UTF-8 character.
        // CJK character '日' is 3 bytes in UTF-8. 341 * 3 = 1023 bytes.
        // Adding one more CJK char would exceed MAX_SIG_LEN=1024.
        // Truncation must fall at byte 1023 (a valid char boundary), not 1024 (mid-char).
        let cjk_char = '日';
        assert_eq!(cjk_char.len_utf8(), 3);
        let char_count = MAX_SIG_LEN / 3 + 1; // 342 chars = 1026 bytes
        let long_sig: String = std::iter::repeat_n(cjk_char, char_count).collect();
        assert!(long_sig.len() > MAX_SIG_LEN);

        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: long_sig,
            scope: "scope".to_string(),
        }];
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry);
        assert!(
            decoded.is_some(),
            "decode must succeed — truncation should be at a valid UTF-8 boundary"
        );
        let decoded = decoded.unwrap();
        // Truncated to 341 CJK chars = 1023 bytes (largest multiple of 3 <= MAX_SIG_LEN)
        assert!(decoded[0].signature.len() <= MAX_SIG_LEN);
        assert!(
            decoded[0]
                .signature
                .is_char_boundary(decoded[0].signature.len())
        );
    }

    #[test]
    fn encode_defs_preserves_full_scope_no_truncation() {
        // Scope is no longer truncated — it should round-trip in full.
        let cjk_char = '漢';
        let long_scope: String = std::iter::repeat_n(cjk_char, 500).collect(); // 1500 bytes

        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "sig".to_string(),
            scope: long_scope.clone(),
        }];
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry);
        assert!(decoded.is_some(), "decode must succeed for long scope");
        let decoded = decoded.unwrap();
        assert_eq!(
            decoded[0].scope, long_scope,
            "scope must be preserved in full"
        );
    }

    #[test]
    fn encode_decode_round_trip_signature_exceeds_max_sig_len() {
        // Regression test: signature exceeding MAX_SIG_LEN must still round-trip.
        // Encode should truncate to MAX_SIG_LEN so decode can succeed.
        let long_sig = "x".repeat(MAX_SIG_LEN + 100);
        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: long_sig.clone(),
            scope: "scope".to_string(),
        }];
        let encoded = encode_defs(&defs);
        // Should NOT return empty Vec (which indicates encoding failure)
        assert!(
            !encoded.is_empty(),
            "encode_defs should not return empty for long signature"
        );
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry);
        assert!(
            decoded.is_some(),
            "decode_data should succeed for truncated long signature"
        );
        // Signature should be truncated to MAX_SIG_LEN bytes (which is a valid UTF-8 boundary for ASCII)
        let decoded = decoded.unwrap();
        assert_eq!(decoded[0].signature.len(), MAX_SIG_LEN);
    }

    #[test]
    fn encode_decode_round_trip_scope_not_truncated() {
        // Scope is no longer truncated — long scope must round-trip in full.
        let long_scope = "y".repeat(2000);
        let defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "sig".to_string(),
            scope: long_scope.clone(),
        }];
        let encoded = encode_defs(&defs);
        assert!(
            !encoded.is_empty(),
            "encode_defs should not return empty for long scope"
        );
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry);
        assert!(
            decoded.is_some(),
            "decode_data should succeed for long scope"
        );
        let decoded = decoded.unwrap();
        assert_eq!(
            decoded[0].scope, long_scope,
            "scope must be preserved in full"
        );
    }

    #[test]
    fn truncate_defs_truncates_long_signature_only() {
        let long_scope = "y".repeat(2000);
        let mut defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "x".repeat(MAX_SIG_LEN + 100),
            scope: long_scope.clone(),
        }];
        truncate_defs(&mut defs);
        assert!(defs[0].signature.len() <= MAX_SIG_LEN);
        assert_eq!(defs[0].scope, long_scope, "scope must not be truncated");
        assert!(defs[0].signature.is_char_boundary(defs[0].signature.len()));
    }

    #[test]
    fn truncate_defs_preserves_short_values() {
        let mut defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: "fn foo()".to_string(),
            scope: "foo".to_string(),
        }];
        truncate_defs(&mut defs);
        assert_eq!(defs[0].signature, "fn foo()");
        assert_eq!(defs[0].scope, "foo");
    }

    #[test]
    fn truncate_defs_matches_encode_decode_round_trip() {
        // After truncate_defs + encode_defs + decode_data, values must be identical.
        // Scope is no longer truncated, so it should round-trip in full.
        let long_sig = "x".repeat(MAX_SIG_LEN + 50);
        let long_scope = "y".repeat(2000);
        let mut defs = vec![DefContent {
            kind: DefKind::Function,
            lines: [1, 1],
            signature: long_sig,
            scope: long_scope,
        }];
        truncate_defs(&mut defs);

        // Scope must not be truncated — signature should be truncated.
        assert_eq!(
            defs[0].scope.len(),
            2000,
            "scope must not be truncated by truncate_defs"
        );
        assert!(defs[0].signature.len() <= MAX_SIG_LEN);

        // Encode and decode
        let encoded = encode_defs(&defs);
        let entry = IndexEntry {
            path_hash: 1,
            mtime_millis: 0,
            file_size: 0,
            data_offset: 0,
            data_len: encoded.len() as u32,
        };
        let decoded = decode_data(&encoded, &entry).unwrap();
        assert_eq!(
            decoded[0].signature, defs[0].signature,
            "truncate_defs output must match encode+decode output"
        );
        assert_eq!(
            decoded[0].scope, defs[0].scope,
            "truncate_defs output must match encode+decode output"
        );
    }

    // =======================================================================
    // Task 5: Helper functions
    // =======================================================================

    #[test]
    fn path_hash_deterministic() {
        let h1 = path_hash(Path::new("src/main.rs"));
        let h2 = path_hash(Path::new("src/main.rs"));
        assert_eq!(h1, h2);
    }

    #[test]
    fn path_hash_different_paths() {
        let h1 = path_hash(Path::new("src/main.rs"));
        let h2 = path_hash(Path::new("src/lib.rs"));
        assert_ne!(h1, h2);
    }

    #[test]
    #[cfg(windows)]
    fn path_hash_cross_platform_consistent() {
        let forward = path_hash(Path::new("src/main.rs"));
        let backslash = path_hash(Path::new("src\\main.rs"));
        assert_eq!(
            forward, backslash,
            "same file should produce same hash regardless of separator"
        );
    }

    #[test]
    fn path_hash_empty_path() {
        let h = path_hash(Path::new(""));
        assert_ne!(h, 0, "even empty path should produce a non-trivial hash");
    }

    #[test]
    fn mtime_millis_returns_some_for_real_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let metadata = tmp.as_file().metadata().unwrap();
        let mtime = mtime_millis(&metadata);
        assert!(mtime.is_some());
        assert!(mtime.unwrap() > 0);
    }

    // =======================================================================
    // Task 6: CacheIndex::load
    // =======================================================================

    #[test]
    fn load_returns_none_for_nonexistent_file() {
        let result = CacheIndex::load(Path::new("/nonexistent/path/cache.bin"));
        assert!(result.is_none());
    }

    #[test]
    fn load_returns_none_for_empty_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let result = CacheIndex::load(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn load_returns_none_for_wrong_version() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut data = vec![0u8; HEADER_SIZE];
        data[0..4].copy_from_slice(&99u32.to_le_bytes()); // wrong version
        data[4..8].copy_from_slice(&0u32.to_le_bytes()); // entry_count = 0
        tmp.as_file().write_all(&data).unwrap();
        tmp.as_file().flush().unwrap();

        let result = CacheIndex::load(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn load_succeeds_for_valid_header() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut data = vec![0u8; HEADER_SIZE];
        data[0..4].copy_from_slice(&VERSION.to_le_bytes());
        data[4..8].copy_from_slice(&0u32.to_le_bytes()); // entry_count = 0
        tmp.as_file().write_all(&data).unwrap();
        tmp.as_file().flush().unwrap();

        let result = CacheIndex::load(tmp.path());
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(idx.entry_count, 0);
    }

    #[test]
    fn load_succeeds_with_entries() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut data = vec![0u8; HEADER_SIZE + INDEX_ENTRY_SIZE];
        data[0..4].copy_from_slice(&VERSION.to_le_bytes());
        data[4..8].copy_from_slice(&1u32.to_le_bytes()); // entry_count = 1
        // Write a dummy index entry with data_offset and data_len pointing within file
        let base = HEADER_SIZE;
        data[base + OFF_PATH_HASH..base + OFF_PATH_HASH + 8].copy_from_slice(&123u64.to_le_bytes());
        data[base + OFF_DATA_OFFSET..base + OFF_DATA_OFFSET + 4]
            .copy_from_slice(&((HEADER_SIZE + INDEX_ENTRY_SIZE) as u32).to_le_bytes());
        data[base + OFF_DATA_LEN..base + OFF_DATA_LEN + 4].copy_from_slice(&0u32.to_le_bytes());
        tmp.as_file().write_all(&data).unwrap();
        tmp.as_file().flush().unwrap();

        let idx = CacheIndex::load(tmp.path()).unwrap();
        assert_eq!(idx.entry_count, 1);
    }

    #[test]
    fn load_returns_none_for_truncated_data_section() {
        // Build a valid cache file, then truncate so DATA section is incomplete.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        let defs = vec![make_def(DefKind::Function, 1, 10, "fn foo()", "foo")];
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            42,
            CacheEvent::Miss {
                path_hash: 42,
                mtime: 1000,
                size: 200,
                defs,
            },
        );
        write_cache(&path, None, &updates).unwrap();

        // Read the file, find an entry's data_offset+data_len, truncate before that
        let full = std::fs::read(&path).unwrap();
        let entry_count = u32::from_le_bytes(full[4..8].try_into().unwrap());
        assert_eq!(entry_count, 1);
        let data_offset = u32::from_le_bytes(
            full[HEADER_SIZE + OFF_DATA_OFFSET..HEADER_SIZE + OFF_DATA_OFFSET + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let data_len = u32::from_le_bytes(
            full[HEADER_SIZE + OFF_DATA_LEN..HEADER_SIZE + OFF_DATA_LEN + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        assert!(data_len > 0);

        // Truncate so the file ends in the middle of the DATA section
        let truncated_len = data_offset + data_len / 2;
        let truncated = &full[..truncated_len];
        std::fs::write(&path, truncated).unwrap();

        // load should reject the truncated file
        assert!(
            CacheIndex::load(&path).is_none(),
            "load should reject cache with truncated DATA section"
        );
    }

    #[test]
    fn load_succeeds_with_valid_data_section() {
        // A cache file with a complete DATA section should load fine.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        let defs = vec![make_def(DefKind::Function, 1, 10, "fn foo()", "foo")];
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            42,
            CacheEvent::Miss {
                path_hash: 42,
                mtime: 1000,
                size: 200,
                defs,
            },
        );
        write_cache(&path, None, &updates).unwrap();

        let idx = CacheIndex::load(&path).unwrap();
        assert_eq!(idx.entry_count, 1);
        match idx.lookup(42, 1000, 200) {
            CacheOutcome::Hit(_) => {}
            other => panic!("expected Hit, got {:?}", other),
        }
    }

    // =======================================================================
    // Task 7: CacheIndex::lookup (binary search)
    // =======================================================================

    /// Helper: build a cache file with sorted path_hashes for lookup tests.
    fn build_test_cache(hashes: &[(u64, u64, u64)]) -> tempfile::NamedTempFile {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let entry_count = hashes.len() as u32;
        let data_offset = HEADER_SIZE + hashes.len() * INDEX_ENTRY_SIZE;
        let mut data = vec![0u8; data_offset + hashes.len() * 4]; // 4 bytes dummy data per entry

        data[0..4].copy_from_slice(&VERSION.to_le_bytes());
        data[4..8].copy_from_slice(&entry_count.to_le_bytes());

        for (i, (path_hash, mtime, size)) in hashes.iter().enumerate() {
            let base = HEADER_SIZE + i * INDEX_ENTRY_SIZE;
            data[base + OFF_PATH_HASH..base + OFF_PATH_HASH + 8]
                .copy_from_slice(&path_hash.to_le_bytes());
            data[base + OFF_MTIME..base + OFF_MTIME + 8].copy_from_slice(&mtime.to_le_bytes());
            data[base + OFF_FILE_SIZE..base + OFF_FILE_SIZE + 8]
                .copy_from_slice(&size.to_le_bytes());
            data[base + OFF_DATA_OFFSET..base + OFF_DATA_OFFSET + 4]
                .copy_from_slice(&((data_offset + i * 4) as u32).to_le_bytes());
            data[base + OFF_DATA_LEN..base + OFF_DATA_LEN + 4].copy_from_slice(&4u32.to_le_bytes());
        }

        tmp.as_file().write_all(&data).unwrap();
        tmp.as_file().flush().unwrap();
        tmp
    }

    #[test]
    fn lookup_hit_single_entry() {
        let tmp = build_test_cache(&[(100, 1000, 200)]);
        let idx = CacheIndex::load(tmp.path()).unwrap();
        match idx.lookup(100, 1000, 200) {
            CacheOutcome::Hit(entry) => {
                assert_eq!(entry.path_hash, 100);
                assert_eq!(entry.mtime_millis, 1000);
                assert_eq!(entry.file_size, 200);
            }
            other => panic!("expected Hit, got {:?}", other),
        }
    }

    #[test]
    fn lookup_stale_mtime_mismatch() {
        let tmp = build_test_cache(&[(100, 1000, 200)]);
        let idx = CacheIndex::load(tmp.path()).unwrap();
        match idx.lookup(100, 999, 200) {
            CacheOutcome::Stale(entry) => {
                assert_eq!(entry.path_hash, 100);
            }
            other => panic!("expected Stale, got {:?}", other),
        }
    }

    #[test]
    fn lookup_stale_size_mismatch() {
        let tmp = build_test_cache(&[(100, 1000, 200)]);
        let idx = CacheIndex::load(tmp.path()).unwrap();
        match idx.lookup(100, 1000, 999) {
            CacheOutcome::Stale(_) => {}
            other => panic!("expected Stale, got {:?}", other),
        }
    }

    #[test]
    fn lookup_not_found() {
        let tmp = build_test_cache(&[(100, 1000, 200)]);
        let idx = CacheIndex::load(tmp.path()).unwrap();
        match idx.lookup(999, 1000, 200) {
            CacheOutcome::NotFound => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn lookup_empty_index() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut data = vec![0u8; HEADER_SIZE];
        data[0..4].copy_from_slice(&VERSION.to_le_bytes());
        data[4..8].copy_from_slice(&0u32.to_le_bytes());
        tmp.as_file().write_all(&data).unwrap();
        tmp.as_file().flush().unwrap();

        let idx = CacheIndex::load(tmp.path()).unwrap();
        match idx.lookup(123, 0, 0) {
            CacheOutcome::NotFound => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn lookup_multiple_entries() {
        // Sorted: 10, 20, 30, 40, 50
        let hashes: Vec<(u64, u64, u64)> = (0..5)
            .map(|i| {
                let h = (i + 1) * 10;
                (h, h * 100, h * 10)
            })
            .collect();
        let tmp = build_test_cache(&hashes);
        let idx = CacheIndex::load(tmp.path()).unwrap();

        // Hit middle
        match idx.lookup(30, 3000, 300) {
            CacheOutcome::Hit(e) => assert_eq!(e.path_hash, 30),
            other => panic!("expected Hit for 30, got {:?}", other),
        }

        // Hit first
        match idx.lookup(10, 1000, 100) {
            CacheOutcome::Hit(e) => assert_eq!(e.path_hash, 10),
            other => panic!("expected Hit for 10, got {:?}", other),
        }

        // Hit last
        match idx.lookup(50, 5000, 500) {
            CacheOutcome::Hit(e) => assert_eq!(e.path_hash, 50),
            other => panic!("expected Hit for 50, got {:?}", other),
        }

        // Not found
        match idx.lookup(25, 0, 0) {
            CacheOutcome::NotFound => {}
            other => panic!("expected NotFound for 25, got {:?}", other),
        }
    }

    // =======================================================================
    // Task 8: write_cache (merge logic)
    // =======================================================================

    /// Test helper: build buffer + atomic write. Tests the same code path
    /// the pipeline uses (build_cache_buffer → write_cache_atomic) without
    /// depending on a convenience wrapper.
    fn write_cache(
        path: &Path,
        old: Option<&CacheIndex>,
        updates: &std::collections::HashMap<u64, CacheEvent>,
    ) -> std::io::Result<()> {
        let buf = build_cache_buffer(old, updates)?;
        write_cache_atomic(path, &buf)
    }

    fn make_def(kind: DefKind, start: u32, end: u32, sig: &str, scope: &str) -> DefContent {
        DefContent {
            kind,
            lines: [start, end],
            signature: sig.to_string(),
            scope: scope.to_string(),
        }
    }

    #[test]
    fn write_cache_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            100,
            CacheEvent::Miss {
                path_hash: 100,
                mtime: 1000,
                size: 200,
                defs: vec![make_def(DefKind::Function, 1, 10, "fn foo()", "foo")],
            },
        );

        write_cache(&path, None, &updates).unwrap();
        assert!(path.exists());

        // Verify we can load it
        let idx = CacheIndex::load(&path).unwrap();
        assert_eq!(idx.entry_count, 1);
        match idx.lookup(100, 1000, 200) {
            CacheOutcome::Hit(_) => {}
            other => panic!("expected Hit, got {:?}", other),
        }
    }

    #[test]
    fn write_cache_round_trip_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");
        let defs = vec![
            make_def(
                DefKind::Function,
                10,
                25,
                "fn process(data: &str)",
                "handler::process",
            ),
            make_def(DefKind::Class, 5, 15, "struct Config", "Config"),
        ];
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            42,
            CacheEvent::Miss {
                path_hash: 42,
                mtime: 5000,
                size: 300,
                defs: defs.clone(),
            },
        );

        write_cache(&path, None, &updates).unwrap();
        let idx = CacheIndex::load(&path).unwrap();
        let entry = match idx.lookup(42, 5000, 300) {
            CacheOutcome::Hit(e) => e,
            other => panic!("expected Hit, got {:?}", other),
        };
        let decoded = decode_data(idx.mapped_bytes(), &entry).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].kind, DefKind::Function);
        assert_eq!(decoded[0].signature, "fn process(data: &str)");
        assert_eq!(decoded[0].lines, [10, 25]);
        assert_eq!(decoded[0].scope, "handler::process");
        assert_eq!(decoded[1].kind, DefKind::Class);
        assert_eq!(decoded[1].scope, "Config");
    }

    #[test]
    fn write_cache_preserves_untouched_entries() {
        // Create initial cache with 2 entries
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        let defs_a = vec![make_def(DefKind::Function, 1, 10, "fn a()", "a")];
        let defs_b = vec![make_def(DefKind::Class, 20, 30, "class B", "B")];

        // Write initial cache
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            10,
            CacheEvent::Miss {
                path_hash: 10,
                mtime: 100,
                size: 50,
                defs: defs_a.clone(),
            },
        );
        updates.insert(
            20,
            CacheEvent::Miss {
                path_hash: 20,
                mtime: 200,
                size: 60,
                defs: defs_b.clone(),
            },
        );
        write_cache(&path, None, &updates).unwrap();

        // Load, then update only entry 10 (entry 20 is untouched)
        let old = CacheIndex::load(&path).unwrap();
        let new_defs = vec![make_def(DefKind::Enum, 1, 5, "enum E", "E")];
        let mut updates2 = std::collections::HashMap::new();
        updates2.insert(
            10,
            CacheEvent::Miss {
                path_hash: 10,
                mtime: 101,
                size: 55,
                defs: new_defs,
            },
        );
        write_cache(&path, Some(&old), &updates2).unwrap();

        // Verify
        let new_idx = CacheIndex::load(&path).unwrap();
        assert_eq!(new_idx.entry_count, 2);

        // Entry 10 should have new data
        match new_idx.lookup(10, 101, 55) {
            CacheOutcome::Hit(e) => {
                let d = decode_data(new_idx.mapped_bytes(), &e).unwrap();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].kind, DefKind::Enum);
            }
            other => panic!("expected Hit for 10, got {:?}", other),
        }

        // Entry 20 should be preserved with old data
        match new_idx.lookup(20, 200, 60) {
            CacheOutcome::Hit(e) => {
                let d = decode_data(new_idx.mapped_bytes(), &e).unwrap();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].kind, DefKind::Class);
                assert_eq!(d[0].scope, "B");
            }
            other => panic!("expected Hit for 20, got {:?}", other),
        }
    }

    #[test]
    fn write_cache_hit_copies_old_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        let defs = vec![make_def(DefKind::Function, 1, 10, "fn foo()", "foo")];
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            42,
            CacheEvent::Miss {
                path_hash: 42,
                mtime: 1000,
                size: 200,
                defs: defs.clone(),
            },
        );
        write_cache(&path, None, &updates).unwrap();

        // Load and simulate a Hit event
        let old = CacheIndex::load(&path).unwrap();
        let entry = match old.lookup(42, 1000, 200) {
            CacheOutcome::Hit(e) => e,
            other => panic!("expected Hit, got {:?}", other),
        };

        let mut updates2 = std::collections::HashMap::new();
        updates2.insert(42, CacheEvent::Hit(entry));
        write_cache(&path, Some(&old), &updates2).unwrap();

        // Verify data is preserved
        let new_idx = CacheIndex::load(&path).unwrap();
        match new_idx.lookup(42, 1000, 200) {
            CacheOutcome::Hit(e) => {
                let d = decode_data(new_idx.mapped_bytes(), &e).unwrap();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].scope, "foo");
            }
            other => panic!("expected Hit, got {:?}", other),
        }
    }

    #[test]
    fn write_cache_entries_sorted_by_path_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        // Insert in non-sorted order
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            300,
            CacheEvent::Miss {
                path_hash: 300,
                mtime: 1,
                size: 1,
                defs: vec![make_def(DefKind::Function, 1, 1, "fn c()", "c")],
            },
        );
        updates.insert(
            100,
            CacheEvent::Miss {
                path_hash: 100,
                mtime: 1,
                size: 1,
                defs: vec![make_def(DefKind::Function, 1, 1, "fn a()", "a")],
            },
        );
        updates.insert(
            200,
            CacheEvent::Miss {
                path_hash: 200,
                mtime: 1,
                size: 1,
                defs: vec![make_def(DefKind::Function, 1, 1, "fn b()", "b")],
            },
        );

        write_cache(&path, None, &updates).unwrap();
        let idx = CacheIndex::load(&path).unwrap();
        assert_eq!(idx.entry_count, 3);

        // Verify sorted order via read_path_hash
        assert_eq!(IndexEntry::read_path_hash(idx.mapped_bytes(), 0), Some(100));
        assert_eq!(IndexEntry::read_path_hash(idx.mapped_bytes(), 1), Some(200));
        assert_eq!(IndexEntry::read_path_hash(idx.mapped_bytes(), 2), Some(300));
    }

    #[test]
    fn write_cache_empty_updates_produces_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        let updates = std::collections::HashMap::new();
        write_cache(&path, None, &updates).unwrap();

        let idx = CacheIndex::load(&path).unwrap();
        assert_eq!(idx.entry_count, 0);
    }

    #[test]
    fn write_cache_no_old_all_new_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        let mut updates = std::collections::HashMap::new();
        updates.insert(
            10,
            CacheEvent::Miss {
                path_hash: 10,
                mtime: 100,
                size: 50,
                defs: vec![],
            },
        );
        updates.insert(
            20,
            CacheEvent::Miss {
                path_hash: 20,
                mtime: 200,
                size: 60,
                defs: vec![make_def(DefKind::Class, 1, 2, "struct S", "S")],
            },
        );

        write_cache(&path, None, &updates).unwrap();
        let idx = CacheIndex::load(&path).unwrap();
        assert_eq!(idx.entry_count, 2);
        assert!(matches!(idx.lookup(10, 100, 50), CacheOutcome::Hit(_)));
        assert!(matches!(idx.lookup(20, 200, 60), CacheOutcome::Hit(_)));
    }

    #[test]
    fn write_cache_atomic_no_corrupt_on_failure() {
        // Verify that write_cache creates the parent directory if needed
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("cache.bin");

        let mut updates = std::collections::HashMap::new();
        updates.insert(
            1,
            CacheEvent::Miss {
                path_hash: 1,
                mtime: 1,
                size: 1,
                defs: vec![],
            },
        );

        write_cache(&path, None, &updates).unwrap();
        assert!(path.exists());
        assert!(
            !path.with_extension("tmp").exists(),
            "temp file should be cleaned up"
        );
    }

    #[test]
    fn two_step_write_replaces_mmap_locked_cache() {
        // Two-step pattern: build buffer while mmap alive, drop mmap, then write.
        // This is the Windows-safe pattern — Windows locks mmap'd files,
        // preventing rename until the mapping is released.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");

        // Create initial cache with one entry
        let mut updates = std::collections::HashMap::new();
        updates.insert(
            42,
            CacheEvent::Miss {
                path_hash: 42,
                mtime: 1000,
                size: 200,
                defs: vec![make_def(DefKind::Function, 1, 10, "fn foo()", "foo")],
            },
        );
        write_cache(&path, None, &updates).unwrap();

        // Mmap the old cache (simulates Phase 1 in pipeline)
        let old = CacheIndex::load(&path).unwrap();
        assert_eq!(old.entry_count, 1);

        // Step 1: Build buffer while mmap is alive (reads old data)
        let new_defs = vec![make_def(DefKind::Enum, 1, 5, "enum E", "E")];
        let mut updates2 = std::collections::HashMap::new();
        updates2.insert(
            42,
            CacheEvent::Miss {
                path_hash: 42,
                mtime: 1001,
                size: 201,
                defs: new_defs,
            },
        );
        let buf = build_cache_buffer(Some(&old), &updates2).unwrap();

        // Release mmap before atomic write
        drop(old);

        // Step 2: Write atomically (mmap released, file is unlocked on Windows)
        write_cache_atomic(&path, &buf).unwrap();

        // Verify the new cache replaced the old one
        let new_idx = CacheIndex::load(&path).unwrap();
        assert_eq!(new_idx.entry_count, 1);
        match new_idx.lookup(42, 1001, 201) {
            CacheOutcome::Hit(e) => {
                let d = decode_data(new_idx.mapped_bytes(), &e).unwrap();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].kind, DefKind::Enum);
                assert_eq!(d[0].scope, "E");
            }
            other => panic!("expected Hit, got {:?}", other),
        }
    }

    // =======================================================================
    // find_project_root
    // =======================================================================

    #[test]
    fn find_project_root_finds_git_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let root = find_project_root(tmp.path());
        assert!(root.is_some());
    }

    #[test]
    fn find_project_root_none_without_git() {
        let tmp = tempfile::tempdir().unwrap();
        let root = find_project_root(tmp.path());
        assert!(root.is_none());
    }
}
