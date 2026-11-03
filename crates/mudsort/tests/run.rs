//! The sort run on a simulated client (`PluginCore.cs`, `activate`,
//! `cancel`, `Current_RenderFrame_Sort`).

mod sim;

use ac_plugin_abi::items::{ItemResult, ItemStatus};
use mudsort::run::{Phase, RowState};
use openac_plugin::Event;
use openac_plugin::world::ObjectClass;
use sim::{CHEST, HORAN, Harness, ME, NAME, PACK1, Req, Sim};

const C: u32 = 0x8000_2001;
const A: u32 = 0x8000_2002;
const B: u32 = 0x8000_2003;

/// Three gems in the main pack, armor levels 10, 30 and 20; two without an
/// appraisal. Sorted by armor level, largest first: A, B, C.
fn three_gems(sim: &mut Sim) {
    sim.add_item(C, "Gem C", ObjectClass::Gem, ME, 10, false);
    sim.add_item(A, "Gem A", ObjectClass::Gem, ME, 30, true);
    sim.add_item(B, "Gem B", ObjectClass::Gem, ME, 20, false);
}

/// Logged in with three gems, sorting by `AL-` into pack 1.
fn ready() -> Harness {
    let mut h = Harness::logged_in(three_gems);
    h.ms("set flags al-");
    h.ms("set dest pack 1");
    h
}

fn chat_has(h: &Harness, line: &str) -> bool {
    h.chat().iter().any(|l| l == line)
}

#[test]
fn a_run_identifies_one_at_a_time_then_moves_in_order_to_the_same_slot() {
    let mut h = ready();
    h.ms("start");
    assert!(chat_has(&h, "[mudsort]: Starting Sort..."));
    assert!(chat_has(&h, "[mudsort]: 3 items added to sort list..."));
    assert_eq!(h.plugin.sorter().phase(), Phase::Identifying);

    // One appraisal at a time: nothing more is asked while it is awaited.
    for _ in 0..3 {
        h.tick();
    }
    assert_eq!(h.sim.identifies(), vec![C]);
    let progress = h.plugin.progress();
    assert_eq!((progress.done, progress.total), (1, 3));
    assert_eq!(progress.current.map(|c| c.id), Some(C));
    let event = h.sim.answer_appraisal().unwrap();
    h.send(event);
    h.tick();
    assert_eq!(h.sim.identifies(), vec![C, B]);
    let event = h.sim.answer_appraisal().unwrap();
    h.send(event);

    // Built, then the first move; the next waits for its answer.
    h.tick();
    h.tick();
    assert!(chat_has(&h, "[mudsort]: 3 items in queue..."));
    assert_eq!(h.plugin.sorter().phase(), Phase::Moving);
    for _ in 0..3 {
        h.tick();
    }
    assert_eq!(
        h.sim.moves(),
        vec![Req::Move {
            id: A,
            container: PACK1,
            slot: 0
        }]
    );
    let rows = h.plugin.rows();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![A, B, C]);
    assert_eq!(rows[0].state, RowState::Moving);
    assert_eq!(rows[0].values, vec!["30".to_owned()]);

    h.run(20);
    assert_eq!(
        h.sim.moves(),
        [A, B, C]
            .map(|id| Req::Move {
                id,
                container: PACK1,
                slot: 0
            })
            .to_vec()
    );
    // Every item went in at slot 0, so the pack holds them reversed, as
    // mudsort left them.
    assert_eq!(h.sim.contents(PACK1), vec!["Gem C", "Gem B", "Gem A"]);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    assert!(h.plugin.rows().iter().all(|r| r.state == RowState::Moved));
    assert_eq!(
        h.sim.fake.submitted,
        vec![format!("/tell {NAME}, done sorting!")]
    );
    assert_eq!(h.chat().last().unwrap(), "[mudsort]: done sorting items!");
    let progress = h.plugin.progress();
    assert_eq!((progress.done, progress.total), (3, 3));
}

