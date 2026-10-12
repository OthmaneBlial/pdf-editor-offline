//! Bounded one-based page ranges shared by the CLI and native desktop.
use crate::{Error, MAX_PAGES, Result};
use std::collections::BTreeSet;

/// Select all pages when omitted; explicit repeats are deduplicated in caller order.
pub fn parse(selection: Option<&str>, count: usize) -> Result<Vec<u32>> {
    parse_inner(selection, count, false)
}

/// Parse comma-separated numbers and ascending ranges, preserving order and rejecting repeats.
pub fn parse_unique(selection: &str, count: usize) -> Result<Vec<u32>> {
    parse_inner(Some(selection), count, true)
}

fn parse_inner(selection: Option<&str>, count: usize, reject_duplicates: bool) -> Result<Vec<u32>> {
    if count == 0 || count > MAX_PAGES {
        return Err(Error::Pages(
            "document page count is outside the supported range",
        ));
    }
    let Some(selection) = selection else {
        return Ok((1..=count as u32).collect());
    };
    if selection.len() > 256 * 1024 {
        return Err(Error::Limit("page selection exceeds 256 KiB"));
    }
    let mut pages = Vec::new();
    let mut seen = BTreeSet::new();
    let mut expanded = 0usize;
    for item in selection.split(',') {
        let (first, last) = item
            .trim()
            .split_once('-')
            .map_or((item.trim(), item.trim()), |pair| pair);
        let number = |value: &str| {
            value
                .trim()
                .parse::<u32>()
                .map_err(|_| Error::Pages("use comma-separated page numbers or ascending ranges"))
        };
        let (first, last) = (number(first)?, number(last)?);
        if first == 0 || first > last || last as usize > count || last as usize > MAX_PAGES {
            return Err(Error::Pages("page range is reversed or outside document"));
        }
        expanded += (last - first + 1) as usize;
        if expanded > MAX_PAGES * 8 {
            return Err(Error::Limit(
                "page selection expands beyond the processing budget",
            ));
        }
        for page in first..=last {
            if seen.insert(page) {
                pages.push(page);
            } else if reject_duplicates {
                return Err(Error::Pages("page selection contains duplicates"));
            }
        }
    }
    if pages.is_empty() {
        return Err(Error::Pages("select at least one page"));
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selections_preserve_order_and_reject_invalid_ranges() {
        assert_eq!(parse(Some("3,1-2,3"), 3).ok(), Some(vec![3, 1, 2]));
        for value in ["", "0", "4", "2-1", "1-999999999", "1--2", "one", "1,"] {
            assert!(parse(Some(value), 3).is_err(), "{value}");
        }
    }

    #[test]
    fn organization_selections_preserve_order_and_refuse_duplicates() {
        assert_eq!(parse_unique("3,1-2", 3).ok(), Some(vec![3, 1, 2]));
        assert!(parse_unique("1,1,2,3", 3).is_err());
        assert!(parse_unique("1-2,2", 3).is_err());
    }

    #[test]
    fn selection_limits_bound_omitted_and_explicit_inputs() {
        assert!(parse(None, 0).is_err());
        assert!(parse(None, MAX_PAGES + 1).is_err());
        assert_eq!(
            parse(None, MAX_PAGES).map(|pages| pages.len()).ok(),
            Some(MAX_PAGES)
        );
        assert!(matches!(
            parse_unique(&" ".repeat(256 * 1024 + 1), 1),
            Err(Error::Limit(_))
        ));
        let every_page = (1..=MAX_PAGES)
            .map(|page| page.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse_unique(&every_page, MAX_PAGES)
                .map(|pages| pages.len())
                .ok(),
            Some(MAX_PAGES)
        );
        let full = format!("1-{MAX_PAGES}");
        assert_eq!(
            parse(Some(&format!("{full},{full}")), MAX_PAGES)
                .map(|pages| pages.len())
                .ok(),
            Some(MAX_PAGES)
        );
        assert!(matches!(
            parse(Some(&vec![full; 9].join(",")), MAX_PAGES),
            Err(Error::Limit(_))
        ));
    }
}
