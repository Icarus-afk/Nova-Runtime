use axum::http::{HeaderValue, StatusCode, header, HeaderMap};
use axum::response::Json;
use serde_json::Value;

/// The conventional CREATE response: 201 Created with a `Location` header
/// pointing at the newly created resource and a JSON body.
pub type Created = (StatusCode, HeaderMap, Json<Value>);

/// Build a 201 Created response with a `Location` header.
pub fn created(location: &str, body: Value) -> Created {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(location) {
        headers.insert(header::LOCATION, v);
    }
    (StatusCode::CREATED, headers, Json(body))
}

/// Append one `<link>; rel="rel"` entry to an existing Link header value,
/// preserving any prior entries.
pub fn push_link(existing: &Option<String>, url: &str, rel: &str) -> String {
    let entry = format!("<{url}>; rel=\"{rel}\"");
    match existing {
        Some(prev) if !prev.is_empty() => format!("{prev}, {entry}"),
        _ => entry,
    }
}

/// Build RFC8288 `Link` header entries for an offset/limit paginated list:
/// `first`, `prev` (when offset > 0), `next` (when more rows remain) and
/// `last`. `params` are the pagination query parameters (e.g. limit/offset)
/// to reproduce; the returned value is suitable for the `Link` header.
pub fn pagination_links(
    path: &str,
    params: &[(&str, String)],
    limit: usize,
    offset: usize,
    total: usize,
) -> String {
    let query = |o: usize| {
        let mut q: Vec<String> = params
            .iter()
            .filter(|(k, _)| *k != "offset")
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        q.push(format!("limit={limit}"));
        q.push(format!("offset={o}"));
        q.join("&")
    };
    let mut link: Option<String> = None;
    link = Some(push_link(&link, &format!("{path}?{}", query(0)), "first"));
    if offset > 0 {
        let prev = offset.saturating_sub(limit);
        link = Some(push_link(&link, &format!("{path}?{}", query(prev)), "prev"));
    }
    if offset + limit < total {
        let next = offset + limit;
        link = Some(push_link(&link, &format!("{path}?{}", query(next)), "next"));
    }
    if total > 0 {
        let last_page = (total.saturating_sub(1) / limit) * limit;
        link = Some(push_link(&link, &format!("{path}?{}", query(last_page)), "last"));
    }
    link.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_created_has_201_and_location() {
        let resp = created("/api/v1/queues/myq", serde_json::json!({"id": "myq"}));
        assert_eq!(resp.0, StatusCode::CREATED);
        assert_eq!(resp.1.get("location").unwrap(), "/api/v1/queues/myq");
    }

    #[test]
    fn test_push_link_builds_first_entry() {
        assert_eq!(
            push_link(&None, "/api/v1/queues?cursor=abc", "next"),
            "</api/v1/queues?cursor=abc>; rel=\"next\""
        );
    }

    #[test]
    fn test_push_link_appends_existing() {
        let first = push_link(&None, "/first", "first");
        let combined = push_link(&Some(first), "/next", "next");
        assert!(combined.contains("rel=\"first\""));
        assert!(combined.contains("rel=\"next\""));
        assert!(combined.contains(","));
    }

    #[test]
    fn test_pagination_links_first_and_last() {
        let links = pagination_links("/api/v1/blobs", &[], 50, 0, 200);
        assert!(links.contains("rel=\"first\""));
        assert!(links.contains("rel=\"next\""));
        assert!(links.contains("rel=\"last\""));
        assert!(!links.contains("rel=\"prev\""));
        assert!(links.contains("offset=0"));
        assert!(links.contains("offset=50"));
    }

    #[test]
    fn test_pagination_links_middle_has_prev() {
        let links = pagination_links("/api/v1/blobs", &[], 10, 20, 35);
        assert!(links.contains("rel=\"prev\""));
        assert!(links.contains("offset=10"));
        assert!(links.contains("rel=\"next\""));
        assert!(links.contains("offset=30"));
    }

    #[test]
    fn test_pagination_links_no_next_when_exhausted() {
        let links = pagination_links("/api/v1/blobs", &[], 10, 30, 35);
        assert!(!links.contains("rel=\"next\""));
        assert!(links.contains("rel=\"last\""));
    }
}
