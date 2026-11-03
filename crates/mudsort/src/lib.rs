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
//! The run (identify, move, give), the commands and the window come next;
//! [`MudSort`] does nothing yet.

pub mod alphanum;
pub mod codes;
pub mod computed;
pub mod keys;
pub mod select;
pub mod sort;
pub mod sort_string;

use openac_plugin::{Event, Host, Plugin, StartInfo};

/// The plugin as the client loads it.
#[derive(Debug, Default)]
pub struct MudSort;

impl Plugin for MudSort {
    fn enable(&mut self, _host: &mut Host<'_>, _info: &StartInfo) -> Result<(), String> {
        Ok(())
    }

    fn on_event(&mut self, _host: &mut Host<'_>, _event: Event) {}
}

openac_plugin::export_plugin!(MudSort);
