//! mudsort's chat lines (`Util.WriteToChat`): `[mudsort]: ` and the text,
//! shown locally in Decal's chat colour 5.

use openac_plugin::Host;
use openac_plugin::chat_markup::escape;

/// What starts every line (`"[" + Globals.PluginName + "]: "`).
pub const PREFIX: &str = "[mudsort]: ";

/// The text class of the lines: Decal's `AddChatText` colour 5, which the
/// client takes as the text class (MossTank's port of UtilityBelt's chat,
/// `vendor_host.rs`, passes Decal's colour through the same way).
pub const TEXT_TYPE: u32 = 5;

/// Shows `[mudsort]: <text>` in chat. The text may hold what the player
/// typed, so it is escaped.
pub fn say(host: &mut Host<'_>, text: &str) {
    host.chat_post_with(&escape(&format!("{PREFIX}{text}")), TEXT_TYPE);
}
