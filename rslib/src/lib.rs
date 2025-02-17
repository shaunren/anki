// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

#![deny(unused_must_use)]

pub mod adding;
pub mod backend;
mod backend_proto;
pub mod browser_rows;
pub mod card;
pub mod cloze;
pub mod collection;
pub mod config;
pub mod dbcheck;
pub mod deckconf;
pub mod decks;
pub mod err;
pub mod findreplace;
mod fluent_proto;
pub mod i18n;
pub mod latex;
pub mod log;
mod markdown;
pub mod media;
pub mod notes;
pub mod notetype;
pub mod ops;
mod preferences;
pub mod prelude;
pub mod revlog;
pub mod scheduler;
pub mod search;
pub mod serde;
mod stats;
pub mod storage;
mod sync;
pub mod tags;
pub mod template;
pub mod template_filters;
pub mod text;
pub mod timestamp;
pub mod types;
pub mod undo;
pub mod version;
