//! Documentation health: which pages need attention, and why.
//!
//! It is worked out from the page index (owner, status, review date, and
//! problems reading the page's information block), so it costs no extra
//! reads. It also says whether the scan managed to read everything, so a
//! share that dropped out is never reported as "all fine".

use std::collections::BTreeMap;

use serde::Serialize;

use crate::search::{PageSummary, ScanProblem, SearchIndex};

/// Every kind of problem a page can have, in the order they are checked.
pub const ISSUES: [&str; 6] = [
    "overdue",
    "no_review_date",
    "no_owner",
    "draft",
    "retired",
    "details",
];

#[derive(Debug, Serialize)]
pub struct PageHealth {
    pub page: PageSummary,
    pub issues: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// The date the report was worked out for (`YYYY-MM-DD`).
    pub today: String,
    /// Every page, with or without problems.
    pub total: usize,
    /// False when part of the documentation could not be read.
    pub complete: bool,
    pub problems: Vec<ScanProblem>,
    /// How many pages have each kind of problem (every kind is present).
    pub counts: BTreeMap<&'static str, usize>,
    /// Pages with at least one problem: overdue or unreadable ones first,
    /// then the most-affected.
    pub pages: Vec<PageHealth>,
}

/// What is wrong with one page. A retired page is finished with, so it needs
/// no owner or review: only its status (and unreadable details) show.
fn issues_for(page: &PageSummary, today: &str) -> Vec<&'static str> {
    let mut issues = Vec::new();
    if page.status.as_deref() == Some("retired") {
        issues.push("retired");
    } else {
        if page
            .review_due_on
            .as_deref()
            .is_some_and(|due| today >= due)
        {
            issues.push("overdue");
        }
        if page.last_reviewed.is_none() {
            issues.push("no_review_date");
        }
        if page.owner.is_none() {
            issues.push("no_owner");
        }
        if page.status.as_deref() == Some("draft") {
            issues.push("draft");
        }
    }
    if !page.warnings.is_empty() {
        issues.push("details");
    }
    issues
}

/// The health of every page in the index as of `today` (`YYYY-MM-DD`).
pub fn report(index: &SearchIndex, today: &str) -> Report {
    let all = index.all();
    let total = all.len();
    let mut counts: BTreeMap<&'static str, usize> = ISSUES.iter().map(|k| (*k, 0)).collect();
    let mut pages = Vec::new();
    for page in all {
        let issues = issues_for(&page, today);
        if issues.is_empty() {
            continue;
        }
        for issue in &issues {
            *counts.entry(issue).or_insert(0) += 1;
        }
        pages.push(PageHealth { page, issues });
    }
    // Pages that are overdue or have unreadable details first (those need
    // someone to act), then the most-affected, then by path.
    let not_urgent = |p: &PageHealth| !p.issues.iter().any(|i| *i == "overdue" || *i == "details");
    pages.sort_by(|a, b| {
        not_urgent(a)
            .cmp(&not_urgent(b))
            .then_with(|| b.issues.len().cmp(&a.issues.len()))
            .then_with(|| a.page.path.to_lowercase().cmp(&b.page.path.to_lowercase()))
    });
    let problems = index.problems().to_vec();
    Report {
        today: today.to_string(),
        total,
        complete: problems.is_empty(),
        problems,
        counts,
        pages,
    }
}
