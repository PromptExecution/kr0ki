//! Element-collection paging: turning one `elements(..)` response into "here is the
//! next cursor, or we're done".
//!
//! The OMG PSM pages `GET .../elements` with `page-after` / `page-before` / `page-size`
//! query params and *should* return an RFC 8288 `Link` header carrying `rel="next"`
//! (this is what the OMG Java pilot and Flexo emit). Eclipse SysON's partial REST API
//! does not send `Link` headers, so a fallback is needed.

use crate::model::Element;

/// Which query-parameter convention a target server uses for element paging.
///
/// Flexo and the OMG Java pilot use hyphenated params (`page-after=`); some
/// JSON:API-flavoured OMG-pilot deployments use bracket form (`page[after]=`)
/// instead. Configured once per [`crate::SysmlV2Client`] via
/// [`crate::SysmlV2Client::with_page_param_style`] — pick the one the target server's
/// own docs/`EVAL-*.md` say it expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageParamStyle {
    #[default]
    Hyphenated,
    JsonApiBracket,
}

impl PageParamStyle {
    pub const fn after_key(self) -> &'static str {
        match self {
            Self::Hyphenated => "page-after",
            Self::JsonApiBracket => "page[after]",
        }
    }

    pub const fn before_key(self) -> &'static str {
        match self {
            Self::Hyphenated => "page-before",
            Self::JsonApiBracket => "page[before]",
        }
    }

    pub const fn size_key(self) -> &'static str {
        match self {
            Self::Hyphenated => "page-size",
            Self::JsonApiBracket => "page[size]",
        }
    }
}

/// Derive the cursor for the next page of elements.
///
/// Priority:
/// 1. **`Link: rel="next"` header present** — parse that URL and return its
///    `page-after` query-parameter value. Authoritative when present.
/// 2. **No `Link` header, but the caller requested an explicit `page-size` and got a
///    full page** (`items.len() >= size`) — assume more remain and use the last
///    element's `@id` as the next cursor. OMG PSM page cursors are element ids, so this
///    is the documented fallback for SysON-style servers.
/// 3. **Otherwise** (short page, or no `Link` header and no size hint) — the collection
///    is exhausted → `None`.
///
/// Consequence for `all_elements`: against a spec-compliant server it follows `Link`
/// headers; against a `Link`-less server it only pages when the caller passed a
/// `page-size` (which `all_elements` itself does not — it takes whatever the server's
/// default page returns). This is deliberate: guessing a cursor without either a
/// `Link` header or an explicit size risks an infinite loop on a server that ignores
/// `page-after`.
pub fn derive_next_after(
    link_header: Option<&str>,
    items: &[Element],
    requested_size: Option<u32>,
    style: PageParamStyle,
) -> Option<String> {
    if let Some(header) = link_header {
        if let Some(next_url) = parse_link_next(header) {
            if let Some(cursor) = extract_query_param(&next_url, style.after_key()) {
                if !cursor.is_empty() {
                    return Some(cursor);
                }
            }
        }
    }

    match requested_size {
        Some(size) if size > 0 && items.len() >= size as usize => {
            items.last().map(|e| e.at_id.clone())
        }
        _ => None,
    }
}

/// Extract the URL of the `rel="next"` entry from an RFC 8288 `Link` header value.
///
/// Handles `<url>; rel="next"`, `rel=next`, `rel='next'`, and multiple comma-separated
/// entries. A URL containing a literal (non-encoded) comma would break the split —
/// acceptable, since the servers in scope percent-encode cursors.
pub fn parse_link_next(header: &str) -> Option<String> {
    for entry in header.split(',') {
        let entry = entry.trim();
        let mut segs = entry.split(';');
        let url_seg = segs.next()?.trim();
        if url_seg.len() < 2 || !url_seg.starts_with('<') || !url_seg.ends_with('>') {
            continue;
        }
        let url = &url_seg[1..url_seg.len() - 1];
        let is_next = segs.any(|s| {
            let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            matches!(s.as_str(), "rel=\"next\"" | "rel=next" | "rel='next'")
        });
        if is_next {
            return Some(url.to_string());
        }
    }
    None
}