#[test]
fn no_tell_when_announcing_is_off() {
    let mut h = ready();
    h.with_host(|p, host| p.update_settings(host, |s| s.announce_when_done = false));
    h.ms("start");
    h.run(40);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    assert!(h.sim.fake.submitted.is_empty());
    assert!(chat_has(&h, "[mudsort]: done sorting items!"));
}

#[test]
fn the_start_slot_is_the_slot_of_every_move() {
    let mut h = ready();
    h.with_host(|p, host| p.update_settings(host, |s| s.start_slot = 2));
    h.ms("start");
    h.run(40);
    let moves = h.sim.moves();
    assert_eq!(moves.len(), 3);
    assert!(moves.iter().all(|m| matches!(m, Req::Move { slot: 2, .. })));
}

#[test]
fn reverse_turns_the_whole_list() {
    let mut h = ready();
    h.with_host(|p, host| p.update_settings(host, |s| s.reverse = true));
    h.ms("start");
    h.run(40);
    let ids: Vec<u32> = h.sim.moves().iter().map(Req::id).collect();
    assert_eq!(ids, vec![C, B, A]);
}

#[test]
fn a_busy_answer_is_asked_again_next_tick() {
    let mut h = ready();
    h.ms("start");
    h.sim
        .move_answers
        .push_back(ItemResult::of(ItemStatus::Busy));
    h.run(40);
    let ids: Vec<u32> = h.sim.moves().iter().map(Req::id).collect();
    assert_eq!(ids, vec![A, A, B, C]);
    assert!(h.plugin.rows().iter().all(|r| r.state == RowState::Moved));
}

#[test]
fn a_row_busy_too_long_is_given_up_and_the_run_goes_on() {
    let mut h = ready();
    h.ms("start");
    for _ in 0..=mudsort::run::MAX_BUSY_ANSWERS {
        h.sim
            .move_answers
            .push_back(ItemResult::of(ItemStatus::Busy));
    }
    h.run(200);
    let rows = h.plugin.rows();
    assert_eq!(rows[0].state, RowState::Failed);
    assert_eq!(rows[0].note.as_deref(), Some("the client stayed busy"));
    assert_eq!(rows[1].state, RowState::Moved);
    assert_eq!(rows[2].state, RowState::Moved);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
}

#[test]
fn a_refusal_marks_the_row_failed_and_the_run_goes_on() {
    let mut h = ready();
    h.ms("start");
    h.sim
        .move_answers
        .push_back(ItemResult::because(ItemStatus::Refused, "It is attuned."));
    h.run(40);
    let rows = h.plugin.rows();
    assert_eq!(rows[0].state, RowState::Failed);
    assert_eq!(rows[0].note.as_deref(), Some("It is attuned."));
    assert_eq!(rows[1].state, RowState::Moved);
    assert_eq!(rows[2].state, RowState::Moved);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    let progress = h.plugin.progress();
    assert_eq!((progress.done, progress.total), (3, 3));
}

#[test]
fn a_run_with_failed_rows_says_how_many_and_the_first_reason() {
    let mut h = ready();
    h.ms("start");
    h.sim
        .move_answers
        .push_back(ItemResult::because(ItemStatus::Refused, "It is attuned."));
    h.sim
        .move_answers
        .push_back(ItemResult::because(ItemStatus::Refused, "It is too heavy."));
    h.run(40);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    // mudsort's own lines as they were, then the count and the first
    // row's reason.
    assert_eq!(
        h.sim.fake.submitted,
        vec![format!("/tell {NAME}, done sorting!")]
    );
    let chat = h.chat();
    assert_eq!(
        chat[chat.len() - 2..],
        [
            "[mudsort]: done sorting items!",
            "[mudsort]: 2 items could not be moved: It is attuned."
        ]
    );

    // One failed row: "item".
    let mut h = ready();
    h.ms("start");
    h.sim
        .move_answers
        .push_back(ItemResult::because(ItemStatus::Refused, "It is attuned."));
    h.run(40);
    assert_eq!(
        h.chat().last().unwrap(),
        "[mudsort]: 1 item could not be moved: It is attuned."
    );
}

/// Sorting the three gems into the open chest.
fn into_the_chest() -> Harness {
    let mut h = ready();
    h.sim.open_chest();
    h.ms(&format!("set dest {CHEST}"));
    h.ms("start");
    h
}

