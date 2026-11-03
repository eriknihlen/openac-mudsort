//! MudSort for openAC-rust: mudzereli's mudsort, a Decal inventory sorter,
//! as a WebAssembly plugin. It sorts the items of one container by a recipe
//! of item properties and moves them, in that order, into another.
//!
//! This crate is the plugin: a `cdylib` built for `wasm32-unknown-unknown`
//! with the openAC plugin SDK, and an `rlib` the native tests link.
//! [`MudSort`] does nothing yet.

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
