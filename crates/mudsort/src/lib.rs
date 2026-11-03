//! MudSort for openAC-rust: mudzereli's mudsort, a Decal inventory sorter,
//! as a WebAssembly plugin. It sorts the items of one container by a recipe
//! of item properties and moves them, in that order, into another.
//!
//! This crate is the plugin: a `cdylib` built for `wasm32-unknown-unknown`
//! with the openAC plugin SDK, and an `rlib` the native tests link. The
//! sort core is pure and host-free:
//!
//! - [`codes`]: the sort keys and their two-letter codes;
//! - [`sort_string`]: reading and writing a sort string (`OC,AL-,NM`);
//! - [`alphanum`]: mudsort's string order;
//! - [`keys`]: how an item's values are read, as Decal read them;
//! - [`computed`]: the computed keys (buffed values, totals);
//! - [`sort`]: the sort itself;
//! - [`select`]: which items a sort takes.
//!
//! Over the client:
//!
//! - [`run`]: the sort run (identify, build the list, move one at a time),
//!   with its progress and rows as plain data for the window;
//! - [`identify`]: the identification queue;
//! - [`commands`]: the `/ms` commands;
//! - [`settings`]: the per-character settings and presets;
//! - [`inspect`]: an item's value for every key (the Inspector);
//! - [`plugin`]: [`MudSort`], the plugin, and what the window binds to.

pub mod alphanum;
pub mod chat;
pub mod codes;
pub mod commands;
pub mod computed;
pub mod identify;
pub mod inspect;
pub mod keys;
pub mod plugin;
pub mod run;
pub mod select;
pub mod settings;
pub mod sort;
pub mod sort_string;

pub use plugin::MudSort;

openac_plugin::export_plugin!(MudSort);