#[test]
fn the_chest_closing_mid_run_stops_the_run_with_one_line() {
    let mut h = into_the_chest();
    until_a_move_is_in_flight(&mut h);
    // A goes in; then the player walks away and the chest closes before
    // the next put, which the client refuses.
    let event = h.sim.answer_move(0).unwrap();
    h.sim.close_chest();
    h.send(event);
    h.run(40);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    // B's put was refused and C was never asked for.
    assert_eq!(
        h.sim.moves(),
        [A, B]
            .map(|id| Req::Put {
                id,
                chest: CHEST,
                slot: 0
            })
            .to_vec()
    );
    let rows = h.plugin.rows();
    assert_eq!(rows[0].state, RowState::Moved);
    for row in &rows[1..] {
        assert_eq!(row.state, RowState::Failed);
        assert_eq!(row.note.as_deref(), Some("the chest was closed"));
    }
    let chat = h.chat();
    assert_eq!(
        chat[chat.len() - 2..],
        [
            "[mudsort]: done sorting items!",
            "[mudsort]: 2 items could not be moved: the chest was closed"
        ]
    );
}

#[test]
fn a_put_refused_while_the_chest_is_still_open_fails_only_its_row() {
    let mut h = into_the_chest();
    h.sim
        .move_answers
        .push_back(ItemResult::of(ItemStatus::InvalidTarget));
    h.run(40);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    let rows = h.plugin.rows();
    assert_eq!(rows[0].state, RowState::Failed);
    assert_eq!(rows[0].note.as_deref(), Some("InvalidTarget"));
    assert_eq!(rows[1].state, RowState::Moved);
    assert_eq!(rows[2].state, RowState::Moved);
    assert_eq!(h.sim.contents(CHEST), vec!["Gem C", "Gem B"]);
}

#[test]
fn a_move_the_server_refuses_fails_its_row() {
    let mut h = ready();
    h.ms("start");
    // Identify both, build the list, send the first move.
    for _ in 0..12 {
        h.tick();
        if let Some(event) = h.sim.answer_appraisal() {
            h.send(event);
        }
        if h.sim.in_flight.is_some() {
            break;
        }
    }
    let event = h.sim.answer_move(0x0029).unwrap();
    h.send(event);
    h.run(40);
    let rows = h.plugin.rows();
    assert_eq!(rows[0].state, RowState::Failed);
    assert_eq!(
        rows[0].note.as_deref(),
        Some("the server refused it (error 0x29)")
    );
    assert_eq!(h.sim.contents(PACK1), vec!["Gem C", "Gem B"]);
}

#[test]
fn stopping_mid_run_asks_nothing_more() {
    let mut h = ready();
    h.ms("start");
    for _ in 0..40 {
        h.tick();
        if let Some(event) = h.sim.answer_appraisal() {
            h.send(event);
        }
        if h.sim.moves().len() == 1 {
            break;
        }
    }
    h.ms("stop");
    assert!(chat_has(&h, "[mudsort]: Stopping Sort..."));
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    for _ in 0..20 {
        h.tick();
        if let Some(event) = h.sim.answer_move(0) {
            h.send(event);
        }
    }
    assert_eq!(h.sim.moves().len(), 1);
    assert!(h.sim.fake.submitted.is_empty());
    assert_eq!(h.plugin.progress().total, 0);
}

#[test]
fn stopping_while_identifying_drops_the_queued_items() {
    // Without the login identification, nothing else wants them.
    let mut h = Harness::logged_in(|sim| {
        three_gems(sim);
        let settings = mudsort::settings::Settings {
            identify_on_login: false,
            ..mudsort::settings::Settings::default()
        };
        sim.storage.insert(
            format!("{NAME}/{}", mudsort::settings::SETTINGS_KEY),
            settings.to_json(),
        );
    });
    h.ms("set flags al-");
    h.ms("set dest pack 1");
    h.ms("start");
    h.tick();
    assert_eq!(h.sim.identifies(), vec![C]);
    h.ms("stop");
    h.sim.drop_appraisal();
    for _ in 0..10 {
        h.tick();
    }
    // C's request was in flight and is not asked again; B is not asked.
    assert_eq!(h.sim.identifies(), vec![C]);
    assert!(h.sim.moves().is_empty());
}

