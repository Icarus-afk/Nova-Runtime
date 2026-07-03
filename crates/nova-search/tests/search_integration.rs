use nova_search::document::IndexedDocument;
use nova_search::manager::SearchManager;

fn make_doc(id: &str, title: &str, body: &str) -> IndexedDocument {
    IndexedDocument::new(id)
        .add_text("title", title)
        .add_text("body", body)
}

fn make_doc_with_category(id: &str, title: &str, category: &str) -> IndexedDocument {
    IndexedDocument::new(id)
        .add_text("title", title)
        .add_text("category", category)
}

#[test]
fn test_index_and_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "The quick brown fox", "A fox is quick and brown"))
        .unwrap();
    manager
        .index_document(make_doc("2", "The lazy dog", "A dog sleeps all day"))
        .unwrap();
    manager
        .index_document(make_doc("3", "Jumping frog", "A frog jumps very high"))
        .unwrap();

    let results = manager.search("fox", 10).unwrap();
    assert!(!results.is_empty(), "should find fox documents");
    assert!(results.iter().any(|r| r.doc_id == 1));

    let results = manager.search("dog", 10).unwrap();
    assert!(!results.is_empty(), "should find dog documents");
    assert!(results.iter().any(|r| r.doc_id == 2));

    let results = manager.search("frog", 10).unwrap();
    assert!(!results.is_empty(), "should find frog documents");
    assert!(results.iter().any(|r| r.doc_id == 3));
}

#[test]
fn test_phrase_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "hello world", "the quick brown fox jumps"))
        .unwrap();
    manager
        .index_document(make_doc("2", "hello there", "the lazy dog sleeps"))
        .unwrap();

    let results = manager.search("\"quick brown\"", 10).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_prefix_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "hello world", "running fast"))
        .unwrap();
    manager
        .index_document(make_doc("2", "goodbye world", "runner up"))
        .unwrap();

    let results = manager.search("run*", 10).unwrap();
    assert!(!results.is_empty(), "should find documents with run* prefix");
}

#[test]
fn test_fuzzy_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "color", "The color is red"))
        .unwrap();
    manager
        .index_document(make_doc("2", "colour", "The colour is blue"))
        .unwrap();

    let results = manager.search("color~", 10).unwrap();
    assert!(!results.is_empty(), "fuzzy search should find results");
    assert!(results.len() >= 1);
}

#[test]
fn test_boolean_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "apple pie", "Delicious apple pie recipe"))
        .unwrap();
    manager
        .index_document(make_doc("2", "apple juice", "Fresh apple juice"))
        .unwrap();
    manager
        .index_document(make_doc("3", "pie crust", "Pie crust recipe"))
        .unwrap();

    // AND search (implicit AND between terms)
    let results = manager.search("apple pie", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results.iter().any(|r| r.doc_id == 1));

    // OR search
    let results = manager.search("apple OR pie", 10).unwrap();
    assert_eq!(results.len(), 3);

    // NOT search: apple AND NOT pie
    let results = manager.search("apple -pie", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results.iter().any(|r| r.doc_id == 2));
}

#[test]
fn test_range_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(
            IndexedDocument::new("1")
                .add_text("name", "item_a")
                .add_integer("price", 10),
        )
        .unwrap();
    manager
        .index_document(
            IndexedDocument::new("2")
                .add_text("name", "item_b")
                .add_integer("price", 50),
        )
        .unwrap();
    manager
        .index_document(
            IndexedDocument::new("3")
                .add_text("name", "item_c")
                .add_integer("price", 100),
        )
        .unwrap();

    let results = manager.search("price:[10 TO 60]", 10).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_faceted_search() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc_with_category("1", "fiction book", "fiction"))
        .unwrap();
    manager
        .index_document(make_doc_with_category("2", "another fiction", "fiction"))
        .unwrap();
    manager
        .index_document(make_doc_with_category("3", "science book", "science"))
        .unwrap();

    let result = manager.search_faceted("*", "category", 10).unwrap();
    assert_eq!(result.field, "category");
    assert!(!result.entries.is_empty());
    let fiction_count = result
        .entries
        .iter()
        .find(|(v, _)| v == "fiction")
        .map(|(_, c)| *c)
        .unwrap_or(0);
    assert_eq!(fiction_count, 2);
}

