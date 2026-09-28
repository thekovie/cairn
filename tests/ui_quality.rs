//! Guards for the UI commitments made for a mixed-age, low-tech-confidence
//! audience, so a later change can't quietly regress them:
//! - WCAG contrast in every theme (body text >= 7:1, other text >= 4.5:1,
//!   focus rings and control borders >= 3:1);
//! - focus outlines are never removed, zoom is never disabled;
//! - every control has a visible label; HTML is only inserted when it is
//!   constant or server-sanitized; nothing is loaded from the network.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn asset(rel: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(rel),
    )
    .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

// -------------------------------------------------------------- contrast

type Rgb = (f64, f64, f64);

fn theme_block<'a>(css: &'a str, selector: &str) -> &'a str {
    let start = css
        .find(selector)
        .unwrap_or_else(|| panic!("no {selector} block"));
    let open = start + css[start..].find('{').unwrap();
    let close = open + css[open..].find('}').unwrap();
    &css[open + 1..close]
}

fn colors(block: &str) -> HashMap<String, Rgb> {
    let mut out = HashMap::new();
    for line in block.lines() {
        let Some(rest) = line.trim().strip_prefix("--") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_end_matches(';').trim();
        if let Some(hex) = value.strip_prefix('#').filter(|h| h.len() == 6) {
            let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
            out.insert(name.trim().to_string(), (c(0), c(2), c(4)));
        }
    }
    out
}

fn luminance((r, g, b): Rgb) -> f64 {
    let lin = |c: f64| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn every_theme_meets_the_contrast_targets() {
    let css = asset("css/tokens.css");
    let body_text = [("fg", "bg"), ("fg", "surface"), ("fg", "surface-2")];
    let other_text = [
        ("fg-muted", "bg"),
        ("fg-muted", "surface"),
        ("fg-muted", "surface-2"),
        ("accent", "bg"),
        ("accent", "surface"),
        ("accent", "accent-soft"),
        ("accent-fg", "accent"),
        ("accent-fg", "accent-hover"),
        ("danger", "surface"),
        ("danger", "danger-soft"),
        ("warn", "surface"),
        ("warn", "warn-soft"),
        ("ok", "surface"),
        ("ok", "ok-soft"),
        ("fg", "accent-soft"),
        ("fg", "danger-soft"),
        ("fg", "ok-soft"),
        ("fg", "mark"),
    ];
    let non_text = [
        ("focus", "bg"),
        ("focus", "surface"),
        ("border-strong", "surface"),
    ];

    let mut failures = Vec::new();
    for theme in ["light", "dark", "contrast"] {
        let c = colors(theme_block(&css, &format!("[data-theme=\"{theme}\"]")));
        let mut check = |pairs: &[(&str, &str)], min: f64| {
            for (fg, bg) in pairs {
                match (c.get(*fg), c.get(*bg)) {
                    (Some(a), Some(b)) => {
                        let r = ratio(*a, *b);
                        if r < min {
                            failures.push(format!(
                                "{theme}: --{fg} on --{bg} is {r:.2}:1, needs {min}:1"
                            ));
                        }
                    }
                    _ => failures.push(format!("{theme}: missing --{fg} or --{bg}")),
                }
            }
        };
        check(&body_text, 7.0);
        check(&other_text, 4.5);
        check(&non_text, 3.0);
    }
    assert!(
        failures.is_empty(),
        "contrast failures:\n{}",
        failures.join("\n")
    );
}

// ---------------------------------------------------------------- markup

fn js_files() -> Vec<(String, String)> {
    [
        "js/core.js",
        "js/app.js",
        "js/views.js",
        "js/setup.js",
        "js/editor.js",
        "js/templates.js",
        "js/export.js",
        "js/timezone.js",
        "js/visual.js",
    ]
    .iter()
    .map(|f| (f.to_string(), asset(f)))
    .collect()
}

#[test]
fn focus_outlines_are_never_removed() {
    for file in [
        "css/tokens.css",
        "css/app.css",
        "css/markdown.css",
        "css/print.css",
    ] {
        let css = asset(file).replace(' ', "");
        for banned in ["outline:none", "outline:0;", "outline:0}"] {
            assert!(
                !css.contains(banned),
                "{file} removes focus outlines ({banned})"
            );
        }
    }
    assert!(
        asset("css/app.css").contains(":focus-visible"),
        "a visible focus style exists"
    );
}

#[test]
fn zoom_is_never_disabled_and_the_page_declares_its_language() {
    let html = asset("index.html");
    assert!(html.contains("<html lang=\"en\""));
    let lower = html.to_lowercase();
    assert!(!lower.contains("user-scalable=no") && !lower.contains("maximum-scale"));
    assert!(html.contains("Skip to main content"));
}

#[test]
fn html_is_only_inserted_when_constant_or_server_sanitized() {
    for (file, src) in js_files() {
        for (n, line) in src.lines().enumerate() {
            if line.contains("innerHTML") {
                let justified = line.contains("sanitized") || line.contains("constant");
                assert!(
                    justified,
                    "{file}:{} inserts HTML without saying why it is safe",
                    n + 1
                );
            }
        }
        assert!(
            !src.contains("insertAdjacentHTML") && !src.contains("document.write"),
            "{file}"
        );
        assert!(
            !src.contains("eval(") && !src.contains("new Function"),
            "{file}"
        );
    }
}

#[test]
fn every_input_has_a_label_and_no_placeholder_stands_in_for_one() {
    for (file, src) in js_files() {
        assert!(
            !src.contains("placeholder"),
            "{file}: placeholders must not replace labels"
        );
        for needle in ["h('input'", "h('textarea'", "h('select'"] {
            for (i, _) in src.match_indices(needle) {
                let window: String = src[i..].chars().take(260).collect();
                let labelled = window.contains("id:")
                    || window.contains("'aria-label'")
                    || window.contains("type: 'radio'")
                    || window.contains("type: 'checkbox'")
                    || window.contains("type: 'file'");
                let snippet: String = window.chars().take(100).collect();
                assert!(
                    labelled,
                    "{file}: control without an id or label: {snippet}"
                );
            }
        }
    }
}

#[test]
fn buttons_always_carry_visible_words() {
    let core = asset("js/core.js");
    assert!(
        core.contains("Buttons must have a visible text label"),
        "button() enforces a label"
    );
    for (file, src) in js_files() {
        if file == "js/core.js" {
            continue;
        }
        // Raw <button> elements are only allowed with text content (setup's big choices).
        let lines: Vec<&str> = src.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if line.contains("h('button'") {
                let near = lines[n..(n + 4).min(lines.len())].join(" ");
                assert!(
                    near.contains("h('strong'"),
                    "{file}:{} raw button without words",
                    n + 1
                );
            }
        }
    }
}

