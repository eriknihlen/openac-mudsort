//! The settings (`Properties/Settings.settings`), kept per character, and
//! identification at login (`CharacterFilter_LoginComplete`).

mod sim;

use mudsort::settings::{MUDSORT_SAVED, Preset, SETTINGS_KEY, Settings};
use mudsort::sort_string;
use openac_plugin::world::ObjectClass;
use sim::{Harness, ME, NAME, PACK1};

fn stored(h: &Harness) -> Option<&String> {
    h.sim.storage.get(&format!("{NAME}/{SETTINGS_KEY}"))
}

#[test]
fn the_defaults_are_mudsorts() {
    let s = Settings::default();
    assert_eq!(s.sort_string, sort_string::DEFAULT);
    assert_eq!(s.class_filter, "");
    assert_eq!(s.name_filter, "");
    assert_eq!(s.start_slot, 0);
    assert!(!s.reverse);
    assert!(s.identify_on_login);
    assert!(s.announce_when_done);
    assert_eq!(
        s.presets,
        vec![
            Preset::new("Everything", "OC,EQ,CV,AL-,AB-,DB-,ED-,ML,MT,NM"),
            Preset::new("Saved 1", "OC,EP,MA,AL-,MX-"),
            Preset::new("Saved 2", "AL-,MX-"),
            Preset::new("Saved 3", "BR-"),
        ]
    );
}

/// mudsort's first saved string names `MA`, which no key has: it decodes
/// as mudsort decoded it, without it.
#[test]
fn mudsorts_saved_strings_decode_as_in_mudsort() {
    let first = sort_string::parse(MUDSORT_SAVED[0]);
    assert_eq!(first.unknown, vec!["MA".to_owned()]);
    assert_eq!(sort_string::serialize(&first.keys), "OC,EP,AL-,MX-");
    assert!(sort_string::parse(MUDSORT_SAVED[1]).unknown.is_empty());
    assert!(sort_string::parse(MUDSORT_SAVED[2]).unknown.is_empty());
}

#[test]
fn a_member_missing_from_the_file_reads_as_its_default() {
    assert_eq!(Settings::from_json("{}").unwrap(), Settings::default());
    let s = Settings::from_json(r#"{"sortString":"NM","reverse":true}"#).unwrap();
    assert_eq!(s.sort_string, "NM");
    assert!(s.reverse);
    assert!(s.identify_on_login);
}

#[test]
fn settings_round_trip_through_the_characters_storage() {
    let mut h = Harness::logged_in(|_| {});
    // Nothing is written until something changes.
    assert!(stored(&h).is_none());
    h.ms("set flags nm,al-");
    h.with_host(|p, host| {
        p.update_settings(host, |s| {
            s.start_slot = 4;
            s.name_filter = "gem".to_owned();
            s.presets.push(Preset::new("Mine", "VA-"));
        });
    });
    let text = stored(&h).cloned().expect("written");
    let saved = Settings::from_json(&text).unwrap();
    assert_eq!(&saved, h.plugin.settings());

    let mut again = Harness::logged_in(|sim| {
        sim.storage.insert(format!("{NAME}/{SETTINGS_KEY}"), text);
    });
    assert_eq!(again.plugin.settings(), &saved);
    assert_eq!(again.plugin.settings().sort_string, "NM,AL-");
    assert_eq!(again.plugin.settings().start_slot, 4);
    // And source and destination are the character again.
    again.ms("set dest pack 1");
    assert_eq!(again.plugin.dest(), PACK1);
    again.send(openac_plugin::Event::LoginComplete);
    assert_eq!(again.plugin.dest(), ME);
}

#[test]
fn an_unreadable_file_reads_as_the_defaults() {
    let h = Harness::logged_in(|sim| {
        sim.storage
            .insert(format!("{NAME}/{SETTINGS_KEY}"), "not json".to_owned());
    });
    assert_eq!(h.plugin.settings(), &Settings::default());
    assert!(
        h.sim
            .fake
            .logs
            .iter()
            .any(|(_, line)| line.contains("could not be read"))
    );
}

#[test]
fn login_queues_the_inventory_for_identification() {
    let mut h = Harness::logged_in(|sim| {
        sim.add_item(0x8000_2001, "Gem", ObjectClass::Gem, ME, 0, false);
        sim.add_item(0x8000_2002, "Known", ObjectClass::Gem, ME, 0, true);
        sim.add_item(0x8000_2003, "Deep", ObjectClass::Gem, PACK1, 0, false);
    });
    h.tick();
    // One at a time.
    h.tick();
    assert_eq!(h.sim.identifies(), vec![0x8000_2001]);
    let event = h.sim.answer_appraisal().unwrap();
    h.send(event);
    h.tick();
    // The appraised one is skipped; the one in a pack is asked about.
    assert_eq!(h.sim.identifies(), vec![0x8000_2001, 0x8000_2003]);
}

#[test]
fn no_identification_at_login_when_switched_off() {
    let text = Settings {
        identify_on_login: false,
        ..Settings::default()
    }
    .to_json();
    let mut h = Harness::logged_in(|sim| {
        sim.add_item(0x8000_2001, "Gem", ObjectClass::Gem, ME, 0, false);
        sim.storage.insert(format!("{NAME}/{SETTINGS_KEY}"), text);
    });
    for _ in 0..5 {
        h.tick();
    }
    assert!(h.sim.identifies().is_empty());
}
