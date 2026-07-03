#[derive(Debug, Clone)]
pub struct PostingEntry {
    pub doc_id: u64,
    pub term_frequency: u32,
    pub positions: Vec<u32>,
}

pub type PostingList = Vec<PostingEntry>;
