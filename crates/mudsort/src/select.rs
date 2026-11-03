//! Which items a sort takes (`PluginCore.cs`, `activate` and
//! `addWorldObject` without recursion): the items directly in the source
//! container that are not worn or wielded, have a place in it, are not a
//! focus, and match the class filter; packs are skipped, not opened.

use openac_plugin::world::ObjectClass;

use crate::computed::class_name;
use crate::keys::{ItemView, key};

/// The items of `items` a sort from `source` takes, in their order.
///
/// `class_filter` keeps only the classes whose name starts with it, ignoring
/// case ("melee" keeps `MeleeWeapon`); `None` or an empty filter keeps every
/// class.
#[must_use]
pub fn select<'a, T: ItemView>(
    items: &'a [T],
    source: u32,
    class_filter: Option<&str>,
) -> Vec<&'a T> {
    let filter = class_filter.map(str::to_lowercase);
    items
        .iter()
        .filter(|item| {
            item.container() == source
                && item.long(key::EQUIPPED_SLOTS) == 0
                && item.long(key::SLOT) != -1
                && item.object_class() != ObjectClass::Foci
                && filter.as_deref().is_none_or(|filter| {
                    class_name(item.object_class())
                        .to_lowercase()
                        .starts_with(filter)
                })
                && item.object_class() != ObjectClass::Container
        })
        .collect()
}