/// Return the (percent-decoded) value of query parameter `key` in `url`.
pub fn extract_query_param(url: &str, key: &str) -> Option<String> {
    let after_q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let query = after_q.split('#').next().unwrap_or("");
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(percent_decode(v));
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push((hi << 4) | lo);
                    i += 3;
                }
                _ => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn els(ids: &[&str]) -> Vec<Element> {
        ids.iter()
            .map(|id| serde_json::from_value(json!({ "@id": id, "@type": "PartUsage" })).unwrap())
            .collect()
    }

    #[test]
    fn link_header_next_wins() {
        let h = r#"<https://s/x?page-after=abc123&page-size=50>; rel="next", <https://s/x?page-before=z>; rel="prev""#;
        let next = derive_next_after(
            Some(h),
            &els(&["e1", "e2"]),
            None,
            PageParamStyle::Hyphenated,
        );
        assert_eq!(next.as_deref(), Some("abc123"));
    }

    #[test]
    fn json_api_bracket_style_reads_bracket_form_cursor() {
        let h = r#"<https://s/x?page[after]=abc123&page[size]=50>; rel="next""#;
        let next = derive_next_after(
            Some(h),
            &els(&["e1", "e2"]),
            None,
            PageParamStyle::JsonApiBracket,
        );
        assert_eq!(next.as_deref(), Some("abc123"));
    }

    #[test]
    fn hyphenated_style_does_not_match_a_bracket_form_link() {
        // A style mismatch must not silently pick up the wrong key's value.
        let h = r#"<https://s/x?page[after]=abc123>; rel="next""#;
        let next = derive_next_after(Some(h), &els(&["e1"]), None, PageParamStyle::Hyphenated);
        assert_eq!(next, None);
    }

    #[test]
    fn param_style_keys() {
        assert_eq!(PageParamStyle::Hyphenated.after_key(), "page-after");
        assert_eq!(PageParamStyle::Hyphenated.before_key(), "page-before");
        assert_eq!(PageParamStyle::Hyphenated.size_key(), "page-size");
        assert_eq!(PageParamStyle::JsonApiBracket.after_key(), "page[after]");
        assert_eq!(PageParamStyle::JsonApiBracket.before_key(), "page[before]");
        assert_eq!(PageParamStyle::JsonApiBracket.size_key(), "page[size]");
    }

    #[test]
    fn link_header_percent_decoded() {
        let h = r#"<https://s/x?page-after=a%2Fb%20c>; rel=next"#;
        assert_eq!(
            extract_query_param("https://s/x?page-after=a%2Fb%20c", "page-after").as_deref(),
            Some("a/b c")
        );
        assert_eq!(
            parse_link_next(h).as_deref(),
            Some("https://s/x?page-after=a%2Fb%20c")
        );
    }

    #[test]
    fn no_link_full_page_with_size_uses_last_id() {
        let next = derive_next_after(
            None,
            &els(&["e1", "e2", "e3"]),
            Some(3),
            PageParamStyle::Hyphenated,
        );
        assert_eq!(next.as_deref(), Some("e3"));
    }

    #[test]
    fn no_link_short_page_is_exhausted() {
        assert_eq!(
            derive_next_after(
                None,
                &els(&["e1", "e2"]),
                Some(3),
                PageParamStyle::Hyphenated
            ),
            None
        );
    }

    #[test]
    fn no_link_no_size_is_exhausted() {
        assert_eq!(
            derive_next_after(None, &els(&["e1", "e2"]), None, PageParamStyle::Hyphenated),
            None
        );
    }

    #[test]
    fn link_header_without_next_falls_through() {
        let h = r#"<https://s/x?page-before=z>; rel="prev""#;
        assert_eq!(
            derive_next_after(Some(h), &els(&["e1"]), None, PageParamStyle::Hyphenated),
            None
        );
    }
}