#[test]
fn logging_off_cancels_the_run() {
    let mut h = ready();
    h.ms("start");
    h.tick();
    h.send(Event::Logoff);
    assert!(chat_has(&h, "[mudsort]: Plugin offline."));
    let asked = h.sim.log.len();
    for _ in 0..20 {
        h.tick();
        if let Some(event) = h.sim.answer_appraisal() {
            h.send(event);
        }
    }
    assert_eq!(h.sim.log.len(), asked);
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
}

#[test]
fn an_item_never_appraised_is_sorted_after_three_requests() {
    let mut h = Harness::logged_in(|sim| {
        sim.add_item(C, "Gem C", ObjectClass::Gem, ME, 10, false);
    });
    h.ms("set flags al-");
    h.ms("set dest pack 1");
    h.ms("start");
    for _ in 0..3 {
        h.tick();
        h.sim.drop_appraisal();
    }
    h.tick();
    assert_eq!(h.sim.identifies(), vec![C, C, C]);
    let row = &h.plugin.rows()[0];
    assert_eq!(row.state, RowState::Ready);
    assert_eq!(row.note.as_deref(), Some("could not be identified"));
    h.run(20);
    assert_eq!(h.sim.moves().len(), 1);
}

#[test]
fn from_the_chest_each_item_is_picked_up_then_put_at_the_slot() {
    let mut h = Harness::logged_in(|sim| {
        sim.open_chest();
        sim.add_item(C, "Gem C", ObjectClass::Gem, CHEST, 10, false);
        sim.add_item(A, "Gem A", ObjectClass::Gem, CHEST, 30, true);
    });
    h.ms("set flags al-");
    h.ms(&format!("set source {CHEST}"));
    h.ms("set dest pack 1");
    h.ms("start");
    assert!(chat_has(&h, "[mudsort]: 2 items added to sort list..."));
    h.run(40);
    assert_eq!(h.sim.identifies(), vec![C]);
    assert_eq!(h.sim.log[0], Req::IdentifyLoot(C));
    assert_eq!(
        h.sim.moves(),
        vec![
            Req::PickUp {
                id: A,
                main_pack: true
            },
            Req::Move {
                id: A,
                container: PACK1,
                slot: 0
            },
            Req::PickUp {
                id: C,
                main_pack: true
            },
            Req::Move {
                id: C,
                container: PACK1,
                slot: 0
            },
        ]
    );
    assert_eq!(h.sim.contents(PACK1), vec!["Gem C", "Gem A"]);
    assert!(h.sim.contents(CHEST).is_empty());
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
}

#[test]
fn into_the_chest_items_are_put_and_a_put_already_done_counts_as_moved() {
    let mut h = ready();
    h.sim.open_chest();
    h.ms(&format!("set dest {CHEST}"));
    h.ms("start");
    h.sim
        .move_answers
        .push_back(ItemResult::of(ItemStatus::Completed));
    h.run(40);
    assert_eq!(
        h.sim.moves(),
        [A, B, C]
            .map(|id| Req::Put {
                id,
                chest: CHEST,
                slot: 0
            })
            .to_vec()
    );
    assert!(h.plugin.rows().iter().all(|r| r.state == RowState::Moved));
    assert_eq!(h.sim.contents(CHEST), vec!["Gem C", "Gem B"]);
}

#[test]
fn to_a_player_items_are_given() {
    let mut h = ready();
    h.sim.selected = Some(HORAN);
    h.ms("set dest");
    assert_eq!(h.plugin.dest(), HORAN);
    h.ms("start");
    h.run(40);
    assert_eq!(
        h.sim.moves(),
        [A, B, C].map(|id| Req::Give { id, to: HORAN }).to_vec()
    );
    assert!(h.sim.contents(ME).is_empty());
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
}