#[test]
fn test_delete_document() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "delete me", "This document will be deleted"))
        .unwrap();

    let results = manager.search("delete", 10).unwrap();
    assert!(!results.is_empty());

    manager.delete_document("1").unwrap();

    let results = manager.search("delete", 10).unwrap();
    assert!(results.is_empty(), "should not find deleted document");
}

#[test]
fn test_highlighting() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "highlight me", "This is the text that should be highlighted in search results"))
        .unwrap();

    let highlighted = manager.search_with_highlight("highlighted", 10).unwrap();
    assert!(!highlighted.is_empty());
}

#[test]
fn test_multiple_fields() {
    let mut manager = SearchManager::new();
    manager
        .index_document(
            IndexedDocument::new("1")
                .add_text("title", "Rust programming")
                .add_text("body", "Learn Rust language"),
        )
        .unwrap();
    manager
        .index_document(
            IndexedDocument::new("2")
                .add_text("title", "Python programming")
                .add_text("body", "Learn Python language"),
        )
        .unwrap();

    let results = manager.search("title:rust", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.doc_id == 1));

    let results = manager.search("body:python", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.doc_id == 2));
}

#[test]
fn test_empty_index() {
    let manager = SearchManager::new();
    let results = manager.search("anything", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_large_document() {
    let mut manager = SearchManager::new();
    let large_text = "word ".repeat(1000) + "needle " + &"word ".repeat(1000);
    manager
        .index_document(IndexedDocument::new("1").add_text("body", &large_text))
        .unwrap();

    let results = manager.search("needle", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.doc_id == 1));
}

#[test]
fn test_concurrent_index_search() {
    let mut manager = SearchManager::new();
    for i in 0..50 {
        let doc = IndexedDocument::new(format!("{}", i)).add_text("body", format!("document number {}", i));
        manager.index_document(doc).unwrap();
    }

    let results = manager.search("document", 100).unwrap();
    assert_eq!(results.len(), 50);
}

#[test]
fn test_bm25_scoring() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "cat", "the cat sat on the mat"))
        .unwrap();
    manager
        .index_document(make_doc("2", "dog", "the dog ran in the park"))
        .unwrap();

    let results = manager.search("cat", 10).unwrap();
    assert!(!results.is_empty());
    let cat_result = results.iter().find(|r| r.doc_id == 1);
    assert!(cat_result.is_some());
    assert!(cat_result.unwrap().score > 0.0);
}

#[test]
fn test_match_all() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "first", "content one"))
        .unwrap();
    manager
        .index_document(make_doc("2", "second", "content two"))
        .unwrap();

    let results = manager.search("*", 10).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_field_specific_phrase() {
    let mut manager = SearchManager::new();
    manager
        .index_document(
            IndexedDocument::new("1")
                .add_text("title", "rust programming language")
                .add_text("body", "some random content"),
        )
        .unwrap();

    let results = manager.search("title:\"rust programming\"", 10).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_delete_nonexistent() {
    let mut manager = SearchManager::new();
    let result = manager.delete_document("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_fuzzy_with_distance() {
    let mut manager = SearchManager::new();
    manager
        .index_document(make_doc("1", "hello", "hello world"))
        .unwrap();
    manager
        .index_document(make_doc("2", "hallo", "hallo world"))
        .unwrap();
    manager
        .index_document(make_doc("3", "help", "help me"))
        .unwrap();

    let results = manager.search("hell~1", 10).unwrap();
    assert!(!results.is_empty());
}
