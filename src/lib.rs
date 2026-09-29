//! Cairn: local-first shared Markdown documentation.
//!
//! Each user runs Cairn on their own computer. It serves a browser UI on the
//! loopback interface and reads and writes ordinary Markdown files in a
//! folder (local or on a network share) using that user's own permissions.

pub mod article;
pub mod config;
pub mod drafts;
pub mod error;
pub mod export;
pub mod fsutil;
pub mod history;
pub mod images;
pub mod links;
pub mod locks;
pub mod manage;
pub mod paths;
pub mod publish;
pub mod search;
pub mod server;
pub mod templates;
pub mod timefmt;
pub mod trash;
pub mod workspace;
