//! Looks through the whole documentation for links and pictures that don't
//! lead anywhere.
//!
//! It uses the same decisions as page rendering (see `article`), so a page
//! never says "broken" about something this check calls fine, or the other
//! way round. Web addresses are never contacted. A check that could not
//! read everything says so instead of reporting a clean result.

use std::collections::HashMap;
use std::fs;
use std::io;

use pulldown_cmark::{Event, Parser, Tag};
use serde::Serialize;

use crate::article::{
    LinkKind, LocalImage, LocalLink, anchor_is_missing, classify, collect_headings, is_image_ext,
    judge_local_image, judge_local_link, md_options, parse_front_matter, percent_decode,
};
use crate::error::CairnError;
use crate::paths::{Root, is_system_path};
use crate::search::{PageSummary, ScanProblem, scan_problem};

/// One link or picture that doesn't lead anywhere.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// `missing_page`, `missing_file`, `missing_image`, `unsupported_image`,
    /// `bad_anchor`, `outside`, `empty_link`, or `blocked`.
    pub kind: &'static str,
    /// The address as written in the page.
    pub target: String,
    /// The page or file it points at, as a workspace path, when it got that far.
    pub resolved: Option<String>,
    /// Line in the page text (counted from the title, as the editor shows it).
    pub line: usize,
    /// What is wrong, in plain words.
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PageFindings {
    pub path: String,
    pub title: String,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Serialize)]
pub struct CheckReport {
    /// Pages that were read and checked.
    pub checked: usize,
    /// False when some page could not be read: the result is then partial.
    pub complete: bool,
    pub unreadable: Vec<ScanProblem>,
    pub total_findings: usize,
    /// Pages with at least one finding.
    pub pages: Vec<PageFindings>,
}

/// Read a workspace file, keeping the reason when it can't be read.
fn read_file(root: &Root, rel: &str) -> io::Result<Vec<u8>> {
    let path = root.resolve(rel).map_err(|e| match e {
        CairnError::NotFound(_) => io::ErrorKind::NotFound.into(),
        other => io::Error::other(other.to_string()),
    })?;
    fs::read(path)
}

/// Heading ids of pages that other pages link into, read once per check.
#[derive(Default)]
pub struct Headings {
    by_page: HashMap<String, Option<Vec<String>>>,
    /// Pages that were linked to but could not be read.
    pub unreadable: Vec<ScanProblem>,
}

impl Headings {
    fn of(&mut self, root: &Root, rel: &str) -> Option<&[String]> {
        if !self.by_page.contains_key(rel) {
            let ids = match read_file(root, rel) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let events: Vec<Event> =
                        Parser::new_ext(parse_front_matter(&text).1, md_options()).collect();
                    Some(collect_headings(&events).1)
                }
                Err(e) => {
                    self.unreadable.push(scan_problem(rel.to_string(), &e));
                    None
                }
            };
            self.by_page.insert(rel.to_string(), ids);
        }
        self.by_page.get(rel).and_then(|ids| ids.as_deref())
    }
}

/// Start offset of every line, to turn a position into a line number.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

fn line_at(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&s| s <= offset)
}

fn finding(
    kind: &'static str,
    target: &str,
    resolved: Option<String>,
    line: usize,
    message: String,
) -> Finding {
    Finding {
        kind,
        target: target.to_string(),
        resolved,
        line,
        message,
    }
}