#[test]
fn without_keys_nothing_is_queued() {
    let mut h = Harness::logged_in(three_gems);
    h.ms("set flags zz");
    h.ms("start");
    h.run(40);
    assert!(chat_has(&h, "[mudsort]: 3 items added to sort list..."));
    assert!(chat_has(&h, "[mudsort]: 0 items in queue..."));
    assert!(h.sim.moves().is_empty());
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
}

#[test]
fn an_invalid_source_does_not_start() {
    let mut h = ready();
    h.ms("set source 12345");
    assert_eq!(h.plugin.source(), 0);
    h.ms("start");
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    assert!(
        h.chat()
            .last()
            .unwrap()
            .starts_with("[mudsort]: cannot sort: the source")
    );
}

#[test]
fn the_preview_lists_the_order_known_so_far_without_starting() {
    let mut h = ready();
    let rows = h.with_host(|p, host| p.preview(host)).unwrap();
    assert_eq!(
        rows.iter().map(|r| (r.id, r.state)).collect::<Vec<_>>(),
        vec![
            (A, RowState::Ready),
            (B, RowState::NeedsId),
            (C, RowState::NeedsId)
        ]
    );
    assert_eq!(h.plugin.columns(), vec!["Armor level"]);
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    assert!(h.sim.log.is_empty());
}

#[test]
fn the_class_and_name_filters_choose_the_items() {
    let mut h = Harness::logged_in(|sim| {
        three_gems(sim);
        sim.add_item(0x8000_3001, "Breastplate", ObjectClass::Armor, ME, 50, true);
    });
    h.ms("set flags al-");
    h.ms("set ocfilter ge");
    h.with_host(|p, host| p.update_settings(host, |s| s.name_filter = "gem b".to_owned()));
    h.ms("start");
    assert!(chat_has(&h, "[mudsort]: 1 items added to sort list..."));
    h.ms("stop");
    h.with_host(|p, host| p.update_settings(host, |s| s.name_filter.clear()));
    h.ms("start");
    assert!(chat_has(&h, "[mudsort]: 3 items added to sort list..."));
    h.ms("clear ocfilter");
    h.ms("start");
    assert!(chat_has(&h, "[mudsort]: 4 items added to sort list..."));
}

/// Ticks (answering appraisals) until the run's first move is in flight.
fn until_a_move_is_in_flight(h: &mut Harness) {
    for _ in 0..40 {
        h.tick();
        if let Some(event) = h.sim.answer_appraisal() {
            h.send(event);
        }
        if h.sim.in_flight.is_some() {
            return;
        }
    }
    panic!("no move went out");
}

#[test]
fn a_move_left_over_from_a_stopped_run_is_not_taken_for_the_next_runs() {
    let mut h = ready();
    h.ms("start");
    until_a_move_is_in_flight(&mut h);
    assert_eq!(
        h.sim.in_flight,
        Some(Req::Move {
            id: A,
            container: PACK1,
            slot: 0
        })
    );
    h.ms("stop");
    h.ms("start");
    // The new run waits for the client, which is busy with the old move.
    for _ in 0..5 {
        h.tick();
    }
    // The old move of A finishes; the new run then moves A itself.
    let event = h.sim.answer_move(0).unwrap();
    h.send(event);
    h.tick();
    h.tick();
    assert_eq!(h.sim.moves().iter().filter(|m| m.id() == A).count(), 2);
    assert!(h.sim.in_flight.is_some());
    // Its answer has not come: the row is still moving.
    assert_eq!(h.plugin.rows()[0].id, A);
    assert_eq!(h.plugin.rows()[0].state, RowState::Moving);
    h.run(40);
    assert!(h.plugin.rows().iter().all(|r| r.state == RowState::Moved));
}

#[test]
fn another_items_completion_does_not_answer_the_move_in_flight() {
    let mut h = ready();
    h.ms("start");
    until_a_move_is_in_flight(&mut h);
    h.sim.foreign_completion(0x8000_9999, 0x0029);
    h.tick();
    h.tick();
    assert_eq!(h.plugin.rows()[0].state, RowState::Moving);
    let event = h.sim.answer_move(0).unwrap();
    h.send(event);
    h.run(40);
    assert!(h.plugin.rows().iter().all(|r| r.state == RowState::Moved));
}

