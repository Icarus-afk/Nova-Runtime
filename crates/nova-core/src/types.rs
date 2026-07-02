use std::sync::atomic::{AtomicU64, Ordering};
use std::fmt;
use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: usize = 4096;
pub const WAL_SEGMENT_SIZE: u64 = 64 * 1024 * 1024; // 64 MB
pub const DEFAULT_BLOCK_CACHE_SIZE: u64 = 256 * 1024 * 1024; // 256 MB
pub const DEFAULT_PAGE_CACHE_SIZE: u64 = 64 * 1024 * 1024; // 64 MB
pub const DEFAULT_MEMTABLE_SIZE: u64 = 64 * 1024 * 1024; // 64 MB
pub const MAX_DOCUMENT_SIZE: usize = 16 * 1024 * 1024; // 16 MB
pub const MAGIC: u32 = 0x4E4F5641; // "NOVA"

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PageId(pub u64);

impl PageId {
    pub const INVALID: PageId = PageId(u64::MAX);

    pub fn new(id: u64) -> Self {
        PageId(id)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn is_valid(self) -> bool {
        self.0 != u64::MAX
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for PageId {
    fn from(id: u64) -> Self {
        PageId(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Lsn(pub u64);

impl Lsn {
    pub const ZERO: Lsn = Lsn(0);
    pub const MAX: Lsn = Lsn(u64::MAX);

    pub fn new(seq: u64) -> Self {
        Lsn(seq)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Lsn {
        Lsn(self.0 + 1)
    }
}

impl Default for Lsn {
    fn default() -> Self {
        Lsn::ZERO
    }
}

impl fmt::Display for Lsn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub u64);

impl TransactionId {
    pub const ZERO: TransactionId = TransactionId(0);

    pub fn new(id: u64) -> Self {
        TransactionId(id)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

static NEXT_TX_ID: AtomicU64 = AtomicU64::new(1);

pub fn allocate_transaction_id() -> TransactionId {
    TransactionId(NEXT_TX_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key(pub Vec<u8>);

impl Key {
    pub fn new(data: Vec<u8>) -> Self {
        Key(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn from_str_key(s: &str) -> Self {
        Key(s.as_bytes().to_vec())
    }
}

impl From<Vec<u8>> for Key {
    fn from(v: Vec<u8>) -> Self {
        Key(v)
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Key(s.as_bytes().to_vec())
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(s) = std::str::from_utf8(&self.0) {
            write!(f, "{}", s)
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Value(pub Vec<u8>);

impl Value {
    pub fn new(data: Vec<u8>) -> Self {
        Value(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value(v)
    }
}

impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression {
    None,
    Snappy,
    Zstd,
}

impl Compression {
    pub fn to_byte(self) -> u8 {
        match self {
            Compression::None => 0,
            Compression::Snappy => 1,
            Compression::Zstd => 2,
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Compression::None),
            1 => Some(Compression::Snappy),
            2 => Some(Compression::Zstd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsyncPolicy {
    EveryWrite,
    EveryNMs(u64),
    Async,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Checksum(pub u32);

impl Checksum {
    pub fn new(value: u32) -> Self {
        Checksum(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Page {
    pub id: PageId,
    pub lsn: Lsn,
    pub data: [u8; PAGE_SIZE],
    pub checksum: Checksum,
    pub flags: u16,
}

impl Page {
    pub fn new(id: PageId) -> Self {
        Page {
            id,
            lsn: Lsn::ZERO,
            data: [0u8; PAGE_SIZE],
            checksum: Checksum(0),
            flags: 0,
        }
    }

    pub fn zeroed() -> Self {
        Page {
            id: PageId::INVALID,
            lsn: Lsn::ZERO,
            data: [0u8; PAGE_SIZE],
            checksum: Checksum(0),
            flags: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.flags & 0x1 != 0
    }

    pub fn mark_dirty(&mut self) {
        self.flags |= 0x1;
    }

    pub fn clear_dirty(&mut self) {
        self.flags &= !0x1;
    }

    pub fn read_u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes(self.data[offset..offset + 2].try_into().unwrap())
    }

    pub fn write_u16(&mut self, offset: usize, value: u16) {
        self.data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub fn read_u32(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.data[offset..offset + 4].try_into().unwrap())
    }

    pub fn write_u32(&mut self, offset: usize, value: u32) {
        self.data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn read_u64(&self, offset: usize) -> u64 {
        u64::from_le_bytes(self.data[offset..offset + 8].try_into().unwrap())
    }

    pub fn write_u64(&mut self, offset: usize, value: u64) {
        self.data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}

/// Isolation level for transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    ReadCommitted,
    RepeatableRead,
    Snapshot,
    Serializable,
}

/// Engine type for hybrid routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineType {
    BTree,
    LSM,
}

/// Trace context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub parent_span_id: Option<[u8; 8]>,
    pub sampled: bool,
}

impl TraceContext {
    pub fn new() -> Self {
        TraceContext {
            trace_id: [0u8; 16],
            span_id: [0u8; 8],
            parent_span_id: None,
            sampled: false,
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        TraceContext::new()
    }
}

/// Compression codec
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionCodec {
    None,
    #[default]
    Snappy,
    Zstd { level: i32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- PageId ---

    #[test]
    fn test_pageid_new() {
        let p = PageId::new(42);
        assert_eq!(p.value(), 42);
    }

    #[test]
    fn test_pageid_invalid() {
        assert!(!PageId::INVALID.is_valid());
        assert_eq!(PageId::INVALID.value(), u64::MAX);
    }

    #[test]
    fn test_pageid_valid() {
        let p = PageId::new(0);
        assert!(p.is_valid());
        let p = PageId::new(1);
        assert!(p.is_valid());
    }

    #[test]
    fn test_pageid_from_u64() {
        let p: PageId = 99u64.into();
        assert_eq!(p, PageId::new(99));
    }

    #[test]
    fn test_pageid_equality() {
        assert_eq!(PageId::new(1), PageId::new(1));
        assert_ne!(PageId::new(1), PageId::new(2));
    }

    #[test]
    fn test_pageid_ordering() {
        assert!(PageId::new(1) < PageId::new(2));
        assert!(PageId::new(5) > PageId::new(3));
        assert!(PageId::new(7) >= PageId::new(7));
    }

    #[test]
    fn test_pageid_clone() {
        let p = PageId::new(10);
        let c = p.clone();
        assert_eq!(p, c);
    }

    #[test]
    fn test_pageid_display() {
        assert_eq!(format!("{}", PageId::new(123)), "123");
    }

    #[test]
    fn test_pageid_debug() {
        assert_eq!(format!("{:?}", PageId::new(7)), "PageId(7)");
    }

    // --- Lsn ---

    #[test]
    fn test_lsn_new() {
        let l = Lsn::new(100);
        assert_eq!(l.value(), 100);
    }

    #[test]
    fn test_lsn_zero() {
        assert_eq!(Lsn::ZERO.value(), 0);
    }

    #[test]
    fn test_lsn_max() {
        assert_eq!(Lsn::MAX.value(), u64::MAX);
    }

    #[test]
    fn test_lsn_next() {
        assert_eq!(Lsn::new(0).next(), Lsn::new(1));
        assert_eq!(Lsn::new(5).next(), Lsn::new(6));
    }

    #[test]
    fn test_lsn_default() {
        assert_eq!(Lsn::default(), Lsn::ZERO);
    }

    #[test]
    fn test_lsn_ordering() {
        assert!(Lsn::new(1) < Lsn::new(2));
        assert!(Lsn::new(10) > Lsn::new(5));
        assert!(Lsn::new(3) >= Lsn::new(3));
    }

    #[test]
    fn test_lsn_equality() {
        assert_eq!(Lsn::new(42), Lsn::new(42));
        assert_ne!(Lsn::new(1), Lsn::new(2));
    }

    #[test]
    fn test_lsn_display() {
        assert_eq!(format!("{}", Lsn::new(999)), "999");
    }

    #[test]
    fn test_lsn_debug() {
        assert_eq!(format!("{:?}", Lsn::new(7)), "Lsn(7)");
    }

    // --- TransactionId ---

    #[test]
    fn test_transactionid_new() {
        let t = TransactionId::new(10);
        assert_eq!(t.value(), 10);
    }

    #[test]
    fn test_transactionid_zero() {
        assert_eq!(TransactionId::ZERO.value(), 0);
    }

    #[test]
    fn test_transactionid_equality() {
        assert_eq!(TransactionId::new(5), TransactionId::new(5));
        assert_ne!(TransactionId::new(5), TransactionId::new(6));
    }

    #[test]
    fn test_transactionid_display() {
        assert_eq!(format!("{}", TransactionId::new(7)), "7");
    }

    #[test]
    fn test_allocate_transaction_id() {
        let id1 = allocate_transaction_id();
        let id2 = allocate_transaction_id();
        assert_ne!(id1, id2);
        assert!(id1.value() >= 1);
        assert!(id2.value() > id1.value());
    }

    // --- Key ---

    #[test]
    fn test_key_new() {
        let k = Key::new(vec![1, 2, 3]);
        assert_eq!(k.as_bytes(), &[1, 2, 3]);
    }

    #[test]
    fn test_key_from_str_key() {
        let k = Key::from_str_key("hello");
        assert_eq!(k.as_bytes(), b"hello");
    }

    #[test]
    fn test_key_len() {
        assert_eq!(Key::new(vec![1, 2, 3]).len(), 3);
        assert_eq!(Key::new(vec![]).len(), 0);
    }

    #[test]
    fn test_key_is_empty() {
        assert!(Key::new(vec![]).is_empty());
        assert!(!Key::new(vec![1]).is_empty());
    }

    #[test]
    fn test_key_from_vec() {
        let k: Key = vec![10u8, 20].into();
        assert_eq!(k.as_bytes(), &[10, 20]);
    }

    #[test]
    fn test_key_from_str() {
        let k: Key = "world".into();
        assert_eq!(k.as_bytes(), b"world");
    }

    #[test]
    fn test_key_as_ref() {
        let k = Key::new(vec![255]);
        assert_eq!(k.as_ref(), &[255u8]);
    }

    #[test]
    fn test_key_display_utf8() {
        let k = Key::from("hello");
        assert_eq!(format!("{}", k), "hello");
    }

    #[test]
    fn test_key_display_non_utf8() {
        let k = Key::new(vec![0, 159, 146, 150]);
        let s = format!("{}", k);
        assert!(s.contains("["));
        assert!(s.contains("]"));
    }

    #[test]
    fn test_key_clone() {
        let k = Key::new(vec![1, 2, 3]);
        let c = k.clone();
        assert_eq!(k, c);
    }

    #[test]
    fn test_key_equality() {
        assert_eq!(Key::new(vec![1]), Key::new(vec![1]));
        assert_ne!(Key::new(vec![1]), Key::new(vec![2]));
    }

    // --- Value ---

    #[test]
    fn test_value_new() {
        let v = Value::new(vec![4, 5, 6]);
        assert_eq!(v.as_bytes(), &[4, 5, 6]);
    }

    #[test]
    fn test_value_len() {
        assert_eq!(Value::new(vec![1, 2]).len(), 2);
        assert_eq!(Value::new(vec![]).len(), 0);
    }

    #[test]
    fn test_value_is_empty() {
        assert!(Value::new(vec![]).is_empty());
        assert!(!Value::new(vec![0]).is_empty());
    }

    #[test]
    fn test_value_from_vec() {
        let v: Value = vec![7u8, 8, 9].into();
        assert_eq!(v.as_bytes(), &[7, 8, 9]);
    }

    #[test]
    fn test_value_as_ref() {
        let v = Value::new(vec![128]);
        assert_eq!(v.as_ref(), &[128u8]);
    }

    #[test]
    fn test_value_clone() {
        let v = Value::new(vec![1, 2, 3]);
        let c = v.clone();
        assert_eq!(v, c);
    }

    #[test]
    fn test_value_equality() {
        assert_eq!(Value::new(vec![1, 2]), Value::new(vec![1, 2]));
        assert_ne!(Value::new(vec![1]), Value::new(vec![2]));
    }

    // --- Compression ---

    #[test]
    fn test_compression_to_byte() {
        assert_eq!(Compression::None.to_byte(), 0);
        assert_eq!(Compression::Snappy.to_byte(), 1);
        assert_eq!(Compression::Zstd.to_byte(), 2);
    }

    #[test]
    fn test_compression_from_byte() {
        assert_eq!(Compression::from_byte(0), Some(Compression::None));
        assert_eq!(Compression::from_byte(1), Some(Compression::Snappy));
        assert_eq!(Compression::from_byte(2), Some(Compression::Zstd));
        assert_eq!(Compression::from_byte(3), None);
        assert_eq!(Compression::from_byte(255), None);
    }

    #[test]
    fn test_compression_roundtrip() {
        for b in 0..=2u8 {
            let c = Compression::from_byte(b).unwrap();
            assert_eq!(c.to_byte(), b);
        }
    }

    #[test]
    fn test_compression_equality() {
        assert_eq!(Compression::None, Compression::None);
        assert_ne!(Compression::None, Compression::Snappy);
    }

    // --- AccessMode ---

    #[test]
    fn test_access_mode_variants() {
        assert_eq!(format!("{:?}", AccessMode::Read), "Read");
        assert_eq!(format!("{:?}", AccessMode::Write), "Write");
        assert_eq!(format!("{:?}", AccessMode::ReadWrite), "ReadWrite");
    }

    // --- FsyncPolicy ---

    #[test]
    fn test_fsync_policy_variants() {
        match FsyncPolicy::EveryWrite {
            FsyncPolicy::EveryWrite => {}
            _ => panic!("expected EveryWrite"),
        }
        match FsyncPolicy::EveryNMs(100) {
            FsyncPolicy::EveryNMs(n) => assert_eq!(n, 100),
            _ => panic!("expected EveryNMs"),
        }
        match FsyncPolicy::Async {
            FsyncPolicy::Async => {}
            _ => panic!("expected Async"),
        }
    }

    // --- Checksum ---

    #[test]
    fn test_checksum_new() {
        let c = Checksum::new(0xDEADBEEF);
        assert_eq!(c.value(), 0xDEADBEEF);
    }

    #[test]
    fn test_checksum_equality() {
        assert_eq!(Checksum::new(1), Checksum::new(1));
        assert_ne!(Checksum::new(1), Checksum::new(2));
    }

    // --- Page ---

    #[test]
    fn test_page_new() {
        let id = PageId::new(42);
        let page = Page::new(id);
        assert_eq!(page.id, id);
        assert_eq!(page.lsn, Lsn::ZERO);
        assert_eq!(page.checksum, Checksum::new(0));
        assert_eq!(page.flags, 0);
        assert!(!page.is_dirty());
    }

    #[test]
    fn test_page_zeroed() {
        let page = Page::zeroed();
        assert_eq!(page.id, PageId::INVALID);
        assert_eq!(page.lsn, Lsn::ZERO);
        assert_eq!(page.checksum, Checksum::new(0));
        assert!(!page.is_dirty());
    }

    #[test]
    fn test_page_dirty_tracking() {
        let mut page = Page::new(PageId::new(1));
        assert!(!page.is_dirty());
        page.mark_dirty();
        assert!(page.is_dirty());
        page.clear_dirty();
        assert!(!page.is_dirty());
    }

    #[test]
    fn test_page_write_read_u16() {
        let mut page = Page::new(PageId::new(1));
        page.write_u16(0, 0xABCD);
        assert_eq!(page.read_u16(0), 0xABCD);
    }

    #[test]
    fn test_page_write_read_u16_non_zero_offset() {
        let mut page = Page::new(PageId::new(1));
        page.write_u16(100, 42);
        assert_eq!(page.read_u16(100), 42);
    }

    #[test]
    fn test_page_write_read_u32() {
        let mut page = Page::new(PageId::new(1));
        page.write_u32(0, 0xDEADBEEF);
        assert_eq!(page.read_u32(0), 0xDEADBEEF);
    }

    #[test]
    fn test_page_write_read_u32_offset() {
        let mut page = Page::new(PageId::new(1));
        page.write_u32(256, 12345);
        assert_eq!(page.read_u32(256), 12345);
    }

    #[test]
    fn test_page_write_read_u64() {
        let mut page = Page::new(PageId::new(1));
        page.write_u64(0, u64::MAX);
        assert_eq!(page.read_u64(0), u64::MAX);
    }

    #[test]
    fn test_page_write_read_u64_offset() {
        let mut page = Page::new(PageId::new(1));
        page.write_u64(512, 0xAABBCCDDEE112233);
        assert_eq!(page.read_u64(512), 0xAABBCCDDEE112233);
    }

    #[test]
    fn test_page_does_not_overlap_writes() {
        let mut page = Page::new(PageId::new(1));
        page.write_u16(0, 0xAAAA);
        page.write_u16(2, 0xBBBB);
        assert_eq!(page.read_u16(0), 0xAAAA);
        assert_eq!(page.read_u16(2), 0xBBBB);
    }

    #[test]
    fn test_page_clone() {
        let mut page = Page::new(PageId::new(5));
        page.mark_dirty();
        let cloned = page.clone();
        assert_eq!(page.id, cloned.id);
        assert_eq!(page.is_dirty(), cloned.is_dirty());
    }

    // --- IsolationLevel ---

    #[test]
    fn test_isolation_level_variants() {
        assert_eq!(format!("{:?}", IsolationLevel::ReadCommitted), "ReadCommitted");
        assert_eq!(format!("{:?}", IsolationLevel::RepeatableRead), "RepeatableRead");
        assert_eq!(format!("{:?}", IsolationLevel::Snapshot), "Snapshot");
        assert_eq!(format!("{:?}", IsolationLevel::Serializable), "Serializable");
    }

    #[test]
    fn test_isolation_level_equality() {
        assert_eq!(IsolationLevel::Snapshot, IsolationLevel::Snapshot);
        assert_ne!(IsolationLevel::ReadCommitted, IsolationLevel::Serializable);
    }

    // --- EngineType ---

    #[test]
    fn test_engine_type_variants() {
        assert_eq!(format!("{:?}", EngineType::BTree), "BTree");
        assert_eq!(format!("{:?}", EngineType::LSM), "LSM");
    }

    // --- TraceContext ---

    #[test]
    fn test_trace_context_new() {
        let ctx = TraceContext::new();
        assert_eq!(ctx.trace_id, [0u8; 16]);
        assert_eq!(ctx.span_id, [0u8; 8]);
        assert_eq!(ctx.parent_span_id, None);
        assert!(!ctx.sampled);
    }

    #[test]
    fn test_trace_context_default() {
        let ctx = TraceContext::default();
        assert_eq!(ctx.trace_id, [0u8; 16]);
        assert_eq!(ctx.span_id, [0u8; 8]);
        assert_eq!(ctx.parent_span_id, None);
        assert!(!ctx.sampled);
    }

    // --- CompressionCodec ---

    #[test]
    fn test_compression_codec_default() {
        assert_eq!(CompressionCodec::default(), CompressionCodec::Snappy);
    }

    #[test]
    fn test_compression_codec_variants() {
        assert_eq!(format!("{:?}", CompressionCodec::None), "None");
        assert_eq!(format!("{:?}", CompressionCodec::Snappy), "Snappy");
        let zstd = CompressionCodec::Zstd { level: 3 };
        assert_eq!(format!("{:?}", zstd), "Zstd { level: 3 }");
    }
}
