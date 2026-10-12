use crate::state::TextKey;

pub const MAX_PAGE_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_SEARCH_PAGES: usize = 200;
pub const MAX_QUERY_CHARS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchKey {
    pub tab: u64,
    pub revision: u64,
    pub generation: u64,
}

pub struct PageText {
    pub key: TextKey,
    pub text: String,
    pub truncated: bool,
}

pub struct SearchHit {
    pub page: usize,
    pub excerpt: String,
}

#[derive(Default)]
pub struct SearchState {
    pub draft: String,
    pub match_case: bool,
    pub key: Option<SearchKey>,
    pub query: String,
    pub query_case: bool,
    pub running: bool,
    pub next_page: usize,
    pub hits: Vec<SearchHit>,
    pub text_was_truncated: bool,
    pub limited: bool,
    pub error: Option<String>,
    generation: u64,
}

impl SearchState {
    pub fn start(&mut self, tab: u64, revision: u64) {
        self.generation += 1;
        self.key = Some(SearchKey {
            tab,
            revision,
            generation: self.generation,
        });
        self.query = self.draft.chars().take(MAX_QUERY_CHARS).collect();
        self.query_case = self.match_case;
        self.running = !self.query.is_empty();
        self.next_page = 0;
        self.hits.clear();
        self.text_was_truncated = false;
        self.limited = false;
        self.error = None;
    }

    pub fn cancel(&mut self) {
        self.key = None;
        self.running = false;
    }

    pub fn accept(
        &mut self,
        key: SearchKey,
        page: usize,
        hit: Option<SearchHit>,
        truncated: bool,
        page_count: usize,
    ) -> bool {
        if self.key != Some(key) || !self.running {
            return false;
        }
        self.next_page = page + 1;
        self.text_was_truncated |= truncated;
        if let Some(hit) = hit {
            self.hits.push(hit);
        }
        self.limited = self.hits.len() >= MAX_SEARCH_PAGES && self.next_page < page_count;
        self.running = self.next_page < page_count && !self.limited;
        !self.running
    }

    pub fn completion_message(&self) -> String {
        if self.limited {
            return format!(
                "Search stopped at {MAX_SEARCH_PAGES} matching pages. Refine your query."
            );
        }
        let count = self.hits.len();
        let noun = if count == 1 { "page" } else { "pages" };
        let prefix = if self.text_was_truncated {
            "Search finished within text limits"
        } else {
            "Search complete"
        };
        format!("{prefix}: {count} matching {noun}.")
    }
}

pub fn limit_page_text(mut text: String) -> (String, bool) {
    let truncated = text.len() > MAX_PAGE_TEXT_BYTES;
    if truncated {
        let mut end = MAX_PAGE_TEXT_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.shrink_to_fit();
    }
    (text, truncated)
}

/// Find a literal match and return an excerpt from the original text.
pub fn literal_excerpt(text: &str, query: &str, match_case: bool) -> Option<String> {
    if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS {
        return None;
    }
    let start = if match_case {
        text.find(query)?
    } else {
        let normalized = text.to_lowercase();
        let position = normalized.find(&query.to_lowercase())?;
        let mut offset = 0;
        text.char_indices().find_map(|(index, character)| {
            let length = character.to_lowercase().map(char::len_utf8).sum::<usize>();
            let contains_start = offset <= position && position < offset + length;
            offset += length;
            contains_start.then_some(index)
        })?
    };
    let begin = text[..start]
        .char_indices()
        .rev()
        .nth(50)
        .map_or(0, |(index, _)| index);
    let end = text[start..]
        .char_indices()
        .nth(150)
        .map_or(text.len(), |(index, _)| start + index);
    let excerpt = text[begin..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some(format!(
        "{}{}{}",
        if begin > 0 { "…" } else { "" },
        excerpt,
        if end < text.len() { "…" } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_search_handles_case_unicode_and_does_not_interpret_regex() {
        assert!(literal_excerpt("Hello PDF [.*]", "hello", false).is_some());
        assert!(literal_excerpt("Hello PDF [.*]", "hello", true).is_none());
        assert!(literal_excerpt("Hello PDF [.*]", "[.*]", true).is_some());
        let text = "İ followed by CAFÉ résumé";
        assert_eq!(literal_excerpt(text, "CAFÉ", false), Some(text.to_string()));
        assert!(literal_excerpt("abc", "", false).is_none());
    }

    #[test]
    fn excerpts_and_text_limits_preserve_utf8() {
        let text = format!("{}target{}", "é".repeat(500), "🦀".repeat(500));
        let excerpt = literal_excerpt(&text, "target", true).expect("excerpt");
        assert!(excerpt.contains("target"));
        assert!(excerpt.chars().count() <= 203);
        let (text, truncated) = limit_page_text("🦀".repeat(MAX_PAGE_TEXT_BYTES));
        assert!(truncated);
        assert!(text.len() <= MAX_PAGE_TEXT_BYTES);
        assert!(text.ends_with('🦀'));
    }

    #[test]
    fn search_excerpt_is_a_compact_paragraph_without_changing_source_text() {
        let source = "Synthetic forms\n\n Name:\tCafé\r\n\r\n\u{a0}Checked: ✓";
        let excerpt = literal_excerpt(source, "Café", true).expect("excerpt");
        assert_eq!(excerpt, "Synthetic forms Name: Café Checked: ✓");
        assert!(source.contains("\n\n"));
        assert!(source.contains('\t'));
    }

    #[test]
    fn stale_searches_are_ignored_and_results_stop_at_the_bound() {
        let mut state = SearchState {
            draft: "needle".into(),
            ..Default::default()
        };
        state.start(1, 0);
        let first = state.key.expect("key");
        state.start(1, 0);
        state.accept(
            first,
            0,
            Some(SearchHit {
                page: 0,
                excerpt: "stale".into(),
            }),
            false,
            500,
        );
        assert!(state.hits.is_empty());
        let key = state.key.expect("key");
        for page in 0..MAX_SEARCH_PAGES {
            state.accept(
                key,
                page,
                Some(SearchHit {
                    page,
                    excerpt: "needle".into(),
                }),
                false,
                500,
            );
        }
        assert_eq!(state.hits.len(), MAX_SEARCH_PAGES);
        assert!(state.limited);
        assert!(!state.running);
        assert_eq!(
            state.completion_message(),
            "Search stopped at 200 matching pages. Refine your query."
        );
    }

    #[test]
    fn final_page_completes_search_disables_stop_and_reports_natural_status() {
        let mut state = SearchState {
            draft: "needle".into(),
            ..Default::default()
        };
        state.start(1, 0);
        let key = state.key.expect("key");
        assert!(state.accept(
            key,
            0,
            Some(SearchHit {
                page: 0,
                excerpt: "needle ✓".into()
            }),
            false,
            1
        ));
        assert!(!state.running);
        assert_eq!(state.next_page, 1);
        assert_eq!(
            state.completion_message(),
            "Search complete: 1 matching page."
        );
        assert!(!state.accept(key, 0, None, false, 1));
        state.start(1, 0);
        let key = state.key.expect("key");
        assert!(state.accept(key, 0, None, true, 1));
        assert_eq!(
            state.completion_message(),
            "Search finished within text limits: 0 matching pages."
        );
    }
}
