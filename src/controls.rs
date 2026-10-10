//! Single source of truth for every keyboard and mouse binding in the game.
//!
//! All input handling (`main.rs`, `player.rs`) reads keys from the constants here, and all
//! on-screen guidance (HUD hint rows, reticle prompts, the F1 field manual) builds its labels
//! from the same [`Binding`] records. Tests at the bottom of this file enforce that:
//! - no key is bound to two actions in contexts that can be active at the same time,
//! - `main.rs` / `player.rs` never use a raw `KeyCode::` literal (so nothing bypasses this table),
//! - every label matches the keys it describes, and all text is renderable by the default font.

use crate::weapon::WeaponMode;
use crate::world_gen::LevelPreset;
use macroquad::prelude::KeyCode;

/// When a binding is live. Used to detect conflicting bindings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Context {
    /// Live at all times, even while the field manual is open (F1 / Esc).
    Always,
    /// Live whenever the field manual is closed (including during replay and the workbench).
    Global,
    /// Live only while actually playing: cursor locked, no workbench, no replay, no manual.
    Gameplay,
    /// Live in campaign mode when the manual is closed and replay is not running.
    Campaign,
    /// Live only during cinematic replay (manual closed).
    Replay,
}

impl Context {
    /// Whether two contexts can be live during the same frame.
    pub fn overlaps(self, other: Context) -> bool {
        use Context::*;
        !matches!(
            (self, other),
            (Gameplay, Replay) | (Replay, Gameplay) | (Campaign, Replay) | (Replay, Campaign)
        )
    }
}

/// Grouping used to lay out the F1 field manual.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Category {
    Movement,
    Weapons,
    Demolition,
    Siege,
    Tactical,
    Campaign,
    Replay,
    System,
}

/// One player action and the input(s) that trigger it.
#[derive(Copy, Clone, Debug)]
pub struct Binding {
    /// Keyboard keys. Empty for mouse-only actions.
    pub keys: &'static [KeyCode],
    /// Short on-screen label, e.g. `"B"`, `"W/A/S/D"`, `"1-0"`, `"LMB"`.
    pub label: &'static str,
    /// Bracketed on-screen token, e.g. `[B]`. The bracket keys render bare (`[` / `]`).
    pub token: &'static str,
    /// What the action does, in on-screen wording.
    pub action: &'static str,
    pub context: Context,
    pub category: Category,
}

// ---------------------------------------------------------------------------------------------
// Key constants (the only place raw KeyCodes live)
// ---------------------------------------------------------------------------------------------
pub const KEY_MANUAL: KeyCode = KeyCode::F1;
pub const KEY_ESCAPE: KeyCode = KeyCode::Escape;
pub const KEY_WORKBENCH: KeyCode = KeyCode::Tab;
pub const KEY_LEVEL_PREV: KeyCode = KeyCode::LeftBracket;
pub const KEY_LEVEL_NEXT: KeyCode = KeyCode::RightBracket;
pub const KEY_RESET: KeyCode = KeyCode::R;
pub const KEY_RESET_ALT: KeyCode = KeyCode::Backspace;
pub const KEY_SCANNER: KeyCode = KeyCode::V;
pub const KEY_MUSIC: KeyCode = KeyCode::M;
pub const KEY_CAMPAIGN: KeyCode = KeyCode::C;
pub const KEY_REPLAY: KeyCode = KeyCode::F10;
pub const KEY_FLIGHT_CAM: KeyCode = KeyCode::X;

pub const KEY_FORWARD: KeyCode = KeyCode::W;
pub const KEY_LEFT: KeyCode = KeyCode::A;
pub const KEY_BACK: KeyCode = KeyCode::S;
pub const KEY_RIGHT: KeyCode = KeyCode::D;
pub const KEY_SPRINT: KeyCode = KeyCode::LeftShift;
pub const KEY_JUMP: KeyCode = KeyCode::Space;
pub const KEY_GRAPPLE: KeyCode = KeyCode::Q;
pub const KEY_INVERT_PITCH: KeyCode = KeyCode::I;

pub const KEY_C4_PLANT: KeyCode = KeyCode::E;
pub const KEY_C4_DETONATE: KeyCode = KeyCode::F;
pub const KEY_SIEGE_DEPLOY: KeyCode = KeyCode::B;
pub const KEY_SIEGE_FIRE: KeyCode = KeyCode::G;
pub const KEY_SIEGE_CYCLE: KeyCode = KeyCode::Z;
pub const KEY_SIEGE_RECALL: KeyCode = KeyCode::Y;
pub const KEY_SLOWMO: KeyCode = KeyCode::T;
pub const KEY_PURGE: KeyCode = KeyCode::H;

