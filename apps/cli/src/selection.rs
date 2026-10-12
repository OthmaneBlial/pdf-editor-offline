use pdf_core::{Error, MAX_PAGES, Result};
use std::collections::BTreeSet;

pub fn parse(selection: Option<&str>, count: usize) -> Result<Vec<u32>> {
    parse_inner(selection, count, false)
}

pub fn parse_unique(selection: &str, count: usize) -> Result<Vec<u32>> {
    parse_inner(Some(selection), count, true)
}

fn parse_inner(selection: Option<&str>, count: usize, reject_duplicates: bool) -> Result<Vec<u32>> {
    let Some(selection) = selection else {
        return Ok((1..=count as u32).collect());
    };
    let mut pages = Vec::new();
    let mut seen = BTreeSet::new();
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
}
