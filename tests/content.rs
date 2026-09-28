//! Acceptance criterion 8 plus rendering and search rules: picture
//! validation and publishing, sanitized Markdown, broken links, no remote
//! fetching, lenient metadata, and search excerpts.

mod common;

use std::collections::HashMap;
use std::fs;

use cairn::article::{RenderContext, Rendered, parse_front_matter, render};
use cairn::error::CairnError;
use cairn::images::{ImageLimits, asset_file_name, validate_image};
use cairn::locks;
use cairn::paths::Root;
use cairn::publish::{
    PublishOptions, PublishOutcome, PublishRequest, assets_dir_rel, image_link_for, publish,
};
use cairn::search::SearchIndex;

const PAGE: &str = "Guides/new-user-setup.md";

fn rendered(root: &Root, rel: &str, md: &str, key: &str) -> Rendered {
    let staged = HashMap::new();
    render(
        md,
        &RenderContext {
            root,
            article_rel: rel,
            read_key: key,
            staged: &staged,
        },
    )
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let dest = to.join(entry.path().strip_prefix(from).unwrap());
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest).unwrap();
        } else {
            fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

// ------------------------------------------------------------------ AC 8

#[test]
fn publishing_a_picture_copies_it_beside_the_page_with_a_working_relative_link() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, PAGE, &alex).unwrap();

    let bytes = common::png(64, 32, 42);
    let info = validate_image(&bytes, Some("Screen Shot.png"), &ImageLimits::default()).unwrap();
    let name = asset_file_name(Some("Screen Shot.png"), &bytes, info.ext, 8);
    assert!(name.starts_with("screen-shot-") && name.ends_with(".png"));
    let target = format!("{}/{name}", assets_dir_rel(PAGE));
    let link = image_link_for(PAGE, &name);
    assert_eq!(link, format!("new-user-setup.assets/{name}"));

    let content = format!("# New user setup\n\n![Login screen]({link})\n");
    let request = PublishRequest {
        root: &ws.root,
        article_rel: PAGE,
        identity: &alex,
        base_hash: None,
        content: &content,
        staged: vec![(target.clone(), bytes.clone())],
    };
    match publish(request, &PublishOptions::default()).unwrap() {
        PublishOutcome::Published { new_assets, .. } => assert_eq!(new_assets, vec![target]),
        other => panic!("{other:?}"),
    }

    // On disk: plain Markdown with a relative link, picture next to it.
    let on_disk = ws.path.join("Guides/new-user-setup.assets").join(&name);
    assert_eq!(fs::read(&on_disk).unwrap(), bytes);
    assert!(
        fs::read_to_string(ws.path.join(PAGE))
            .unwrap()
            .contains(&format!("]({link})"))
    );

    // "Another computer": a copy of the folder at a different location
    // resolves the same relative link.
    let copy = tempfile::tempdir().unwrap();
    copy_dir(&ws.path, copy.path());
    let other = Root::new(copy.path()).unwrap();
    let md = fs::read_to_string(copy.path().join(PAGE)).unwrap();
    let html = rendered(&other, PAGE, &md, "K");
    assert!(html.broken_links.is_empty(), "{:?}", html.broken_links);
    assert!(
        html.html
            .contains(&format!("/ws-file/Guides/new-user-setup.assets/{name}?k=K"))
    );
}

#[test]
fn supported_formats_are_accepted_by_content() {
    let limits = ImageLimits::default();
    assert_eq!(
        validate_image(&common::png(3, 3, 1), Some("a.png"), &limits)
            .unwrap()
            .ext,
        "png"
    );
    assert_eq!(
        validate_image(&common::jpeg(3, 3), Some("a.JPEG"), &limits)
            .unwrap()
            .ext,
        "jpg"
    );
    // A minimal valid 1×1 GIF.
    let gif = [
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff,
        0x21, 0xf9, 4, 1, 0, 0, 0, 0, 0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 1, 0, 0x3b,
    ];
    assert_eq!(validate_image(&gif, None, &limits).unwrap().ext, "gif");
}

#[test]
fn invalid_pictures_are_rejected_with_a_clear_reason() {
    let limits = ImageLimits::default();
    let reject = |bytes: &[u8], name: Option<&str>, limits: &ImageLimits| match validate_image(
        bytes, name, limits,
    ) {
        Err(CairnError::InvalidImage(msg)) => msg,
        other => panic!("expected rejection, got {other:?}"),
    };
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    assert!(reject(svg, Some("x.svg"), &limits).contains("SVG"));
    assert!(
        reject(b"MZ\x90\x00not a picture", Some("x.png"), &limits).contains("isn't a supported")
    );
    assert!(reject(&common::jpeg(4, 4), Some("fake.png"), &limits).contains("actually contains"));
    let mut truncated = common::png(50, 50, 2);
    truncated.truncate(truncated.len() / 2);
    assert!(reject(&truncated, None, &limits).contains("damaged"));
    assert!(reject(&[], None, &limits).contains("empty"));

    let tight = ImageLimits {
        max_bytes: 100,
        ..ImageLimits::default()
    };
    assert!(reject(&common::png(64, 64, 1), None, &tight).contains("too large"));
    let small = ImageLimits {
        max_dimension: 16,
        ..ImageLimits::default()
    };
    assert!(reject(&common::png(64, 8, 1), None, &small).contains("too big"));
}