pub const KEY_CONTRACT_START: KeyCode = KeyCode::Enter;
pub const KEY_CONTRACT_NEXT: KeyCode = KeyCode::N;
pub const KEY_CONTRACT_PREV: KeyCode = KeyCode::P;

pub const KEY_REPLAY_PAUSE: KeyCode = KeyCode::Space;
pub const KEY_REPLAY_BACK: KeyCode = KeyCode::Left;
pub const KEY_REPLAY_FWD: KeyCode = KeyCode::Right;
pub const KEY_REPLAY_FASTER: KeyCode = KeyCode::Up;
pub const KEY_REPLAY_SLOWER: KeyCode = KeyCode::Down;

/// Kinetic grapple maximum anchor distance (static voxels only), in metres.
pub const GRAPPLE_RANGE: f32 = 60.0;

/// Direct weapon selection keys (1-9, then 0 for the tenth weapon).
pub const WEAPON_KEYS: [(KeyCode, WeaponMode); 10] = [
    (KeyCode::Key1, WeaponMode::TitanRam),
    (KeyCode::Key2, WeaponMode::HavocMortar),
    (KeyCode::Key3, WeaponMode::EventHorizon),
    (KeyCode::Key4, WeaponMode::SolsticeLance),
    (KeyCode::Key5, WeaponMode::ApexRailgun),
    (KeyCode::Key6, WeaponMode::FrostbiteMortar),
    (KeyCode::Key7, WeaponMode::SeismicDisrupter),
    (KeyCode::Key8, WeaponMode::TeslaCaster),
    (KeyCode::Key9, WeaponMode::PlasmaFlamer),
    (KeyCode::Key0, WeaponMode::PlasmaWireSaw),
];

/// Direct level jump keys. Level 1 (Apex Skyscraper) is reached with `[` / `]` or reset.
pub const LEVEL_KEYS: [(KeyCode, LevelPreset); 8] = [
    (KeyCode::F2, LevelPreset::SuspensionSkybridge),
    (KeyCode::F3, LevelPreset::MilitaryFortress),
    (KeyCode::F4, LevelPreset::DominoMonoliths),
    (KeyCode::F5, LevelPreset::GothicCitadel),
    (KeyCode::F6, LevelPreset::NeoTokyoSpire),
    (KeyCode::F7, LevelPreset::ChemicalRefinery),
    (KeyCode::F8, LevelPreset::OrbitalLaunchGantry),
    (KeyCode::F9, LevelPreset::QuantumFusionReactor),
];

const WEAPON_KEYCODES: [KeyCode; 10] = [
    KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5,
    KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9, KeyCode::Key0,
];
const LEVEL_KEYCODES: [KeyCode; 8] = [
    KeyCode::F2, KeyCode::F3, KeyCode::F4, KeyCode::F5,
    KeyCode::F6, KeyCode::F7, KeyCode::F8, KeyCode::F9,
];

// ---------------------------------------------------------------------------------------------
// Bindings
// ---------------------------------------------------------------------------------------------
macro_rules! binding {
    ($name:ident, [$($k:expr),*], $label:expr, $action:expr, $ctx:ident, $cat:ident) => {
        pub const $name: Binding = Binding {
            keys: &[$($k),*],
            label: $label,
            token: concat!("[", $label, "]"),
            action: $action,
            context: Context::$ctx,
            category: Category::$cat,
        };
    };
}

// System
binding!(MANUAL, [KEY_MANUAL], "F1", "OPEN / CLOSE THIS MANUAL", Always, System);
binding!(ESCAPE, [KEY_ESCAPE], "ESC", "CLOSE PANEL / FREE CURSOR", Always, System);
binding!(WORKBENCH, [KEY_WORKBENCH], "TAB", "WEAPON WORKBENCH", Global, System);
pub const LEVEL_PREV: Binding = Binding {
    keys: &[KEY_LEVEL_PREV],
    label: "[",
    token: "[",
    action: "PREVIOUS LEVEL",
    context: Context::Global,
    category: Category::System,
};
pub const LEVEL_NEXT: Binding = Binding {
    keys: &[KEY_LEVEL_NEXT],
    label: "]",
    token: "]",
    action: "NEXT LEVEL",
    context: Context::Global,
    category: Category::System,
};
binding!(RESET, [KEY_RESET, KEY_RESET_ALT], "R/BKSP", "RESET LEVEL", Global, System);
pub const LEVEL_JUMP: Binding = Binding {
    keys: &LEVEL_KEYCODES,
    label: "F2-F9",
    token: "[F2-F9]",
    action: "JUMP TO LEVEL 2-9",
    context: Context::Global,
    category: Category::System,
};