#[test]
fn a_move_answered_unavailable_is_asked_again() {
    let mut h = ready();
    h.ms("start");
    for _ in 0..5 {
        h.sim
            .move_answers
            .push_back(ItemResult::of(ItemStatus::Unavailable));
    }
    h.run(60);
    let ids: Vec<u32> = h.sim.moves().iter().map(Req::id).collect();
    assert_eq!(ids, vec![A, A, A, A, A, A, B, C]);
    assert!(h.plugin.rows().iter().all(|r| r.state == RowState::Moved));
}

#[test]
fn a_move_unavailable_too_long_is_given_up() {
    let mut h = ready();
    h.ms("start");
    for _ in 0..=mudsort::run::MAX_BUSY_ANSWERS {
        h.sim
            .move_answers
            .push_back(ItemResult::of(ItemStatus::Unavailable));
    }
    h.run(200);
    let rows = h.plugin.rows();
    assert_eq!(rows[0].state, RowState::Failed);
    assert_eq!(
        rows[0].note.as_deref(),
        Some("the client was not available")
    );
    assert_eq!(rows[1].state, RowState::Moved);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
}

#[test]
fn an_identify_answered_unavailable_counts_as_an_attempt() {
    let mut h = ready();
    h.ms("start");
    for _ in 0..mudsort::identify::MAX_ATTEMPTS {
        h.sim.identify_answers.push_back(ItemStatus::Unavailable);
    }
    h.run(60);
    assert_eq!(&h.sim.identifies()[..3], &[C, C, C]);
    // Given up after three: sorted without an appraisal, not asked again.
    assert!(!h.sim.identifies()[3..].contains(&C));
    assert!(
        h.sim
            .fake
            .logs
            .iter()
            .any(|(_, line)| line.contains("the client was not available"))
    );
    let c = h.plugin.rows().into_iter().find(|r| r.id == C).unwrap();
    assert_eq!(c.state, RowState::Moved);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
}

#[test]
fn a_chest_item_that_joins_a_stack_on_pick_up_counts_as_moved() {
    let mut h = Harness::logged_in(|sim| {
        sim.open_chest();
        sim.add_item(C, "Gem C", ObjectClass::Gem, CHEST, 10, true);
        sim.add_item(A, "Gem A", ObjectClass::Gem, CHEST, 30, true);
        sim.merge_on_pickup.push(A);
    });
    h.ms("set flags al-");
    h.ms(&format!("set source {CHEST}"));
    h.ms("set dest pack 1");
    h.ms("start");
    h.run(40);
    let rows = h.plugin.rows();
    assert_eq!(rows[0].id, A);
    assert_eq!(rows[0].state, RowState::Moved);
    assert_eq!(rows[0].note.as_deref(), Some("joined a stack"));
    assert_eq!(rows[1].state, RowState::Moved);
    assert_eq!(h.sim.contents(PACK1), vec!["Gem C"]);
}

#[test]
fn logging_off_and_on_forgets_a_finished_run() {
    let mut h = ready();
    h.ms("start");
    h.run(40);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    h.send(Event::Logoff);
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    assert!(h.plugin.rows().is_empty());
    h.send(Event::LoginComplete);
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    assert!(h.plugin.rows().is_empty());
    assert_eq!(h.plugin.progress().total, 0);
}

#[test]
fn stopping_gives_the_login_identification_its_items_back() {
    // Logged in with identification on: the whole inventory is queued, B
    // among it. The sort takes C and B to the front; stopping it while C
    // is asked about puts B back, so the login identification still gets
    // to it.
    let mut h = ready();
    h.ms("start");
    h.tick();
    assert_eq!(h.sim.identifies(), vec![C]);
    h.ms("stop");
    h.sim.drop_appraisal();
    for _ in 0..20 {
        h.tick();
        if let Some(event) = h.sim.answer_appraisal() {
            h.send(event);
        }
    }
    assert!(h.sim.identifies().contains(&B), "{:?}", h.sim.identifies());
    assert!(h.sim.moves().is_empty());
}
