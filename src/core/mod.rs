//! Everything that is not UI: downloads, adb, updates, OAuth, OS integration.

pub mod adb;
pub mod cache;
pub mod config;
pub mod download;
pub mod elevation;
pub mod error;
pub mod http;
pub mod launcher;
pub mod log;
pub mod manifest;
pub mod oauth;
pub mod paths;
pub mod pc_update;
pub mod platform;
pub mod process;
pub mod quest_install;
pub mod quest_update;
pub mod revive;
pub mod zip;