// Movement
binding!(MOVE, [KEY_FORWARD, KEY_LEFT, KEY_BACK, KEY_RIGHT], "W/A/S/D", "MOVE / STRAFE", Gameplay, Movement);
binding!(SPRINT, [KEY_SPRINT], "SHIFT", "SPRINT", Gameplay, Movement);
binding!(JUMP, [KEY_JUMP], "SPACE", "JUMP / SLINGSHOT OFF WINCH", Gameplay, Movement);
binding!(GRAPPLE, [KEY_GRAPPLE], "Q", "GRAPPLE (60M) OR JET-DASH", Gameplay, Movement);
binding!(INVERT_PITCH, [KEY_INVERT_PITCH], "I", "INVERT MOUSE PITCH", Gameplay, Movement);

// Weapons
binding!(FIRE, [], "LMB", "FIRE WEAPON", Gameplay, Weapons);
binding!(ALT_FIRE, [], "RMB", "ALT-FIRE (SEE LIST)", Gameplay, Weapons);
pub const WEAPON_SELECT: Binding = Binding {
    keys: &WEAPON_KEYCODES,
    label: "1-0",
    token: "[1-0]",
    action: "SELECT WEAPON",
    context: Context::Global,
    category: Category::Weapons,
};

// Demolition
binding!(C4_PLANT, [KEY_C4_PLANT], "E", "PLANT C4 ON AIMED SURFACE", Gameplay, Demolition);
binding!(C4_DETONATE, [KEY_C4_DETONATE], "F", "DETONATE ALL C4", Gameplay, Demolition);

// Siege
binding!(SIEGE_DEPLOY, [KEY_SIEGE_DEPLOY], "B", "DEPLOY / MOVE BATTERY", Gameplay, Siege);
binding!(SIEGE_FIRE, [KEY_SIEGE_FIRE], "G", "FIRE BATTERY AT AIM", Gameplay, Siege);
binding!(SIEGE_CYCLE, [KEY_SIEGE_CYCLE], "Z", "CYCLE BATTERY TYPE", Gameplay, Siege);
binding!(SIEGE_RECALL, [KEY_SIEGE_RECALL], "Y", "RECALL ALL BATTERIES", Gameplay, Siege);

// Tactical
binding!(SLOWMO, [KEY_SLOWMO], "T", "SLOW-MO ON / OFF", Gameplay, Tactical);
binding!(SLOWMO_RATE, [], "WHEEL", "SLOW-MO SPEED (WHILE ON)", Gameplay, Tactical);
binding!(SCANNER, [KEY_SCANNER], "V", "STRESS SCANNER", Global, Tactical);
binding!(PURGE, [KEY_PURGE], "H", "HAZARD FLUID DISCHARGE", Gameplay, Tactical);
binding!(MUSIC, [KEY_MUSIC], "M", "MUSIC ON / OFF", Global, Tactical);

// Campaign
binding!(CAMPAIGN, [KEY_CAMPAIGN], "C", "CAMPAIGN / SANDBOX", Global, Campaign);
binding!(CONTRACT_START, [KEY_CONTRACT_START], "ENTER", "START / ADVANCE / RETRY", Campaign, Campaign);
binding!(CONTRACT_NEXT, [KEY_CONTRACT_NEXT], "N", "NEXT CONTRACT", Campaign, Campaign);
binding!(CONTRACT_PREV, [KEY_CONTRACT_PREV], "P", "PREVIOUS CONTRACT", Campaign, Campaign);

// Replay
binding!(REPLAY, [KEY_REPLAY], "F10", "CINEMATIC REPLAY", Global, Replay);
binding!(FLIGHT_CAM, [KEY_FLIGHT_CAM], "X", "PROJECTILE FLIGHT CAM", Global, Replay);
binding!(REPLAY_PAUSE, [KEY_REPLAY_PAUSE], "SPACE", "PLAY / PAUSE", Replay, Replay);
binding!(REPLAY_SCRUB, [KEY_REPLAY_BACK, KEY_REPLAY_FWD], "LEFT/RIGHT", "SCRUB TIMELINE", Replay, Replay);
binding!(REPLAY_SPEED, [KEY_REPLAY_FASTER, KEY_REPLAY_SLOWER], "UP/DOWN", "PLAYBACK SPEED", Replay, Replay);
binding!(REPLAY_ORBIT, [], "DRAG", "ORBIT CAMERA", Replay, Replay);
binding!(REPLAY_ZOOM, [], "WHEEL", "ZOOM CAMERA", Replay, Replay);