// ------------------------------------------------------------ rendering

#[test]
fn markdown_features_render_and_scripts_never_do() {
    let ws = common::workspace();
    let md = "# Title\n\n## Steps\n\n- a\n- b\n\n1. one\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n\
              ```\ncode\n```\n\n<script>alert('x')</script>\n\n<img src=x onerror=alert(1)>\n\n\
              [bad](javascript:alert(1)) [web](https://example.com)\n";
    let r = rendered(&ws.root, "Guides/a.md", md, "KEY");
    for tag in [
        "<h1 id=\"title\">",
        "<h2 id=\"steps\">",
        "<ul>",
        "<ol>",
        "<table>",
        "<pre><code>",
    ] {
        assert!(r.html.contains(tag), "missing {tag} in {}", r.html);
    }
    assert!(
        !r.html.contains("<script"),
        "raw HTML is shown as text, never run"
    );
    assert!(
        !r.html.contains("<img"),
        "raw <img> with an event handler is only shown as text"
    );
    assert!(r.html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(!r.html.contains("javascript:"));
    assert!(r.html.contains("&lt;script&gt;"));
    assert!(r.html.contains("href=\"https://example.com\""));
    let toc: Vec<_> = r.toc.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(toc, ["Title", "Steps"]);
}

#[test]
fn remote_images_are_never_fetched() {
    let ws = common::workspace();
    let r = rendered(
        &ws.root,
        "Guides/a.md",
        "![tracker](https://example.com/pixel.png)\n",
        "K",
    );
    assert!(!r.html.contains("<img"), "{}", r.html);
    assert!(r.html.contains("External picture (not loaded)"));
}

#[test]
fn missing_local_targets_are_flagged_and_existing_ones_link_inside_the_app() {
    let ws = common::workspace();
    common::write(&ws.path.join("Guides/exists.md"), "# Exists\n");
    let md = "[ok](exists.md) [gone](missing.md) [up](../README.md) \
              [escape](../../../etc/passwd) ![pic](nope.png)\n";
    let r = rendered(&ws.root, "Guides/a.md", md, "K");
    assert!(r.html.contains("href=\"#/page/Guides/exists.md\""));
    assert!(r.html.contains("href=\"#/page/README.md\""));
    assert!(r.html.contains("class=\"broken-link\""));
    assert!(r.html.contains("Missing picture"));
    assert!(r.broken_links.contains(&"Guides/missing.md".to_string()));
    assert!(r.broken_links.contains(&"Guides/nope.png".to_string()));
    assert_eq!(r.broken_links.len(), 3, "{:?}", r.broken_links);
}

#[test]
fn invalid_metadata_never_makes_a_page_unreadable() {
    let text = "---\nowner: [unclosed\nstatus: whatever\n---\n# Still readable\n\nBody.\n";
    let (meta, body) = parse_front_matter(text);
    assert!(!meta.warnings.is_empty());
    assert!(body.contains("Still readable"));

    let unterminated = "---\nowner: Sam\n# No closing fence\n";
    let (meta, body) = parse_front_matter(unterminated);
    assert!(!meta.warnings.is_empty());
    assert_eq!(body, unterminated);
}

// ---------------------------------------------------------------- search

#[test]
fn search_finds_titles_and_bodies_with_highlighted_excerpts() {
    let ws = common::workspace();
    common::write(
        &ws.path.join("Guides/printer.md"),
        "# Printer setup\n\nHold the power button for ten seconds.\n",
    );
    common::write(
        &ws.path.join("Guides/email.md"),
        "# Email\n\nAsk about the printer queue.\n",
    );
    common::write(
        &ws.path.join("_system/history/secret.md"),
        "# printer secret\n",
    );
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();

    let hits = index.search("printer", 10);
    let paths: Vec<_> = hits.iter().map(|h| h.page.path.as_str()).collect();
    assert_eq!(
        paths,
        ["Guides/printer.md", "Guides/email.md"],
        "title first; _system never indexed"
    );
    assert!(
        hits[1]
            .excerpt
            .iter()
            .any(|s| s.hit && s.text.eq_ignore_ascii_case("printer"))
    );
    assert_eq!(index.search("power button", 10).len(), 1);
    assert!(index.search("nothing-matches-this", 10).is_empty());

    // Incremental refresh notices deletions.
    fs::remove_file(ws.path.join("Guides/email.md")).unwrap();
    index.refresh(&ws.root).unwrap();
    assert_eq!(index.search("printer", 10).len(), 1);
}
