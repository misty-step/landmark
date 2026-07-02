use chrono::Utc;
use clap::{Args, CommandFactory, Parser, Subcommand};
use hmac::{Hmac, Mac};
use pulldown_cmark::{Options, Parser as MarkdownParser, html};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[cfg(test)]
mod classification_tests;
mod cli;
mod describe;
mod errors;
mod manifest;
mod providers;
mod release_classification;
mod release_kit;
mod release_ops;
mod replay;
mod self_release;
mod setup_fleet;
mod synthesis;
#[cfg(test)]
mod tests;
mod util;

pub(crate) use cli::*;
pub(crate) use describe::*;
pub(crate) use errors::*;
pub(crate) use manifest::*;
pub(crate) use providers::*;
pub(crate) use release_classification::*;
pub(crate) use release_ops::*;
pub(crate) use replay::*;
pub(crate) use self_release::*;
pub(crate) use setup_fleet::*;
pub(crate) use synthesis::*;
pub(crate) use util::*;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) const VALID_NOTES: &str = "## Improvements\n\n- Added a replay harness that checks release behavior in a disposable repo.\n- Captured release body updates, artifacts, tags, and structured logs.\n- Kept the run local so no production secrets or GitHub releases are touched.\n";
pub(crate) const INVALID_NOTES: &str = "hello, here are the release notes";

fn main() {
    let cli = Cli::parse();
    let error_format = cli.error_format.clone();
    if let Err(error) = cli::run(cli) {
        if error_format == "json" {
            eprintln!("{}", structured_error_json(&error.to_string()));
        } else {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}