/// Every binding in the game. The F1 manual and the conflict tests iterate this list.
pub const BINDINGS: &[Binding] = &[
    MANUAL, ESCAPE, WORKBENCH, LEVEL_PREV, LEVEL_NEXT, RESET, LEVEL_JUMP,
    MOVE, SPRINT, JUMP, GRAPPLE, INVERT_PITCH,
    FIRE, ALT_FIRE, WEAPON_SELECT,
    C4_PLANT, C4_DETONATE,
    SIEGE_DEPLOY, SIEGE_FIRE, SIEGE_CYCLE, SIEGE_RECALL,
    SLOWMO, SLOWMO_RATE, SCANNER, PURGE, MUSIC,
    CAMPAIGN, CONTRACT_START, CONTRACT_NEXT, CONTRACT_PREV,
    REPLAY, FLIGHT_CAM, REPLAY_PAUSE, REPLAY_SCRUB, REPLAY_SPEED, REPLAY_ORBIT, REPLAY_ZOOM,
];

/// Human-readable name for a key, used to verify labels against their key lists.
pub fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::W => "W", KeyCode::A => "A", KeyCode::S => "S", KeyCode::D => "D",
        KeyCode::Q => "Q", KeyCode::E => "E", KeyCode::R => "R", KeyCode::T => "T",
        KeyCode::Y => "Y", KeyCode::I => "I", KeyCode::P => "P", KeyCode::F => "F",
        KeyCode::G => "G", KeyCode::H => "H", KeyCode::Z => "Z", KeyCode::X => "X",
        KeyCode::C => "C", KeyCode::V => "V", KeyCode::B => "B", KeyCode::N => "N",
        KeyCode::M => "M",
        KeyCode::Key0 => "0", KeyCode::Key1 => "1", KeyCode::Key2 => "2", KeyCode::Key3 => "3",
        KeyCode::Key4 => "4", KeyCode::Key5 => "5", KeyCode::Key6 => "6", KeyCode::Key7 => "7",
        KeyCode::Key8 => "8", KeyCode::Key9 => "9",
        KeyCode::F1 => "F1", KeyCode::F2 => "F2", KeyCode::F3 => "F3", KeyCode::F4 => "F4",
        KeyCode::F5 => "F5", KeyCode::F6 => "F6", KeyCode::F7 => "F7", KeyCode::F8 => "F8",
        KeyCode::F9 => "F9", KeyCode::F10 => "F10",
        KeyCode::Space => "SPACE", KeyCode::LeftShift => "SHIFT", KeyCode::Tab => "TAB",
        KeyCode::Escape => "ESC", KeyCode::Enter => "ENTER", KeyCode::Backspace => "BKSP",
        KeyCode::LeftBracket => "[", KeyCode::RightBracket => "]",
        KeyCode::Left => "LEFT", KeyCode::Right => "RIGHT", KeyCode::Up => "UP", KeyCode::Down => "DOWN",
        _ => "?",
    }
}

/// Weapon display name without the parenthesised subtitle, e.g. `"APEX RAILGUN"`.
pub fn short_weapon_name(mode: WeaponMode) -> &'static str {
    let full = mode.name();
    match full.find(" (") {
        Some(idx) => &full[..idx],
        None => full,
    }
}

