//! The identification queue: Decal's `IDQueue` as mudsort used it
//! (`Core.IDQueue.AddToQueue`), over the client's appraisal requests.
//!
//! The client has one appraisal in flight at a time: a second request is
//! answered `Busy`, and is asked again next tick. A request that ends
//! without an appraisal (the client gave up on it, or the server could not
//! appraise the object) is asked again, up to [`MAX_ATTEMPTS`] times; then
//! the item goes on without one (`DIVERGENCES.md`, MS-8).

use std::collections::{HashMap, HashSet, VecDeque};

use openac_plugin::Host;
use openac_plugin::items::ItemStatus;

/// How many requests one object gets before it goes on unidentified.
pub const MAX_ATTEMPTS: u32 = 3;

/// How many queued objects a tick may find already identified (one object
/// lookup each) before it stops for that tick.
const SKIPS_PER_TICK: usize = 32;

/// How an object is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// Something the client knows (`identify`): an owned item.
    World,
    /// An item in the open container (`identify_loot`).
    Loot,
}

#[derive(Debug, Clone, Copy)]
struct Want {
    id: u32,
    via: Via,
    /// Already looked up and found without an appraisal.
    checked: bool,
}

/// The queue.
#[derive(Debug, Default)]
pub struct Identifier {
    queue: VecDeque<Want>,
    waiting: Option<Want>,
    attempts: HashMap<u32, u32>,
    given_up: HashSet<u32>,
}

impl Identifier {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn queued(&self, id: u32) -> bool {
        self.queue.iter().any(|want| want.id == id)
    }

    /// Adds an object at the end (login identification); nothing when it
    /// is already queued or awaited.
    pub fn push_back(&mut self, id: u32, via: Via) {
        if self.is_pending(id) {
            return;
        }
        self.queue.push_back(Want {
            id,
            via,
            checked: false,
        });
    }

    /// Puts objects at the front, in their order (a sort waits on them),
    /// known to have no appraisal; an object already queued moves up.
    pub fn push_front(&mut self, wants: &[(u32, Via)]) {
        self.queue
            .retain(|want| !wants.iter().any(|(id, _)| *id == want.id));
        for &(id, via) in wants.iter().rev() {
            if self.waiting.is_some_and(|want| want.id == id) {
                continue;
            }
            self.given_up.remove(&id);
            self.attempts.remove(&id);
            self.queue.push_front(Want {
                id,
                via,
                checked: true,
            });
        }
    }

    /// Whether the object is still to be identified: queued, or its
    /// request is in flight.
    #[must_use]
    pub fn is_pending(&self, id: u32) -> bool {
        self.waiting.is_some_and(|want| want.id == id) || self.queued(id)
    }

    /// The object whose appraisal is awaited.
    #[must_use]
    pub fn awaiting(&self) -> Option<u32> {
        self.waiting.map(|want| want.id)
    }

    /// Whether the object was given up on (it goes on unidentified).
    #[must_use]
    pub fn gave_up(&self, id: u32) -> bool {
        self.given_up.contains(&id)
    }

    /// How many objects are still to be identified.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len() + usize::from(self.waiting.is_some())
    }

    /// Nothing is still to be identified.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drops objects (a cancelled sort's): queued ones are not asked about,
    /// and one in flight is not asked about again (its request is left to
    /// finish; the client answers `Busy` until it does).
    pub fn remove(&mut self, ids: &[u32]) {
        self.queue.retain(|want| !ids.contains(&want.id));
        if self.waiting.is_some_and(|want| ids.contains(&want.id)) {
            self.waiting = None;
        }
    }

    /// Forgets everything (logoff).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// An appraisal arrived.
    pub fn received(&mut self, id: u32) {
        if self.waiting.is_some_and(|want| want.id == id) {
            self.waiting = None;
        }
        self.queue.retain(|want| want.id != id);
        self.attempts.remove(&id);
    }

    /// One tick: notices a request that ended without an appraisal, then
    /// asks about the next object when nothing is in flight.
    pub fn tick(&mut self, host: &mut Host<'_>) {
        if let Some(want) = self.waiting {
            if host.loot_state().appraisal.awaiting_id == want.id {
                return;
            }
            self.waiting = None;
            if !has_appraisal(host, want.id) {
                let attempts = self.attempts.entry(want.id).or_default();
                *attempts += 1;
                if *attempts < MAX_ATTEMPTS {
                    self.queue.push_front(Want {
                        checked: true,
                        ..want
                    });
                } else {
                    self.attempts.remove(&want.id);
                    self.given_up.insert(want.id);
                    host.info(&format!(
                        "No appraisal of {:#010X} after {MAX_ATTEMPTS} requests; it is sorted without one.",
                        want.id
                    ));
                }
            }
        }
        if self.waiting.is_some() {
            return;
        }
        let mut skips = 0;
        while let Some(want) = self.queue.front().copied() {
            if !want.checked {
                match host.object(want.id) {
                    Some(object) if !object.has_appraisal_data => {
                        if let Some(front) = self.queue.front_mut() {
                            front.checked = true;
                        }
                    }
                    // Gone, or already identified.
                    _ => {
                        self.queue.pop_front();
                        skips += 1;
                        if skips >= SKIPS_PER_TICK {
                            return;
                        }
                        continue;
                    }
                }
            }
            let result = match want.via {
                Via::World => host.identify(want.id),
                Via::Loot => host.identify_loot(want.id),
            };
            match result.status {
                ItemStatus::Started => {
                    self.queue.pop_front();
                    self.waiting = Some(Want {
                        checked: true,
                        ..want
                    });
                }
                // One appraisal in flight at a time, or not in the world:
                // ask again next tick.
                ItemStatus::Busy | ItemStatus::Unavailable => {}
                ItemStatus::Completed => {
                    self.queue.pop_front();
                }
                ItemStatus::InvalidItem | ItemStatus::InvalidTarget | ItemStatus::Refused => {
                    self.queue.pop_front();
                    self.given_up.insert(want.id);
                    host.info(&format!(
                        "{:#010X} cannot be identified ({:?}); it is sorted without an appraisal.",
                        want.id, result.status
                    ));
                }
            }
            return;
        }
    }
}

/// Whether the client holds an appraisal of the object.
#[must_use]
pub fn has_appraisal(host: &mut Host<'_>, id: u32) -> bool {
    host.object(id)
        .is_some_and(|object| object.has_appraisal_data)
}