/// Everything wrong with the links and pictures in one page's text.
/// `article_rel` is the page's workspace path and `body` its text without
/// the information block at the top.
pub fn check_page(
    root: &Root,
    article_rel: &str,
    body: &str,
    headings: &mut Headings,
) -> Vec<Finding> {
    let starts = line_starts(body);
    let events: Vec<(Event, std::ops::Range<usize>)> = Parser::new_ext(body, md_options())
        .into_offset_iter()
        .collect();
    let plain: Vec<Event> = events.iter().map(|(e, _)| e.clone()).collect();
    let own_ids = collect_headings(&plain).1;
    let no_staged = HashMap::new();
    let mut out = Vec::new();

    for (event, range) in events {
        let line = line_at(&starts, range.start);
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let dest: &str = &dest_url;
                match classify(dest) {
                    LinkKind::External => {}
                    LinkKind::Unsafe => out.push(finding(
                        "blocked",
                        dest,
                        None,
                        line,
                        "This kind of link isn't allowed in a page.".into(),
                    )),
                    LinkKind::Anchor => {
                        if anchor_is_missing(&own_ids, dest) {
                            out.push(finding(
                                "bad_anchor",
                                dest,
                                None,
                                line,
                                format!(
                                    "There is no heading called “{}” on this page.",
                                    percent_decode(&dest[1..])
                                ),
                            ));
                        }
                    }
                    LinkKind::Local if dest.trim().is_empty() => out.push(finding(
                        "empty_link",
                        dest,
                        None,
                        line,
                        "This link has no address.".into(),
                    )),
                    LinkKind::Local => match judge_local_link(root, article_rel, dest) {
                        LocalLink::Found(rel) => {
                            if let Some(f) = check_fragment(root, headings, dest, &rel, line) {
                                out.push(f);
                            }
                        }
                        LocalLink::Missing(rel) => out.push(missing_link(&rel, dest, line)),
                        LocalLink::Outside => out.push(outside(dest, line, "link")),
                    },
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let dest: &str = &dest_url;
                match classify(dest) {
                    LinkKind::External => {}
                    LinkKind::Anchor | LinkKind::Unsafe => out.push(finding(
                        "blocked",
                        dest,
                        None,
                        line,
                        "This picture address isn't allowed in a page.".into(),
                    )),
                    LinkKind::Local => {
                        match judge_local_image(root, article_rel, &no_staged, dest) {
                            LocalImage::Found(_) | LocalImage::Staged(_) => {}
                            LocalImage::Missing(rel) => {
                                out.push(missing_picture(root, &rel, dest, line));
                            }
                            LocalImage::Outside => out.push(outside(dest, line, "picture")),
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn missing_link(rel: &str, dest: &str, line: usize) -> Finding {
    if is_system_path(rel) {
        return into_cairns_folder(rel, dest, line);
    }
    let kind = if rel.to_ascii_lowercase().ends_with(".md") {
        "missing_page"
    } else {
        "missing_file"
    };
    finding(
        kind,
        dest,
        Some(rel.to_string()),
        line,
        format!("“{rel}” doesn't exist. It may have been renamed, moved, or deleted."),
    )
}

fn missing_picture(root: &Root, rel: &str, dest: &str, line: usize) -> Finding {
    if is_system_path(rel) {
        return into_cairns_folder(rel, dest, line);
    }
    // It is there, but not a kind of picture Cairn shows.
    if !is_image_ext(rel) && root.resolve(rel).is_ok() {
        return finding(
            "unsupported_image",
            dest,
            Some(rel.to_string()),
            line,
            format!(
                "“{rel}” is there, but Cairn only shows png, jpg, jpeg, gif, or webp pictures."
            ),
        );
    }
    finding(
        "missing_image",
        dest,
        Some(rel.to_string()),
        line,
        format!("The picture “{rel}” is missing. It may have been renamed, moved, or deleted."),
    )
}

fn outside(dest: &str, line: usize, what: &str) -> Finding {
    finding(
        "outside",
        dest,
        None,
        line,
        format!("This {what} points outside the documentation folder."),
    )
}

fn into_cairns_folder(rel: &str, dest: &str, line: usize) -> Finding {
    finding(
        "outside",
        dest,
        Some(rel.to_string()),
        line,
        "This points into Cairn's own folder, which pages can't link to.".into(),
    )
}

/// `other.md#heading`: the page is there, but is the heading?
fn check_fragment(
    root: &Root,
    headings: &mut Headings,
    dest: &str,
    rel: &str,
    line: usize,
) -> Option<Finding> {
    if is_system_path(rel) {
        return Some(into_cairns_folder(rel, dest, line));
    }
    let (_, fragment) = dest.split_once('#')?;
    let wanted = percent_decode(fragment);
    if wanted.is_empty() || !rel.to_ascii_lowercase().ends_with(".md") {
        return None;
    }
    let ids = headings.of(root, rel)?;
    if ids.contains(&wanted) {
        return None;
    }
    Some(finding(
        "bad_anchor",
        dest,
        Some(rel.to_string()),
        line,
        format!("The page “{rel}” has no heading called “{wanted}”."),
    ))
}

/// Lets a long check report its progress and be stopped.
pub struct CheckProgress<'a> {
    pub cancelled: &'a dyn Fn() -> bool,
    /// Called after each page, with how many pages are done.
    pub on_page: &'a dyn Fn(usize),
}

/// Check every page in `pages` (from the page index). Pages are read one
/// after another and not locked, so a page edited during the check may be
/// judged by either version. Run it again after changes.
pub fn check_workspace(root: &Root, pages: &[PageSummary]) -> CheckReport {
    let never = || false;
    let nothing = |_done: usize| {};
    check_workspace_with(
        root,
        pages,
        &CheckProgress {
            cancelled: &never,
            on_page: &nothing,
        },
    )
    .expect("a check that cannot be stopped finishes")
}

/// [`check_workspace`] that reports progress and can be stopped. Returns
/// `None` if it was stopped.
pub fn check_workspace_with(
    root: &Root,
    pages: &[PageSummary],
    progress: &CheckProgress,
) -> Option<CheckReport> {
    let mut headings = Headings::default();
    let mut unreadable: Vec<ScanProblem> = Vec::new();
    let mut with_findings = Vec::new();
    let mut checked = 0;
    let mut total_findings = 0;
    for (i, page) in pages.iter().enumerate() {
        if (progress.cancelled)() {
            return None;
        }
        let bytes = match read_file(root, &page.path) {
            Ok(bytes) => bytes,
            Err(e) => {
                unreadable.push(scan_problem(page.path.clone(), &e));
                (progress.on_page)(i + 1);
                continue;
            }
        };
        checked += 1;
        let text = String::from_utf8_lossy(&bytes);
        let findings = check_page(root, &page.path, parse_front_matter(&text).1, &mut headings);
        if !findings.is_empty() {
            total_findings += findings.len();
            with_findings.push(PageFindings {
                path: page.path.clone(),
                title: page.title.clone(),
                findings,
            });
        }
        (progress.on_page)(i + 1);
    }
    for problem in std::mem::take(&mut headings.unreadable) {
        if !unreadable.iter().any(|p| p.path == problem.path) {
            unreadable.push(problem);
        }
    }
    Some(CheckReport {
        checked,
        complete: unreadable.is_empty(),
        unreadable,
        total_findings,
        pages: with_findings,
    })
}