/// Whether a character can be drawn by macroquad's built-in ProggyClean font
/// (printable ASCII, Latin-1 supplement and the euro sign; verified against the font's cmap).
pub fn is_renderable_char(c: char) -> bool {
    matches!(c, ' '..='~' | '\u{A0}'..='\u{FF}' | '\u{20AC}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_is_bound_twice_in_overlapping_contexts() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for (k_idx, ka) in a.keys.iter().enumerate() {
                assert!(
                    !a.keys[k_idx + 1..].contains(ka),
                    "binding '{}' lists key {:?} twice", a.action, ka
                );
            }
            for b in &BINDINGS[i + 1..] {
                if !a.context.overlaps(b.context) {
                    continue;
                }
                for ka in a.keys {
                    assert!(
                        !b.keys.contains(ka),
                        "key {:?} is bound to both '{}' ({:?}) and '{}' ({:?})",
                        ka, a.action, a.context, b.action, b.context
                    );
                }
            }
        }
    }

    #[test]
    fn labels_match_their_keys() {
        for b in BINDINGS {
            if b.keys.is_empty() {
                assert!(
                    matches!(b.label, "LMB" | "RMB" | "WHEEL" | "DRAG"),
                    "mouse binding '{}' has unexpected label {}", b.action, b.label
                );
                continue;
            }
            let names: Vec<&str> = b.keys.iter().map(|k| key_name(*k)).collect();
            assert!(!names.contains(&"?"), "binding '{}' uses a key with no display name", b.action);
            if names.len() == 1 {
                assert_eq!(b.label, names[0], "label for '{}' does not match its key", b.action);
            } else if b.label.contains('-') && !b.label.contains('/') {
                // Range label like "1-0" or "F2-F9": must name the first and last key.
                let (first, last) = b.label.split_once('-').unwrap();
                assert_eq!(first, names[0], "range label for '{}' starts wrong", b.action);
                assert_eq!(last, *names.last().unwrap(), "range label for '{}' ends wrong", b.action);
            } else {
                let parts: Vec<&str> = b.label.split('/').collect();
                assert_eq!(parts, names, "label for '{}' does not list its keys", b.action);
            }
        }
    }

    #[test]
    fn every_label_has_a_bracketed_token() {
        for b in BINDINGS {
            let t = b.token;
            if b.label == "[" || b.label == "]" {
                assert_eq!(t, b.label);
            } else {
                assert_eq!(t, format!("[{}]", b.label), "token for '{}' does not match its label", b.action);
            }
        }
    }

    #[test]
    fn weapon_and_level_key_tables_match_bindings() {
        let weapon_keys: Vec<KeyCode> = WEAPON_KEYS.iter().map(|(k, _)| *k).collect();
        assert_eq!(weapon_keys.as_slice(), WEAPON_SELECT.keys);
        let level_keys: Vec<KeyCode> = LEVEL_KEYS.iter().map(|(k, _)| *k).collect();
        assert_eq!(level_keys.as_slice(), LEVEL_JUMP.keys);
    }

    #[test]
    fn input_code_never_bypasses_the_binding_table() {
        for (file, src) in [("main.rs", include_str!("main.rs")), ("player.rs", include_str!("player.rs"))] {
            for (n, line) in src.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                assert!(
                    !code.contains("KeyCode::"),
                    "{}:{} uses a raw KeyCode; add it to controls.rs instead: {}",
                    file, n + 1, line.trim()
                );
            }
        }
    }

    #[test]
    fn binding_text_is_renderable_by_default_font() {
        for b in BINDINGS {
            for s in [b.label, b.action, b.token] {
                assert!(s.chars().all(is_renderable_char), "unrenderable char in '{}'", s);
            }
        }
    }

    #[test]
    fn all_hud_source_strings_are_renderable_by_default_font() {
        // Any code (outside comments) in files that draw text must only use glyphs the
        // built-in font has. Lines may opt out with the marker "glyph-check: allow".
        let files = [
            ("main.rs", include_str!("main.rs")),
            ("ui.rs", include_str!("ui.rs")),
            ("campaign.rs", include_str!("campaign.rs")),
            ("replay.rs", include_str!("replay.rs")),
            ("customizer.rs", include_str!("customizer.rs")),
            ("siege.rs", include_str!("siege.rs")),
            ("weapon.rs", include_str!("weapon.rs")),
            ("music.rs", include_str!("music.rs")),
            ("fluid.rs", include_str!("fluid.rs")),
        ];
        for (file, src) in files {
            for (n, line) in src.lines().enumerate() {
                if line.contains("glyph-check: allow") {
                    continue;
                }
                let code = line.split("//").next().unwrap_or("");
                if let Some(bad) = code.chars().find(|c| !c.is_whitespace() && !is_renderable_char(*c)) {
                    panic!(
                        "{}:{} contains '{}' (U+{:04X}) which the default font cannot draw: {}",
                        file, n + 1, bad, bad as u32, line.trim()
                    );
                }
            }
        }
    }

    #[test]
    fn grapple_range_matches_player_code() {
        let src = include_str!("player.rs");
        assert!(
            src.contains("crate::controls::GRAPPLE_RANGE"),
            "player.rs must use controls::GRAPPLE_RANGE so the HUD prompt stays accurate"
        );
    }
}
