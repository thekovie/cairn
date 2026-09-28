//! Shared helpers for integration tests.
#![allow(dead_code)]

use std::io::Cursor;
use std::path::{Path, PathBuf};

use cairn::locks::Identity;
use cairn::paths::Root;
use cairn::workspace::{InitTarget, initialize};

pub struct TestWorkspace {
    pub _dir: tempfile::TempDir,
    pub path: PathBuf,
    pub root: Root,
}

/// A fresh, initialized workspace in a temp folder.
pub fn workspace() -> TestWorkspace {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = initialize(
        &InitTarget::NewChild {
            parent: dir.path().to_path_buf(),
            name: "Docs".into(),
        },
        "Test Docs",
    )
    .expect("init");
    let path = PathBuf::from(&out.root);
    let root = Root::new(&path).expect("root");
    TestWorkspace {
        _dir: dir,
        path,
        root,
    }
}

/// A distinct client identity (as if on another computer).
pub fn identity(name: &str) -> Identity {
    Identity {
        session_id: uuid::Uuid::new_v4().to_string(),
        display_name: name.into(),
        os_user: name.to_lowercase(),
        host: format!("{}-PC", name.to_uppercase()),
    }
}

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, text).unwrap();
}

/// A small valid PNG.
pub fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([shade, 80, 200]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

pub fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([10, 200, 30]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
    out.into_inner()
}
