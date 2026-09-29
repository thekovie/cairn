//! Keeping links right when pages and folders move.
//!
//! Links between pages are ordinary relative Markdown links, so moving a page
//! breaks links *to* it (from other pages) and, when it changes folder,
//! links *from* it. [`rewrite_links`] fixes both: it finds each local link's
//! destination in the source text and replaces only that, leaving the rest
//! of the page exactly as written.

use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, LinkType, Parser, Tag};

use crate::article::{
    encode_path, is_local_link, md_options, parse_front_matter, resolve_local_target,
};
use crate::publish::assets_dir_rel;

/// What moved, and where to.
#[derive(Debug, Clone, PartialEq)]
pub enum PathMap {
    /// A page, together with its `.assets` pictures folder.
    Page { from: String, to: String },
    /// A folder and everything in it.
    Folder { from: String, to: String },
}

impl PathMap {
    /// Where `rel` lives after the move, or `None` if it doesn't move.
    pub fn apply(&self, rel: &str) -> Option<String> {
        match self {
            PathMap::Page { from, to } => {
                if rel.eq_ignore_ascii_case(from) {
                    return Some(to.clone());
                }
                replace_prefix(rel, &assets_dir_rel(from), &assets_dir_rel(to))
            }
            PathMap::Folder { from, to } => replace_prefix(rel, from, to),
        }
    }

    /// A word every link into the moved place must contain: the page's file
    /// name (without `.md`) or the folder's name. Lowercase.
    pub fn needle(&self) -> String {
        let from = match self {
            PathMap::Page { from, .. } => from.strip_suffix(".md").unwrap_or(from),
            PathMap::Folder { from, .. } => from.as_str(),
        };
        from.rsplit('/').next().unwrap_or(from).to_lowercase()
    }
}

fn replace_prefix(rel: &str, from: &str, to: &str) -> Option<String> {
    let head = rel.get(..from.len())?;
    if !head.eq_ignore_ascii_case(from) {
        return None;
    }
    match &rel[from.len()..] {
        "" => Some(to.to_string()),
        rest if rest.starts_with('/') => Some(format!("{to}{rest}")),
        _ => None,
    }
}

/// Whether `text` might link into the moved place (a cheap pre-check before
/// parsing a page).
pub fn may_link_to(text: &str, map: &PathMap) -> bool {
    let needle = map.needle();
    let lower = text.to_lowercase();
    lower.contains(&needle) || lower.contains(&encode_path(&needle).to_lowercase())
}

/// Relative, URL-encoded link from the page `from_page` to `target`.
pub fn relative_link(from_page: &str, target: &str) -> String {
    let from_dir: Vec<&str> = from_page.split('/').collect();
    let from_dir = &from_dir[..from_dir.len() - 1];
    let tgt: Vec<&str> = target.split('/').collect();
    let common = from_dir
        .iter()
        .zip(&tgt[..tgt.len() - 1])
        .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
        .count();
    let mut parts: Vec<&str> = vec![".."; from_dir.len() - common];
    parts.extend(&tgt[common..]);
    encode_path(&parts.join("/"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rewritten {
    pub text: String,
    /// Links changed.
    pub updated: usize,
    /// Links that needed changing but couldn't be found in the source text
    /// (written in an unusual way); left as they were.
    pub unmatched: usize,
}

/// Rewrite the local links in a page that was at `old_rel` and is now at
/// `new_rel`, after the move described by `map`. Returns `None` when no
/// link needs to change.
pub fn rewrite_links(text: &str, old_rel: &str, new_rel: &str, map: &PathMap) -> Option<Rewritten> {
    let (_, body) = parse_front_matter(text);
    let offset = body.as_ptr() as usize - text.as_ptr() as usize;
    let new_dest = |dest: &str| -> Option<String> {
        if !is_local_link(dest) {
            return None;
        }
        let target = resolve_local_target(old_rel, dest)?;
        let moved = map.apply(&target).unwrap_or(target);
        if resolve_local_target(new_rel, dest).is_some_and(|t| t.eq_ignore_ascii_case(&moved)) {
            return None; // still right from the new place
        }
        let suffix = dest.find(['#', '?']).map_or("", |i| &dest[i..]);
        Some(format!("{}{suffix}", relative_link(new_rel, &moved)))
    };

    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut unmatched = 0;
    let parser = Parser::new_ext(body, md_options());
    for (_, def) in parser.reference_definitions().iter() {
        let Some(new) = new_dest(&def.dest) else {
            continue;
        };
        match find_dest(&body[def.span.clone()], &def.dest, "]:") {
            Some(r) => edits.push((shift(r, offset + def.span.start), new)),
            None => unmatched += 1,
        }
    }
    for (event, range) in parser.into_offset_iter() {
        let (Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            ..
        })
        | Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url,
            ..
        })) = event
        else {
            continue;
        };
        let Some(new) = new_dest(&dest_url) else {
            continue;
        };
        match find_dest(&body[range.clone()], &dest_url, "](") {
            Some(r) => edits.push((shift(r, offset + range.start), new)),
            None => unmatched += 1,
        }
    }
    if edits.is_empty() && unmatched == 0 {
        return None;
    }
    edits.sort_by_key(|(r, _)| r.start);
    edits.dedup_by_key(|(r, _)| r.start);
    let mut out = text.to_string();
    let mut last_start = usize::MAX;
    for (range, new) in edits.iter().rev() {
        if range.end > last_start {
            continue; // overlapping (shouldn't happen): keep the first one
        }
        out.replace_range(range.clone(), new);
        last_start = range.start;
    }
    Some(Rewritten {
        text: out,
        updated: edits.len(),
        unmatched,
    })
}

