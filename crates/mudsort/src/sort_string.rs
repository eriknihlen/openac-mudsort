//! The sort string: comma-separated codes, each optionally followed by `-`
//! for descending (`OC,AL-,NM`), as mudsort reads and writes it
//! (`PluginCore.cs`, `createSortFlagListFromString` and
//! `sortFlagListToString`; `SortFlag.cs`, `decode`).

use crate::codes::{self, Flag};

/// The sort string a new character starts with: mudsort's default with its
/// dead codes replaced (`DIVERGENCES.md`, MS-1).
pub const DEFAULT: &str = "OC,EQ,CV,AL-,AB-,DB-,ED-,ML,MT,NM";

/// mudsort's own default (`Properties/Settings.settings`,
/// `DefaultSortString`). `DG` and `MA` match no key, and `ES` is where an
/// item is worn now, which is 0 for anything in a pack.
pub const MUDSORT_DEFAULT: &str = "OC,ES,CV,AL-,AB-,DG-,ED-,ML,MA,NM";

/// One key of a sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    /// The key.
    pub flag: &'static Flag,
    /// Largest first.
    pub descending: bool,
}

/// A parsed sort string.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SortString {
    /// The keys, first key first.
    pub keys: Vec<SortKey>,
    /// The parts that named no key, as written (mudsort skipped them and
    /// logged an error).
    pub unknown: Vec<String>,
}

/// Reads a sort string. Each comma-separated part names the key whose code
/// is its first two characters (in any case); it is descending when it is
/// exactly three characters long and the third is `-` (so `AL- ` and
/// `AL-X` are ascending, as in mudsort). A part that names no key is
/// skipped and reported; an empty part is skipped silently.
///
/// mudsort kept one direction per key, not per part: a key named twice
/// sorts both times in the direction of its last part (`AL,AL-` is two
/// descending passes).
#[must_use]
pub fn parse(text: &str) -> SortString {
    let mut parsed = SortString::default();
    for part in text.split(',') {
        let chars: Vec<char> = part.chars().collect();
        let flag = (chars.len() >= 2)
            .then(|| chars[..2].iter().collect::<String>().to_uppercase())
            .and_then(|code| codes::by_code(&code));
        match flag {
            Some(flag) => {
                let descending = chars.len() == 3 && chars[2] == '-';
                for key in &mut parsed.keys {
                    if key.flag == flag {
                        key.descending = descending;
                    }
                }
                parsed.keys.push(SortKey { flag, descending });
            }
            None if part.is_empty() => {}
            None => parsed.unknown.push(part.to_owned()),
        }
    }
    parsed
}

/// Writes the keys as a sort string.
#[must_use]
pub fn serialize(keys: &[SortKey]) -> String {
    keys.iter()
        .map(|key| {
            format!(
                "{}{}",
                key.flag.code(),
                if key.descending { "-" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}
