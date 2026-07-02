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

    pub fn from_str(s: &str) -> Self {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionCodec {
    None,
    Snappy,
    Zstd { level: i32 },
}

impl Default for CompressionCodec {
    fn default() -> Self { CompressionCodec::Snappy }
}
