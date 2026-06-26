//! Render raw bridge JSON responses into clean, LLM-friendly Markdown.

use serde_json::Value;

// ---------------------------------------------------------------------------
// Search results
// ---------------------------------------------------------------------------

/// Format a search response from the bridge into compact Markdown.
///
/// Expected `data` shape:
/// ```json
/// {
///   "totalResults": 230762,
///   "totalPages": 23077,
///   "articleProfiles": [{ "pmid", "title", "authors", "journalCitation",
///                          "snippet", "position", "doi" }, ...]
/// }
/// ```
pub fn format_search(data: &Value) -> String {
    let total = data
        .get("totalResults")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let profiles = data
        .get("articleProfiles")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if profiles.is_empty() {
        return format!("No results found. (total indexed: {total})");
    }

    let mut out = String::with_capacity(1024);
    out.push_str(&format!("Found **{total}** results\n\n"));

    for (i, article) in profiles.iter().enumerate() {
        let pmid = str_or(article, "pmid", "?");
        let title = str_or(article, "title", "Untitled");
        let authors = str_or(article, "authors", "");
        let journal = str_or(article, "journalCitation", "");
        let snippet = str_or(article, "snippet", "");

        out.push_str(&format!("{i}. **[{pmid}] {title}**\n"));
        out.push_str(&format!("   Authors: {authors}\n"));
        if !journal.is_empty() {
            out.push_str(&format!("   Journal: {journal}\n"));
        }
        if !snippet.is_empty() {
            // Truncate very long snippets to keep the output manageable.
            let truncated = truncate(snippet, 280);
            out.push_str(&format!("   Summary: {truncated}\n"));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Article detail
// ---------------------------------------------------------------------------

/// Format a detail response from the bridge into structured Markdown.
///
/// Expected `data` shape:
/// ```json
/// {
///   "pmid", "doi", "title", "authors": [{ "name", "position", "affiliations" }],
///   "affiliations": [{ "institution" }],
///   "abstract", "keywords": [{ "text", "isMeSH" }],
///   "meshTerms": [{ "text", "isMeSH" }],
///   "publicationTypes": [string],
///   "references": [{ "pmid"?, "citation" }],
///   "similarArticles": [string],
///   "fullTextSources": [{ "name", "url", "type" }],
///   "journalInfo": { "title", "volume", "issue" },
///   "conflictOfInterestStatement"
/// }
/// ```
pub fn format_detail(data: &Value) -> String {
    let mut out = String::with_capacity(2048);

    // Title
    let title = str_or(data, "title", "Untitled");
    let pmid = str_or(data, "pmid", "?");
    let doi = str_or(data, "doi", "");
    out.push_str(&format!("## {title}\n\n"));

    // Identification line
    out.push_str(&format!("**PMID:** {pmid}"));
    if !doi.is_empty() {
        out.push_str(&format!("  |  **DOI:** {doi}"));
    }
    out.push('\n');

    // Journal info
    if let Some(ji) = data.get("journalInfo") {
        let jname = str_or(ji, "title", "");
        let vol = str_or(ji, "volume", "");
        let iss = str_or(ji, "issue", "");
        if !jname.is_empty() {
            let mut journal_line = format!("**Journal:** {jname}");
            let mut parts: Vec<String> = Vec::new();
            if !vol.is_empty() {
                parts.push(vol.to_string());
            }
            if !iss.is_empty() {
                parts.push(format!("({iss})"));
            }
            if !parts.is_empty() {
                journal_line.push_str(&format!(", {}", parts.join(" ")));
            }
            out.push_str(&format!("{journal_line}\n"));
        }
    }
    out.push('\n');

    // Authors
    if let Some(authors) = data.get("authors").and_then(|v| v.as_array()) {
        let names: Vec<&str> = authors
            .iter()
            .filter_map(|a| a.get("name").and_then(|v| v.as_str()))
            .collect();
        if !names.is_empty() {
            out.push_str("**Authors:** ");
            out.push_str(&names.join(", "));
            out.push_str("\n\n");
        }
    }

    // Publication types — clean noisy strings
    if let Some(types) = data.get("publicationTypes").and_then(|v| v.as_array()) {
        let cleaned: Vec<&str> = types
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| is_clean_pub_type(s))
            .collect();
        if !cleaned.is_empty() {
            out.push_str("**Publication Types:** ");
            out.push_str(&cleaned.join(", "));
            out.push_str("\n\n");
        }
    }

    // Abstract
    let abstract_text = str_or(data, "abstract", "");
    if !abstract_text.is_empty() {
        out.push_str("### Abstract\n\n");
        out.push_str(abstract_text.trim());
        out.push_str("\n\n");
    }

    // Keywords (non-MeSH)
    let keywords = collect_labelled_items(data, "keywords", "isMeSH", false);
    if !keywords.is_empty() {
        out.push_str("**Keywords:** ");
        out.push_str(&keywords.join(", "));
        out.push_str("\n\n");
    }

    // MeSH terms
    let mesh = collect_labelled_items(data, "meshTerms", "isMeSH", true);
    if !mesh.is_empty() {
        out.push_str("**MeSH Terms:**\n");
        for term in &mesh {
            out.push_str(&format!("- {term}\n"));
        }
        out.push('\n');
    }

    // References — deduplicate and clean citations
    if let Some(refs) = data.get("references").and_then(|v| v.as_array()) {
        let mut seen = std::collections::HashSet::new();
        let clean_refs: Vec<String> = refs
            .iter()
            .filter_map(|r| {
                let citation = clean_citation(r.get("citation").and_then(|v| v.as_str())?);
                if seen.insert(citation.clone()) {
                    Some(citation)
                } else {
                    None
                }
            })
            .take(20) // cap to avoid bloating context
            .collect();
        if !clean_refs.is_empty() {
            out.push_str("### References\n\n");
            for (i, entry) in clean_refs.iter().enumerate() {
                let num = i + 1;
                out.push_str(&format!("{num}. {entry}\n"));
            }
            out.push('\n');
        }
    }

    // Full-text sources
    if let Some(sources) = data.get("fullTextSources").and_then(|v| v.as_array()) {
        let links: Vec<String> = sources
            .iter()
            .filter_map(|s| {
                let url = s.get("url")?.as_str()?;
                let name = s.get("name")?.as_str()?;
                if url == "#" || url.is_empty() {
                    None
                } else {
                    Some(format!("- [{name}]({url})"))
                }
            })
            .collect();
        if !links.is_empty() {
            out.push_str("**Full Text:**\n");
            out.push_str(&links.join("\n"));
            out.push_str("\n\n");
        }
    }

    // Conflict of interest
    let coi = str_or(data, "conflictOfInterestStatement", "");
    if !coi.is_empty() && coi != "None declared." {
        out.push_str("**Conflict of Interest:** ");
        out.push_str(&truncate(coi, 300));
        out.push_str("\n");
    }

    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn str_or<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(|v| v.as_str()).unwrap_or(default)
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        // Don't cut mid-UTF-8; find a safe boundary.
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        let trimmed = &s[..end];
        // Also try to break at a space.
        if let Some(space) = trimmed.rfind(' ') {
            &s[..space]
        } else {
            trimmed
        }
    }
}

/// Collect text items from an array, optionally filtering on a boolean field.
fn collect_labelled_items<'a>(
    data: &'a Value,
    array_key: &str,
    flag_key: &str,
    flag_value: bool,
) -> Vec<&'a str> {
    data.get(array_key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let matches = item
                        .get(flag_key)
                        .and_then(|v| v.as_bool())
                        .map(|b| b == flag_value)
                        .unwrap_or(false);
                    if matches {
                        item.get("text").and_then(|v| v.as_str())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Check whether a raw publication-type string is clean (free of HTML noise).
fn is_clean_pub_type(s: &str) -> bool {
    let noise = [
        "Search in PubMed",
        "Search in MeSH",
        "Add to Search",
        "Actions",
    ];
    !noise.iter().any(|n| s.contains(n)) && s.len() < 120
}

/// Remove HTML-like noise from a raw citation string (newlines, extra
/// whitespace, trailing "PubMed" / "PMC" labels).
fn clean_citation(raw: &str) -> String {
    let s = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            let noise = ["PubMed", "PMC", "Actions", "-", "Search in", "Add to"];
            !noise.iter().any(|n| l.starts_with(n)) && !l.is_empty()
        })
        .collect::<Vec<_>>()
        .join(" ");
    s.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_search_basic() {
        let data = json!({
            "totalResults": 2,
            "totalPages": 1,
            "articleProfiles": [
                {
                    "pmid": "12345678",
                    "title": "A Study on Something",
                    "authors": "Smith J, Doe A",
                    "journalCitation": "Nature. 2024;1(1):1-10.",
                    "snippet": "This is a short summary.",
                    "doi": "10.1234/a"
                },
                {
                    "pmid": "87654321",
                    "title": "Another Study",
                    "authors": "Brown B",
                    "journalCitation": "",
                    "snippet": ""
                }
            ]
        });

        let rendered = format_search(&data);
        assert!(rendered.contains("Found **2** results"));
        assert!(rendered.contains("[12345678] A Study on Something"));
        assert!(rendered.contains("Authors: Smith J, Doe A"));
        assert!(rendered.contains("Journal: Nature"));
        assert!(rendered.contains("[87654321] Another Study"));
    }

    #[test]
    fn format_search_empty() {
        let data = json!({ "totalResults": 0, "articleProfiles": [] });
        let rendered = format_search(&data);
        assert!(rendered.contains("No results found"));
    }

    #[test]
    fn format_detail_basic() {
        let data = json!({
            "pmid": "12345678",
            "doi": "10.1234/a",
            "title": "My Paper Title",
            "authors": [
                { "name": "Alice A", "position": 1, "affiliations": [] },
                { "name": "Bob B", "position": 2, "affiliations": [] }
            ],
            "journalInfo": { "title": "Nature", "volume": "1", "issue": "2" },
            "abstract": "This is the abstract text.",
            "keywords": [
                { "text": "Humans", "isMeSH": false }
            ],
            "meshTerms": [
                { "text": "Cancer / therapy", "isMeSH": true }
            ],
            "publicationTypes": [
                "Review\n    Actions\n              Search in PubMed\n",
                "Review",
                "Journal Article"
            ],
            "references": [
                { "pmid": "111", "citation": "Smith et al. Nature. 2020;1:1-5.\n            \n              \n                \n                  -\n                  PubMed" }
            ],
            "fullTextSources": [
                { "name": "Free PMC", "url": "https://pmc.ncbi.nlm.nih.gov/articles/123/", "type": "1" },
                { "name": "Full text links", "url": "#", "type": "1" }
            ],
            "conflictOfInterestStatement": ""
        });

        let rendered = format_detail(&data);

        assert!(rendered.contains("## My Paper Title"));
        assert!(rendered.contains("**PMID:** 12345678"));
        assert!(rendered.contains("**DOI:** 10.1234/a"));
        assert!(rendered.contains("**Journal:** Nature, 1 (2)"));
        assert!(rendered.contains("**Authors:** Alice A, Bob B"));
        // Publication types should be cleaned — only "Review" and "Journal Article", not the noisy one
        assert!(rendered.contains("Review, Journal Article"));
        assert!(rendered.contains("### Abstract"));
        assert!(rendered.contains("**Keywords:** Humans"));
        assert!(rendered.contains("**MeSH Terms:**"));
        assert!(rendered.contains("- Cancer / therapy"));
        assert!(rendered.contains("### References"));
        // Full text: Free PMC link present, "#" link filtered out
        assert!(rendered.contains("[Free PMC]"));
        assert!(!rendered.contains("[Full text links]"));
        // Citation should be cleaned of noise
        assert!(rendered.contains("Smith et al. Nature. 2020;1:1-5."));
        assert!(!rendered.contains("Search in PubMed"));
    }
}