fn shift(r: Range<usize>, by: usize) -> Range<usize> {
    r.start + by..r.end + by
}

/// Find where the destination `dest` is written in `src` (a link or a
/// reference definition), right after the last `marker` that is followed by
/// it. Handles `<angle brackets>`. The range covers the destination itself
/// (with its brackets).
fn find_dest(src: &str, dest: &str, marker: &str) -> Option<Range<usize>> {
    if dest.is_empty() {
        return None;
    }
    for (pos, _) in src.rmatch_indices(marker) {
        let after = pos + marker.len();
        let rest = &src[after..];
        let start = after + (rest.len() - rest.trim_start().len());
        let here = &src[start..];
        if let Some(inner) = here.strip_prefix('<')
            && inner.starts_with(dest)
            && inner[dest.len()..].starts_with('>')
        {
            return Some(start..start + dest.len() + 2);
        }
        if here.starts_with(dest)
            && here[dest.len()..]
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == ')')
        {
            return Some(start..start + dest.len());
        }
    }
    None
}

/// Set the page's title (its first level-1 heading), keeping everything
/// else. Adds a heading at the top if there is none.
pub fn set_title(text: &str, title: &str) -> String {
    let title: String = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let (_, body) = parse_front_matter(text);
    let offset = body.as_ptr() as usize - text.as_ptr() as usize;
    let heading = Parser::new_ext(body, md_options())
        .into_offset_iter()
        .find_map(|(event, range)| match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => Some(range),
            _ => None,
        });
    let mut out = text.to_string();
    match heading {
        Some(range) => {
            let src = &body[range.clone()];
            let trailing = &src[src.trim_end_matches(['\r', '\n']).len()..];
            out.replace_range(shift(range, offset), &format!("# {title}{trailing}"));
        }
        None => out.insert_str(offset, &format!("# {title}\n\n")),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(from: &str, to: &str) -> PathMap {
        PathMap::Page {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn relative_links_between_folders() {
        assert_eq!(relative_link("Guides/a.md", "Guides/b.md"), "b.md");
        assert_eq!(relative_link("Guides/a.md", "Ref/c.md"), "../Ref/c.md");
        assert_eq!(relative_link("a.md", "Ref/My Page.md"), "Ref/My%20Page.md");
        assert_eq!(relative_link("A/B/x.md", "A/y.md"), "../y.md");
    }

    #[test]
    fn page_map_moves_its_pictures_too() {
        let m = page("Guides/a.md", "Ref/b.md");
        assert_eq!(m.apply("guides/A.md").as_deref(), Some("Ref/b.md"));
        assert_eq!(
            m.apply("Guides/a.assets/p.png").as_deref(),
            Some("Ref/b.assets/p.png")
        );
        assert_eq!(m.apply("Guides/ab.md"), None);
        let f = PathMap::Folder {
            from: "Guides".into(),
            to: "How-to".into(),
        };
        assert_eq!(f.apply("Guides/x/y.md").as_deref(), Some("How-to/x/y.md"));
        assert_eq!(f.apply("Guidesx/y.md"), None);
    }

    #[test]
    fn links_to_a_moved_page_are_updated_and_nothing_else_changes() {
        let text = "# Home\n\nSee [the printer](Guides/printer.md#setup) and \
                    [other](Guides/other.md) and [web](https://x.test/Guides/printer.md).\n\n\
                    [ref]: <Guides/printer.md> \"Printer\"\n";
        let m = page("Guides/printer.md", "Hardware/Printers/printer.md");
        let r = rewrite_links(text, "index.md", "index.md", &m).unwrap();
        assert_eq!(r.updated, 2);
        assert_eq!(r.unmatched, 0);
        assert!(r.text.contains("(Hardware/Printers/printer.md#setup)"));
        assert!(
            r.text
                .contains("[ref]: Hardware/Printers/printer.md \"Printer\"")
        );
        assert!(r.text.contains("(Guides/other.md)"));
        assert!(r.text.contains("(https://x.test/Guides/printer.md)"));
    }

    #[test]
    fn a_moved_page_keeps_its_own_links_and_pictures() {
        let text = "---\nowner: Sam\n---\n# Printer\n\n![shot](printer.assets/s.png)\n\
                    Back to [setup](setup.md).\n";
        let m = page("Guides/printer.md", "Hardware/printer-guide.md");
        let r = rewrite_links(text, "Guides/printer.md", "Hardware/printer-guide.md", &m).unwrap();
        assert!(r.text.starts_with("---\nowner: Sam\n---\n"));
        assert!(r.text.contains("(printer-guide.assets/s.png)"));
        assert!(r.text.contains("(../Guides/setup.md)"));
        assert_eq!(r.updated, 2);
    }

    #[test]
    fn nothing_to_change_returns_none() {
        let m = page("Guides/printer.md", "Guides/printer-2.md");
        assert!(rewrite_links("No links here.", "a.md", "a.md", &m).is_none());
        // Moving a whole folder keeps links between its own pages.
        let f = PathMap::Folder {
            from: "Guides".into(),
            to: "How-to".into(),
        };
        assert!(rewrite_links("[b](b.md)", "Guides/a.md", "How-to/a.md", &f).is_none());
    }

    #[test]
    fn titles_are_replaced_or_added() {
        assert_eq!(set_title("# Old\n\nBody\n", "New"), "# New\n\nBody\n");
        assert_eq!(set_title("Old\n===\n\nBody", "New"), "# New\n\nBody");
        assert_eq!(
            set_title("---\nstatus: draft\n---\nBody\n", "New"),
            "---\nstatus: draft\n---\n# New\n\nBody\n"
        );
    }
}
