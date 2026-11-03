//! The `/ms` commands (`PluginCore.cs`, `ProcessMSCommand`), read the way
//! mudsort read them: the whole line lower-cased and trimmed, each command
//! matched by its text in mudsort's order (so `/ms startle` starts a sort,
//! as it did).

/// What `/ms help` lists, after `listing help / commands...`.
pub const HELP: [&str; 7] = [
    "/ms start - start sorting",
    "/ms stop - cancel sorting",
    "/ms set source (pack <1-8>|player|<containerID>) - set source container to: pack 1-8, player, given containerID, or selection (no argument)",
    "/ms set dest (pack <1-8>|player|<containerID>) - set destination to: pack 1-8, player, given containerID, or selection (no argument)",
    "/ms set flags (sort flag string) - sets the sort string to the given argument",
    "/ms set ocfilter (object class string) - sets the object class filter to the given argument",
    "/ms clear ocfilter - clears the object class filter",
];

/// The line before [`HELP`].
pub const HELP_HEADER: &str = "listing help / commands...";

/// The description the verb is registered with.
pub const DESCRIPTION: &str = "MudSort: sort items by a recipe of properties (/ms help)";

/// What `set source` or `set dest` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// No argument: the current selection.
    Selection,
    /// `player`: the character itself.
    Player,
    /// `pack N`: the character's pack in pack slot N - 1.
    Pack(i32),
    /// An object id.
    Id(u32),
    /// An argument that names nothing (`pack x`, `abc`): mudsort's
    /// `Int32.Parse` threw here.
    Invalid(String),
}

/// One `/ms` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `/ms`, `/ms help`.
    Help,
    /// `/ms clear ocfilter`, `/ms set ocfilter`.
    ClearClassFilter,
    /// `/ms set ocfilter <prefix>` (lower-cased, as mudsort kept it).
    SetClassFilter(String),
    /// `/ms set source ...`.
    SetSource(Target),
    /// `/ms set dest ...`.
    SetDest(Target),
    /// `/ms set flags <string>` (upper-cased, as mudsort did).
    SetFlags(String),
    /// `/ms start`.
    Start,
    /// `/ms stop`.
    Stop,
    /// Anything else after `/ms`.
    Unknown(String),
}

/// Reads an id as mudsort's `Int32.Parse` did (a signed decimal, spaces
/// around allowed), and also as an unsigned decimal or `0x` hexadecimal.
#[must_use]
pub fn parse_id(text: &str) -> Option<u32> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix("0x") {
        return u32::from_str_radix(hex, 16).ok();
    }
    text.parse::<i32>()
        .map(|id| id as u32)
        .ok()
        .or_else(|| text.parse::<u32>().ok())
}

fn target(argument: &str) -> Target {
    if argument.starts_with("player") {
        return Target::Player;
    }
    if let Some(number) = argument.strip_prefix("pack") {
        return number
            .trim()
            .parse::<i32>()
            .map_or_else(|_| Target::Invalid(argument.to_owned()), Target::Pack);
    }
    parse_id(argument).map_or_else(|| Target::Invalid(argument.to_owned()), Target::Id)
}

/// Reads `/ms` and what was typed after it.
#[must_use]
pub fn parse(arguments: &str) -> Command {
    let line = format!("/ms {arguments}").trim().to_lowercase();
    let line = line.as_str();
    if line.starts_with("/ms help") || line == "/ms" {
        return Command::Help;
    }
    if line == "/ms set ocfilter" || line == "/ms clear ocfilter" {
        return Command::ClearClassFilter;
    }
    if let Some(filter) = line.strip_prefix("/ms set ocfilter ") {
        return Command::SetClassFilter(filter.to_owned());
    }
    if line == "/ms set source" {
        return Command::SetSource(Target::Selection);
    }
    if line == "/ms set dest" {
        return Command::SetDest(Target::Selection);
    }
    if let Some(argument) = line.strip_prefix("/ms set source ") {
        return Command::SetSource(target(argument));
    }
    if let Some(argument) = line.strip_prefix("/ms set dest ") {
        return Command::SetDest(target(argument));
    }
    if let Some(flags) = line.strip_prefix("/ms set flags ") {
        return Command::SetFlags(flags.to_uppercase());
    }
    if line.starts_with("/ms start") {
        return Command::Start;
    }
    if line.starts_with("/ms stop") {
        return Command::Stop;
    }
    Command::Unknown(line.to_owned())
}