#[test]
fn only_the_chosen_editing_mode_is_shown() {
    // `.pane` sets `display: flex`, so hiding the visual pane needs a more
    // specific rule than `.pane-visual` alone (which `.pane` would override).
    let css = asset("css/app.css");
    assert!(
        css.contains(".editor-panes:not([data-view=\"visual\"]) .pane-visual { display: none; }")
    );
    assert!(css.contains(".editor-panes[data-view=\"visual\"] .pane-preview { display: none; }"));
}

#[test]
fn the_vendored_editor_engine_never_talks_to_the_network() {
    let bundle = asset("vendor/milkdown.js");
    for call in [
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "sendBeacon",
        "EventSource",
        "importScripts",
        "eval(",
        "new Function",
    ] {
        assert!(!bundle.contains(call), "the editor bundle uses {call}");
    }
}

#[test]
fn the_whole_ui_is_embedded_and_nothing_loads_from_the_network() {
    let names = cairn::server::embedded_asset_names();
    for needed in [
        "index.html",
        "css/tokens.css",
        "css/app.css",
        "css/markdown.css",
        "css/print.css",
        "js/app.js",
        "js/core.js",
        "js/views.js",
        "js/setup.js",
        "js/editor.js",
        "js/templates.js",
        "js/export.js",
        "js/timezone.js",
        "js/visual.js",
        "vendor/milkdown.js",
        "fonts/InterVariable-latin.woff2",
        "fonts/Inter-LICENSE.txt",
    ] {
        assert!(
            names.iter().any(|n| n == needed),
            "{needed} is not embedded"
        );
    }
    let mut sources = js_files();
    for css in [
        "css/tokens.css",
        "css/app.css",
        "css/markdown.css",
        "css/print.css",
        "index.html",
    ] {
        sources.push((css.to_string(), asset(css)));
    }
    for (file, src) in sources {
        for remote in [
            "//cdn",
            "googleapis",
            "gstatic",
            "unpkg",
            "<script src=\"http",
            "@import url(\"http",
        ] {
            assert!(
                !src.contains(remote),
                "{file} loads something from the network ({remote})"
            );
        }
    }
}
