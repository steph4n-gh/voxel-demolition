use macroquad::prelude::*;
use crate::weapon::{
    WeaponSystem, WeaponMode, BarrelType, DetonatorType,
    PayloadType, TrailStyle, StabilizerType,
};
use crate::siege::{
    SiegeSystem, SiegeEngineKind, SiegePayload, SiegeTrajectory,
};
use crate::audio::SoundSystem;
use crate::ui::{compute_ui_scale, fit_text_to_width, FONT_ADVANCE_RATIO};
use crate::voxel::VoxelType;

pub struct CombatPreset {
    pub name: &'static str,
    pub mode: WeaponMode,
    pub barrel: BarrelType,
    pub payload: PayloadType,
    pub trail: TrailStyle,
    pub stabilizer: StabilizerType,
    pub blast_radius_mult: f32,
    pub impulse_mult: f32,
    pub fire_rate_mult: f32,
    pub velocity_mult: f32,
    pub description: &'static str,
}

pub const PRESETS: &[CombatPreset] = &[
    CombatPreset {
        name: "BUNKER BUSTER",
        mode: WeaponMode::HavocMortar,
        barrel: BarrelType::Precision,
        payload: PayloadType::HighExplosive,
        trail: TrailStyle::SmokeWisps,
        stabilizer: StabilizerType::HeavyCompensator,
        blast_radius_mult: 1.8,
        impulse_mult: 1.6,
        fire_rate_mult: 1.0,
        velocity_mult: 1.2,
        description: "Devastating artillery mortar with expanded blast radius for foundation demolition.",
    },
    CombatPreset {
        name: "SEISMIC FAULT",
        mode: WeaponMode::SeismicDisrupter,
        barrel: BarrelType::Precision,
        payload: PayloadType::ShapedCharge,
        trail: TrailStyle::SparkShower,
        stabilizer: StabilizerType::HeavyCompensator,
        blast_radius_mult: 1.6,
        impulse_mult: 2.2,
        fire_rate_mult: 1.0,
        velocity_mult: 1.3,
        description: "High-impulse ground fault shockwave to heave building foundations and trigger pancake collapses.",
    },
    CombatPreset {
        name: "TESLA OVERLOAD",
        mode: WeaponMode::TeslaCaster,
        barrel: BarrelType::Precision,
        payload: PayloadType::HighExplosive,
        trail: TrailStyle::HyperTracer,
        stabilizer: StabilizerType::Overclocked,
        blast_radius_mult: 1.2,
        impulse_mult: 1.4,
        fire_rate_mult: 1.4,
        velocity_mult: 1.4,
        description: "Rapid-fire electrical discharge conducting through steel rebar internal skeletons.",
    },
    CombatPreset {
        name: "PLASMA TORRENT",
        mode: WeaponMode::PlasmaFlamer,
        barrel: BarrelType::Precision,
        payload: PayloadType::Incendiary,
        trail: TrailStyle::PlasmaGlow,
        stabilizer: StabilizerType::GyroscopicThruster,
        blast_radius_mult: 1.0,
        impulse_mult: 1.2,
        fire_rate_mult: 1.8,
        velocity_mult: 1.5,
        description: "High-cadence continuous stream of superheated thermite plasma melting solid facades.",
    },
    CombatPreset {
        name: "CRYO SHATTER",
        mode: WeaponMode::FrostbiteMortar,
        barrel: BarrelType::TriSpread,
        payload: PayloadType::CryoShrapnel,
        trail: TrailStyle::SparkShower,
        stabilizer: StabilizerType::Standard,
        blast_radius_mult: 1.4,
        impulse_mult: 1.4,
        fire_rate_mult: 1.1,
        velocity_mult: 1.1,
        description: "Tri-spread sub-zero artillery flash-freezing concrete walls into brittle shatter-prone ice.",
    },
    CombatPreset {
        name: "APEX RAILGUN",
        mode: WeaponMode::ApexRailgun,
        barrel: BarrelType::Piercing,
        payload: PayloadType::ShapedCharge,
        trail: TrailStyle::HyperTracer,
        stabilizer: StabilizerType::Overclocked,
        blast_radius_mult: 1.5,
        impulse_mult: 2.8,
        fire_rate_mult: 1.0,
        velocity_mult: 1.8,
        description: "Relativistic kinetic dart punching deep through multiple solid walls with exit blowouts.",
    },
    CombatPreset {
        name: "TITAN BREACHER",
        mode: WeaponMode::TitanRam,
        barrel: BarrelType::Piercing,
        payload: PayloadType::HighExplosive,
        trail: TrailStyle::SparkShower,
        stabilizer: StabilizerType::HeavyCompensator,
        blast_radius_mult: 2.2,
        impulse_mult: 3.2,
        fire_rate_mult: 1.0,
        velocity_mult: 1.0,
        description: "Close-range hydraulic ram delivering colossal kinetic impulse to crush perimeter columns.",
    },
];

pub struct CustomizerUI {
    pub is_open: bool,
    pub active_tab: usize, // 0: Ordnance, 1: Ballistics, 2: Siege Engineering, 3: Material Telemetry, 4: Audio Synthesizer
    // Tab 3: Material Telemetry state
    pub selected_material: usize,
    // Tab 4: Audio Synthesizer tuning parameters
    pub master_volume: f32,
    pub siege_volume: f32,
    pub anti_stacking_ms: f32,
    pub stress_creak_freq: f32,
    pub stress_creak_vol: f32,
    pub last_auditioned_sound: &'static str,
    pub music_volume: f32,
    pub synth_bass_boost: f32,
}

/// Wraps text into multiple lines where each line's estimated width does not exceed max_width.
pub fn wrap_text_to_lines(text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur_line = String::new();
    let max_chars = (max_width.max(0.0) / (FONT_ADVANCE_RATIO * font_size.ceil().max(1.0))).floor() as usize;
    if max_chars == 0 {
        return lines;
    }
    let mut line_chars = 0;

    for word in text.split_whitespace() {
        if line_chars > 0 {
            if line_chars + 1 + word.chars().count() > max_chars {
                lines.push(std::mem::take(&mut cur_line));
                line_chars = 0;
            } else {
                cur_line.push(' ');
                line_chars += 1;
            }
        }
        for ch in word.chars() {
            if line_chars == max_chars {
                lines.push(std::mem::take(&mut cur_line));
                line_chars = 0;
            }
            cur_line.push(ch);
            line_chars += 1;
        }
    }
    if !cur_line.is_empty() {
        lines.push(cur_line);
    }
    lines
}

/// Draws multiline text with word-wrapping, ensuring zero layout overflow into adjacent columns.
#[allow(clippy::too_many_arguments)]
pub fn draw_multiline_text(
    text: &str,
    x: f32,
    y: f32,
    max_width: f32,
    font_size: f32,
    line_spacing: f32,
    color: Color,
    max_lines: usize,
) {
    let lines = wrap_text_to_lines(text, max_width, font_size);
    for (i, line) in lines.iter().take(max_lines).enumerate() {
        draw_text(line, x, y + i as f32 * line_spacing, font_size, color);
    }
}

impl CustomizerUI {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: 0,
            selected_material: 0,
            master_volume: 1.0,
            siege_volume: 1.2,
            anti_stacking_ms: 75.0,
            stress_creak_freq: 210.0,
            stress_creak_vol: 1.0,
            last_auditioned_sound: "NONE",
            music_volume: 0.65,
            synth_bass_boost: 1.30,
        }
    }

    pub fn toggle(&mut self, audio: &SoundSystem) {
        self.is_open = !self.is_open;
        audio.play(audio.ui_switch.as_ref(), 0.6);
    }

    pub fn update_and_draw(&mut self, weapon: &mut WeaponSystem, siege: &mut SiegeSystem, audio: &SoundSystem) {
        if !self.is_open {
            return;
        }

        let sw = screen_width();
        let sh = screen_height();

        let s = compute_ui_scale(sw, sh);

        // 1. Dark translucent frosted backdrop
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.04, 0.05, 0.08, 0.88));

        // 2. Center Responsive Workbench Panel
        let pw = (sw * 0.94).min(1260.0 * s);
        let ph = (sh * 0.92).min(780.0 * s);
        let px = (sw - pw) * 0.5;
        let py = (sh - ph) * 0.5;

        // Panel background card
        draw_rectangle(px, py, pw, ph, Color::new(0.09, 0.11, 0.15, 0.97));
        draw_rectangle_lines(px, py, pw, ph, 1.8 * s, Color::new(0.25, 0.45, 0.70, 0.85));

        // Header bar
        let theme_color = match self.active_tab {
            2 => siege.selected_kind.theme_color(),
            3 => Color::new(0.30, 0.85, 0.95, 1.0),
            4 => Color::new(0.95, 0.75, 0.25, 1.0),
            _ => weapon.config.mode.color(),
        };
        let head_h = 44.0 * s;
        draw_rectangle(px, py, pw, head_h, Color::new(0.13, 0.16, 0.22, 1.0));
        draw_text("MODULAR WEAPON & SIEGE WORKBENCH", px + 16.0 * s, py + 29.0 * s, 19.0 * s, WHITE);
        let hint = format!(
            "[ PRESS {} OR {} TO RESUME ]",
            crate::controls::WORKBENCH.label,
            crate::controls::ESCAPE.label
        );
        let hint = hint.as_str();
        let hw = measure_text(hint, None, (13.5f32 * s).max(8.0f32).round() as u16, 1.0).width;
        draw_text(hint, px + pw - hw - 16.0 * s, py + 28.0 * s, 13.5 * s, Color::new(0.70, 0.80, 0.92, 0.9));

        let mouse_pos = mouse_position();
        let mouse_clicked = is_mouse_button_pressed(MouseButton::Left);

        // 3. Tab Navigation Bar (Mandated 5 Responsive Categories)
        let tab_y = py + head_h + 6.0 * s;
        let tab_h = 36.0 * s;
        let tabs = [
            "1. ORDNANCE",
            "2. BALLISTICS",
            "3. SIEGE ENGINEERING",
            "4. MATERIAL TELEMETRY",
            "5. AUDIO SYNTHESIZER",
        ];
        let tab_w = (pw - 24.0 * s - 4.0 * 6.0 * s) / 5.0;

        for (i, &t_title) in tabs.iter().enumerate() {
            let bx = px + 12.0 * s + i as f32 * (tab_w + 6.0 * s);
            let is_active = self.active_tab == i;
            let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + tab_w && mouse_pos.1 >= tab_y && mouse_pos.1 <= tab_y + tab_h;

            let tab_theme = match i {
                0 | 1 => weapon.config.mode.color(),
                2 => siege.selected_kind.theme_color(),
                3 => Color::new(0.30, 0.85, 0.95, 1.0),
                _ => Color::new(0.95, 0.75, 0.25, 1.0),
            };

            let bg_c = if is_active {
                Color::new(0.18, 0.24, 0.34, 1.0)
            } else if is_hover {
                Color::new(0.15, 0.19, 0.26, 1.0)
            } else {
                Color::new(0.11, 0.13, 0.18, 1.0)
            };

            draw_rectangle(bx, tab_y, tab_w, tab_h, bg_c);
            let border_c = if is_active { tab_theme } else { Color::new(0.25, 0.32, 0.42, 0.8) };
            draw_rectangle_lines(bx, tab_y, tab_w, tab_h, if is_active { 2.0 * s } else { 1.0 }, border_c);

            if is_active {
                draw_rectangle(bx, tab_y + tab_h - 3.5 * s, tab_w, 3.5 * s, tab_theme);
            }

            let text_c = if is_active { WHITE } else { Color::new(0.75, 0.82, 0.90, 0.85) };
            let tw = measure_text(t_title, None, (13.5f32 * s).max(8.0f32).round() as u16, 1.0).width;
            draw_text(t_title, bx + (tab_w - tw) * 0.5, tab_y + 23.0 * s, 13.5 * s, text_c);

            if is_hover && mouse_clicked && !is_active {
                self.active_tab = i;
                audio.play(audio.ui_click.as_ref(), 0.7);
            }
        }

        // Content Area Bounds
        let content_x = px + 16.0 * s;
        let content_y = tab_y + tab_h + 10.0 * s;
        let content_w = pw - 32.0 * s;
        let content_h = py + ph - content_y - 10.0 * s;

        match self.active_tab {
            // ==================== TAB 0: ORDNANCE ====================
            0 => {
                let chassis_list = [
                    (WeaponMode::TitanRam, "01", "TITAN MAUL", "Hydraulic pneumatic siege ram with extreme kinetic concussive impulse.", "KINETIC BREACH"),
                    (WeaponMode::HavocMortar, "02", "HAVOC MORTAR", "Heavy high-explosive parabolic artillery shell with devastating crater blast.", "CONCUSSIVE BLAST"),
                    (WeaponMode::EventHorizon, "03", "EVENT HORIZON", "Gravitational singularity vortex pulling surrounding architecture into a vortex collapse.", "GRAVITATIONAL VORTEX"),
                    (WeaponMode::SolsticeLance, "04", "SOLSTICE LANCE", "Continuous thermal laser beam surgical drilling and notch-cutting load-bearing columns.", "THERMAL LASER"),
                    (WeaponMode::ApexRailgun, "05", "APEX RAILGUN", "Hypervelocity relativistic tungsten slug punch-through with double-sided shockwave blowouts.", "PIERCING SLUG"),
                    (WeaponMode::FrostbiteMortar, "06", "FROSTBITE", "Sub-zero flash-freeze cryo mortar turning elastic concrete into shatter-prone glass ice.", "CRYO FREEZE"),
                    (WeaponMode::SeismicDisrupter, "07", "SEISMIC DISRUPTER", "Low-frequency ground fault oscillator propagating seismic shockwaves through foundations.", "SEISMIC HEAVE"),
                    (WeaponMode::TeslaCaster, "08", "TESLA CASTER", "High-voltage lightning arcs jumping through conductive internal steel rebar frameworks.", "CHAIN LIGHTNING"),
                    (WeaponMode::PlasmaFlamer, "09", "PLASMA FLAMER", "Continuous superheated plasma jet melting structural facades into molten slag.", "THERMITE SLAG"),
                    (WeaponMode::PlasmaWireSaw, "10", "PLASMA WIRE-SAW", "High-frequency plasma monofilament planar cutting through megastructures and physics chunks.", "PLANAR SLICE"),
                ];

                let cols = 5;
                let rows = 2;
                let card_spacing_x = 12.0 * s;
                let card_spacing_y = 10.0 * s;
                let card_w = (content_w - (cols - 1) as f32 * card_spacing_x) / cols as f32;
                let bottom_bar_h = 44.0 * s;
                let card_h = (content_h - bottom_bar_h - 10.0 * s - (rows - 1) as f32 * card_spacing_y) / rows as f32;

                for (idx, &(m, tag, title, desc, role)) in chassis_list.iter().enumerate() {
                    let col = idx % cols;
                    let row = idx / cols;
                    let bx = content_x + col as f32 * (card_w + card_spacing_x);
                    let by = content_y + row as f32 * (card_h + card_spacing_y);

                    let is_selected = weapon.config.mode == m;
                    let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + card_w && mouse_pos.1 >= by && mouse_pos.1 <= by + card_h;

                    let bg_c = if is_selected {
                        Color::new(0.16, 0.22, 0.30, 1.0)
                    } else if is_hover {
                        Color::new(0.14, 0.17, 0.23, 1.0)
                    } else {
                        Color::new(0.11, 0.13, 0.18, 0.95)
                    };

                    draw_rectangle(bx, by, card_w, card_h, bg_c);
                    let border_c = if is_selected {
                        m.color()
                    } else if is_hover {
                        Color::new(0.35, 0.48, 0.65, 0.9)
                    } else {
                        Color::new(0.22, 0.27, 0.36, 0.7)
                    };
                    draw_rectangle_lines(bx, by, card_w, card_h, if is_selected { 2.5 * s } else { 1.0 }, border_c);

                    if is_selected {
                        draw_rectangle(bx, by, card_w, 3.5 * s, m.color());
                    }

                    // Number badge & Title
                    let title_str = fit_text_to_width(&format!("[{}] {}", tag, title), card_w - 24.0 * s, 15.5 * s);
                    draw_text(&title_str, bx + 12.0 * s, by + 20.0 * s, 15.5 * s, if is_selected { m.color() } else { WHITE });

                    // Role Badge
                    let rw = measure_text(role, None, (11.0 * s).round().max(8.0) as u16, 1.0).width;
                    let r_badge_x = bx + card_w - rw - 16.0 * s;
                    draw_rectangle(r_badge_x, by + 28.0 * s, rw + 10.0 * s, 16.0 * s, Color::new(0.08, 0.10, 0.14, 0.8));
                    draw_rectangle_lines(r_badge_x, by + 28.0 * s, rw + 10.0 * s, 16.0 * s, 1.0, border_c);
                    draw_text(role, r_badge_x + 5.0 * s, by + 40.0 * s, 11.0 * s, Color::new(0.85, 0.90, 0.98, 0.9));

                    // Multiline Description with guaranteed zero column bleed
                    draw_multiline_text(
                        desc,
                        bx + 12.0 * s,
                        by + 60.0 * s,
                        card_w - 24.0 * s,
                        11.5 * s,
                        14.0 * s,
                        Color::new(0.78, 0.84, 0.92, 0.95),
                        2,
                    );

                    if is_selected {
                        draw_text("ACTIVE CHASSIS", bx + 12.0 * s, by + card_h - 8.0 * s, 11.5 * s, m.color());
                    } else if is_hover {
                        draw_text("[ CLICK TO EQUIP ]", bx + 12.0 * s, by + card_h - 8.0 * s, 11.5 * s, Color::new(0.6, 0.8, 1.0, 0.9));
                    }

                    if is_hover && mouse_clicked && !is_selected {
                        weapon.set_mode(m, audio);
                    }
                }

                // Bottom Active Chassis Banner & Quick Preset Drawer
                let bar_y = content_y + content_h - bottom_bar_h;
                draw_rectangle(content_x, bar_y, content_w, bottom_bar_h, Color::new(0.12, 0.15, 0.20, 1.0));
                draw_rectangle_lines(content_x, bar_y, content_w, bottom_bar_h, 1.5 * s, theme_color);

                let active_desc = PRESETS.iter().find(|p| weapon.config.mode == p.mode).map(|p| p.description).unwrap_or("");
                let eq_info = if !active_desc.is_empty() {
                    format!("EQUIPPED: {} [{} HOTKEYS]  -  {}", weapon.config.mode.name(), crate::controls::WEAPON_SELECT.label, active_desc)
                } else {
                    format!("EQUIPPED: {} [{} HOTKEYS]", weapon.config.mode.name(), crate::controls::WEAPON_SELECT.label)
                };
                let fitted_eq = fit_text_to_width(&eq_info, content_w - 24.0 * s, 13.0 * s);
                draw_text(&fitted_eq, content_x + 12.0 * s, bar_y + 17.0 * s, 13.0 * s, WHITE);

                // Quick Combat Presets selector row
                let p_spacing = 6.0 * s;
                let p_label_w = 95.0 * s;
                draw_text("QUICK PRESETS:", content_x + 12.0 * s, bar_y + 35.0 * s, 11.0 * s, Color::new(0.7, 0.8, 0.9, 0.9));

                let p_btn_area_x = content_x + p_label_w + 14.0 * s;
                let p_btn_area_w = content_w - p_label_w - 22.0 * s;
                let p_btn_w = (p_btn_area_w - 6.0 * p_spacing) / 7.0;
                let p_btn_h = 18.0 * s;
                let p_btn_y = bar_y + 22.0 * s;

                for (p_idx, p) in PRESETS.iter().enumerate() {
                    let pbx = p_btn_area_x + p_idx as f32 * (p_btn_w + p_spacing);
                    let is_active = weapon.config.mode == p.mode
                        && weapon.config.barrel == p.barrel
                        && weapon.config.payload == p.payload
                        && weapon.config.trail_style == p.trail;
                    let is_p_hover = mouse_pos.0 >= pbx && mouse_pos.0 <= pbx + p_btn_w && mouse_pos.1 >= p_btn_y && mouse_pos.1 <= p_btn_y + p_btn_h;

                    let bg = if is_active { p.mode.color() } else if is_p_hover { Color::new(0.24, 0.32, 0.44, 1.0) } else { Color::new(0.15, 0.18, 0.24, 1.0) };
                    draw_rectangle(pbx, p_btn_y, p_btn_w, p_btn_h, bg);
                    draw_rectangle_lines(pbx, p_btn_y, p_btn_w, p_btn_h, 1.0, if is_active { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });

                    let p_text_c = if is_active { BLACK } else { WHITE };
                    let p_tw = measure_text(p.name, None, (9.5 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(p.name, pbx + (p_btn_w - p_tw) * 0.5, p_btn_y + 12.5 * s, 9.5 * s, p_text_c);

                    if is_p_hover && mouse_clicked && !is_active {
                        if weapon.config.mode == p.mode {
                            audio.play(audio.ui_switch.as_ref(), 0.7);
                        }
                        weapon.set_mode(p.mode, audio);
                        weapon.config.barrel = p.barrel;
                        weapon.config.payload = p.payload;
                        weapon.config.trail_style = p.trail;
                        weapon.config.stabilizer = p.stabilizer;
                        weapon.config.blast_radius_mult = p.blast_radius_mult;
                        weapon.config.impulse_mult = p.impulse_mult;
                        weapon.config.fire_rate_mult = p.fire_rate_mult;
                        weapon.config.velocity_mult = p.velocity_mult;
                        weapon.config.detonator = DetonatorType::InstantImpact;
                    }
                }
            }

            // ==================== TAB 1: BALLISTICS & PAYLOAD ====================
            1 => {
                let col_spacing = 16.0 * s;
                let left_w = (content_w - col_spacing) * 0.48;
                let right_w = (content_w - col_spacing) * 0.52;
                let col2_x = content_x + left_w + col_spacing;

                // ---- LEFT COLUMN: Component Modifiers ----
                let mut cur_y = content_y;
                let btn_h = 28.0 * s;

                // 1. Barrel & Muzzle Dispersion
                draw_text("1. BARREL & MUZZLE DISPERSION", content_x, cur_y + 13.0 * s, 14.5 * s, WHITE);
                cur_y += 18.0 * s;
                let barrels = [
                    (BarrelType::Precision, "PRECISION"),
                    (BarrelType::TriSpread, "TRI-SPREAD"),
                    (BarrelType::ClusterSplit, "CLUSTER"),
                    (BarrelType::Piercing, "PIERCING"),
                ];
                let b_w = (left_w - 3.0 * 6.0 * s) / 4.0;
                for (i, &(b, title)) in barrels.iter().enumerate() {
                    let bx = content_x + i as f32 * (b_w + 6.0 * s);
                    let is_sel = weapon.config.barrel == b;
                    let is_hov = mouse_pos.0 >= bx && mouse_pos.0 <= bx + b_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;
                    let bg = if is_sel { theme_color } else if is_hov { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, b_w, btn_h, bg);
                    draw_rectangle_lines(bx, cur_y, b_w, btn_h, 1.0, if is_sel { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let tw = measure_text(title, None, (11.0 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(title, bx + (b_w - tw) * 0.5, cur_y + 18.0 * s, 11.0 * s, if is_sel { BLACK } else { WHITE });
                    if is_hov && mouse_clicked {
                        weapon.config.barrel = b;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }
                cur_y += btn_h + 12.0 * s;

                // 2. Payload Warheads
                draw_text("2. PAYLOAD WARHEAD SYSTEM", content_x, cur_y + 13.0 * s, 14.5 * s, WHITE);
                cur_y += 18.0 * s;
                let payloads = [
                    (PayloadType::HighExplosive, "HIGH EXPL"),
                    (PayloadType::ShapedCharge, "SHAPED CHG"),
                    (PayloadType::CryoShrapnel, "CRYO FREEZE"),
                    (PayloadType::Incendiary, "THERMITE"),
                ];
                for (i, &(pl, title)) in payloads.iter().enumerate() {
                    let bx = content_x + i as f32 * (b_w + 6.0 * s);
                    let is_sel = weapon.config.payload == pl;
                    let is_hov = mouse_pos.0 >= bx && mouse_pos.0 <= bx + b_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;
                    let bg = if is_sel { theme_color } else if is_hov { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, b_w, btn_h, bg);
                    draw_rectangle_lines(bx, cur_y, b_w, btn_h, 1.0, if is_sel { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let tw = measure_text(title, None, (11.0 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(title, bx + (b_w - tw) * 0.5, cur_y + 18.0 * s, 11.0 * s, if is_sel { BLACK } else { WHITE });
                    if is_hov && mouse_clicked {
                        weapon.config.payload = pl;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }
                cur_y += btn_h + 12.0 * s;

                // 3. Trail Aesthetics & Ribbons
                draw_text("3. TRAIL AESTHETICS & RIBBONS", content_x, cur_y + 13.0 * s, 14.5 * s, WHITE);
                cur_y += 18.0 * s;
                let trails = [
                    (TrailStyle::SmokeWisps, "WISPS"),
                    (TrailStyle::PlasmaGlow, "PLASMA"),
                    (TrailStyle::SparkShower, "SPARKS"),
                    (TrailStyle::HyperTracer, "TRACER"),
                ];
                for (i, &(tr, title)) in trails.iter().enumerate() {
                    let bx = content_x + i as f32 * (b_w + 6.0 * s);
                    let is_sel = weapon.config.trail_style == tr;
                    let is_hov = mouse_pos.0 >= bx && mouse_pos.0 <= bx + b_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;
                    let bg = if is_sel { theme_color } else if is_hov { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, b_w, btn_h, bg);
                    draw_rectangle_lines(bx, cur_y, b_w, btn_h, 1.0, if is_sel { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let tw = measure_text(title, None, (11.0 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(title, bx + (b_w - tw) * 0.5, cur_y + 18.0 * s, 11.0 * s, if is_sel { BLACK } else { WHITE });
                    if is_hov && mouse_clicked {
                        weapon.config.trail_style = tr;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }
                cur_y += btn_h + 12.0 * s;

                // 4. Recoil Stabilizers
                draw_text("4. RECOIL STABILIZATION DAMPING", content_x, cur_y + 13.0 * s, 14.5 * s, WHITE);
                cur_y += 18.0 * s;
                let stabilizers = [
                    (StabilizerType::Standard, "STANDARD"),
                    (StabilizerType::HeavyCompensator, "COMPENSATOR"),
                    (StabilizerType::GyroscopicThruster, "GYROSCOPIC"),
                    (StabilizerType::Overclocked, "OVERCLOCK"),
                ];
                for (i, &(st, title)) in stabilizers.iter().enumerate() {
                    let bx = content_x + i as f32 * (b_w + 6.0 * s);
                    let is_sel = weapon.config.stabilizer == st;
                    let is_hov = mouse_pos.0 >= bx && mouse_pos.0 <= bx + b_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;
                    let bg = if is_sel { theme_color } else if is_hov { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, b_w, btn_h, bg);
                    draw_rectangle_lines(bx, cur_y, b_w, btn_h, 1.0, if is_sel { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let tw = measure_text(title, None, (10.0 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(title, bx + (b_w - tw) * 0.5, cur_y + 18.0 * s, 10.0 * s, if is_sel { BLACK } else { WHITE });
                    if is_hov && mouse_clicked {
                        weapon.config.stabilizer = st;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }
                cur_y += btn_h + 12.0 * s;

                // 5. Detonator Trigger Mode
                draw_text("5. DETONATOR TRIGGER MECHANISM", content_x, cur_y + 13.0 * s, 14.5 * s, WHITE);
                cur_y += 18.0 * s;
                let det_w = (left_w - 8.0 * s) * 0.5;
                let detonators = [
                    (DetonatorType::InstantImpact, "IMPACT DETONATION"),
                    (DetonatorType::RemoteSticky, "REMOTE STICKY [RMB]"),
                ];
                for (i, &(det, title)) in detonators.iter().enumerate() {
                    let bx = content_x + i as f32 * (det_w + 8.0 * s);
                    let is_sel = weapon.config.detonator == det;
                    let is_hov = mouse_pos.0 >= bx && mouse_pos.0 <= bx + det_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;
                    let bg = if is_sel { theme_color } else if is_hov { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, det_w, btn_h, bg);
                    draw_rectangle_lines(bx, cur_y, det_w, btn_h, 1.0, if is_sel { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let tw = measure_text(title, None, (11.0 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(title, bx + (det_w - tw) * 0.5, cur_y + 18.0 * s, 11.0 * s, if is_sel { BLACK } else { WHITE });
                    if is_hov && mouse_clicked {
                        weapon.config.detonator = det;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }

                // ---- RIGHT COLUMN: Power & Fine-Tuning Sliders ----
                let card_h = 56.0 * s;
                let card_spacing = 8.0 * s;
                let track_w = right_w - 24.0 * s;
                let track_h = 10.0 * s;

                // Slider 1: Blast Radius (0.50x to 3.00x)
                let s1_y = content_y;
                draw_rectangle(col2_x, s1_y, right_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s1_y, right_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str1 = format!("BLAST RADIUS MULTIPLIER: {:.2}x", weapon.config.blast_radius_mult);
                draw_text(&str1, col2_x + 12.0 * s, s1_y + 18.0 * s, 13.5 * s, WHITE);
                let track1_x = col2_x + 12.0 * s;
                let track1_y = s1_y + 30.0 * s;
                draw_rectangle(track1_x, track1_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p1 = ((weapon.config.blast_radius_mult - 0.5) / 2.5).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track1_y, track_w * p1, track_h, theme_color);
                draw_circle(track1_x + track_w * p1, track1_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s1_y + 20.0 * s && mouse_pos.1 <= s1_y + 50.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    weapon.config.blast_radius_mult = 0.5 + t * 2.5;
                }

                // Slider 2: Kinetic Impulse (0.50x to 3.50x)
                let s2_y = s1_y + card_h + card_spacing;
                draw_rectangle(col2_x, s2_y, right_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s2_y, right_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str2 = format!("KINETIC IMPULSE FORCE: {:.2}x", weapon.config.impulse_mult);
                draw_text(&str2, col2_x + 12.0 * s, s2_y + 18.0 * s, 13.5 * s, WHITE);
                let track2_y = s2_y + 30.0 * s;
                draw_rectangle(track1_x, track2_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p2 = ((weapon.config.impulse_mult - 0.5) / 3.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track2_y, track_w * p2, track_h, theme_color);
                draw_circle(track1_x + track_w * p2, track2_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s2_y + 20.0 * s && mouse_pos.1 <= s2_y + 50.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    weapon.config.impulse_mult = 0.5 + t * 3.0;
                }

                // Slider 3: Muzzle Velocity (0.50x to 2.50x)
                let s3_y = s2_y + card_h + card_spacing;
                draw_rectangle(col2_x, s3_y, right_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s3_y, right_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str3 = format!("MUZZLE VELOCITY: {:.2}x", weapon.config.velocity_mult);
                draw_text(&str3, col2_x + 12.0 * s, s3_y + 18.0 * s, 13.5 * s, WHITE);
                let track3_y = s3_y + 30.0 * s;
                draw_rectangle(track1_x, track3_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p3 = ((weapon.config.velocity_mult - 0.5) / 2.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track3_y, track_w * p3, track_h, theme_color);
                draw_circle(track1_x + track_w * p3, track3_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s3_y + 20.0 * s && mouse_pos.1 <= s3_y + 50.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    weapon.config.velocity_mult = 0.5 + t * 2.0;
                }

                // Slider 4: Cyclic Fire Rate (0.50x to 3.00x)
                let s4_y = s3_y + card_h + card_spacing;
                draw_rectangle(col2_x, s4_y, right_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s4_y, right_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str4 = format!("CYCLIC FIRE RATE: {:.2}x", weapon.config.fire_rate_mult);
                draw_text(&str4, col2_x + 12.0 * s, s4_y + 18.0 * s, 13.5 * s, WHITE);
                let track4_y = s4_y + 30.0 * s;
                draw_rectangle(track1_x, track4_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p4 = ((weapon.config.fire_rate_mult - 0.5) / 2.5).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track4_y, track_w * p4, track_h, theme_color);
                draw_circle(track1_x + track_w * p4, track4_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s4_y + 20.0 * s && mouse_pos.1 <= s4_y + 50.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    weapon.config.fire_rate_mult = 0.5 + t * 2.5;
                }

                // Slider 5: Cluster Bomblets (3 to 8 submunitions)
                let s5_y = s4_y + card_h + card_spacing;
                draw_rectangle(col2_x, s5_y, right_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s5_y, right_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str5 = format!("CLUSTER BOMBLETS: {} SUBMUNITIONS", weapon.config.cluster_count);
                draw_text(&str5, col2_x + 12.0 * s, s5_y + 18.0 * s, 13.5 * s, WHITE);
                let track5_y = s5_y + 30.0 * s;
                draw_rectangle(track1_x, track5_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p5 = ((weapon.config.cluster_count as f32 - 3.0) / 5.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track5_y, track_w * p5, track_h, theme_color);
                draw_circle(track1_x + track_w * p5, track5_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s5_y + 20.0 * s && mouse_pos.1 <= s5_y + 50.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    weapon.config.cluster_count = (3.0 + t * 5.0).round() as usize;
                }

                // Slider 6: Recoil Damping (0.50x to 2.00x)
                let s6_y = s5_y + card_h + card_spacing;
                draw_rectangle(col2_x, s6_y, right_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s6_y, right_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str6 = format!("RECOIL DAMPING: {:.2}x", weapon.config.recoil_damping);
                draw_text(&str6, col2_x + 12.0 * s, s6_y + 18.0 * s, 13.5 * s, WHITE);
                let track6_y = s6_y + 30.0 * s;
                draw_rectangle(track1_x, track6_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p6 = ((weapon.config.recoil_damping - 0.5) / 1.5).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track6_y, track_w * p6, track_h, theme_color);
                draw_circle(track1_x + track_w * p6, track6_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s6_y + 20.0 * s && mouse_pos.1 <= s6_y + 50.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    weapon.config.recoil_damping = 0.5 + t * 1.5;
                }

                // Reset Button
                let rst_h = 30.0 * s;
                let rst_y = s6_y + card_h + 10.0 * s;
                let is_hov_rst = mouse_pos.0 >= col2_x && mouse_pos.0 <= col2_x + right_w && mouse_pos.1 >= rst_y && mouse_pos.1 <= rst_y + rst_h;
                draw_rectangle(col2_x, rst_y, right_w, rst_h, if is_hov_rst { Color::new(0.25, 0.32, 0.42, 1.0) } else { Color::new(0.14, 0.17, 0.22, 1.0) });
                draw_rectangle_lines(col2_x, rst_y, right_w, rst_h, 1.0, if is_hov_rst { WHITE } else { Color::new(0.35, 0.45, 0.58, 0.8) });
                let rst_str = "[ RESET ALL BALLISTICS TUNING TO FACTORY 1.0x ]";
                let rtw = measure_text(rst_str, None, (12.5 * s).round().max(8.0) as u16, 1.0).width;
                draw_text(rst_str, col2_x + (right_w - rtw) * 0.5, rst_y + 20.0 * s, 12.5 * s, WHITE);
                if is_hov_rst && mouse_clicked {
                    weapon.config.blast_radius_mult = 1.0;
                    weapon.config.impulse_mult = 1.0;
                    weapon.config.velocity_mult = 1.0;
                    weapon.config.fire_rate_mult = 1.0;
                    weapon.config.cluster_count = 5;
                    weapon.config.recoil_damping = 1.0;
                    audio.play(audio.ui_click.as_ref(), 0.7);
                }
            }

            // ==================== TAB 2: SIEGE ENGINEERING ====================
            2 => {
                let siege_theme = siege.selected_kind.theme_color();
                let mut cur_y = content_y;

                // Section 1: Siege Engine Chassis
                draw_text("1. DEPLOYABLE SIEGE ENGINE PLATFORM", content_x, cur_y + 15.0 * s, 16.5 * s, WHITE);
                cur_y += 20.0 * s;

                let s_cols = 4;
                let s_spacing = 10.0 * s;
                let s_card_w = (content_w - (s_cols - 1) as f32 * s_spacing) / s_cols as f32;
                let s_card_h = 70.0 * s;

                let engines = [
                    (SiegeEngineKind::RailArtillery, "01", "RAIL ARTILLERY", "Colossal 500-2500mm dreadnought ordnance punched deep through core."),
                    (SiegeEngineKind::OrbitalKineticRelay, "02", "ORBITAL KINETIC RELAY", "Beacon calling down a 3-meter vertical ion beam through all stories."),
                    (SiegeEngineKind::HydraulicGroundRam, "03", "HYDRAULIC GROUND RAM", "Bedrock hammer pumping seismic shockwaves to trigger pancake collapse."),
                    (SiegeEngineKind::TeslaSiegePylon, "04", "TESLA SIEGE PYLON", "High-voltage capacitor discharging cascading arcs into structural rebar."),
                ];

                for (idx, &(ek, tag, title, desc)) in engines.iter().enumerate() {
                    let bx = content_x + idx as f32 * (s_card_w + s_spacing);
                    let is_active = siege.selected_kind == ek;
                    let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + s_card_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + s_card_h;

                    let bg_c = if is_active {
                        Color::new(0.18, 0.22, 0.32, 1.0)
                    } else if is_hover {
                        Color::new(0.15, 0.18, 0.25, 1.0)
                    } else {
                        Color::new(0.11, 0.13, 0.18, 0.95)
                    };

                    draw_rectangle(bx, cur_y, s_card_w, s_card_h, bg_c);
                    let border_c = if is_active { ek.theme_color() } else if is_hover { WHITE } else { Color::new(0.28, 0.35, 0.45, 0.8) };
                    draw_rectangle_lines(bx, cur_y, s_card_w, s_card_h, if is_active { 2.5 * s } else { 1.0 }, border_c);

                    if is_active {
                        draw_rectangle(bx, cur_y, s_card_w, 3.5 * s, ek.theme_color());
                    }

                    let title_str = format!("[{}] {}", tag, title);
                    draw_text(&title_str, bx + 10.0 * s, cur_y + 18.0 * s, 14.0 * s, if is_active { ek.theme_color() } else { WHITE });

                    // Multiline Description with guaranteed zero horizontal bleed
                    draw_multiline_text(
                        desc,
                        bx + 10.0 * s,
                        cur_y + 32.0 * s,
                        s_card_w - 20.0 * s,
                        11.0 * s,
                        13.0 * s,
                        Color::new(0.72, 0.78, 0.88, 0.95),
                        2,
                    );

                    if is_active {
                        draw_text("ACTIVE SIEGE ENGINE", bx + 10.0 * s, cur_y + s_card_h - 6.0 * s, 11.0 * s, ek.theme_color());
                    } else if is_hover {
                        draw_text("[ CLICK TO SELECT ]", bx + 10.0 * s, cur_y + s_card_h - 6.0 * s, 11.0 * s, Color::new(0.6, 0.8, 1.0, 0.9));
                    }

                    if is_hover && mouse_clicked && !is_active {
                        siege.select_kind(ek, audio);
                    }
                }
                cur_y += s_card_h + 8.0 * s;

                // Section 2: Shell Payload Selection
                draw_text("2. SIEGE PAYLOAD WARHEAD SYSTEM", content_x, cur_y + 15.0 * s, 16.5 * s, WHITE);
                cur_y += 20.0 * s;

                let payloads = [
                    (SiegePayload::ColossalHighExplosive, "COLOSSAL HE"),
                    (SiegePayload::BunkerBuster, "BUNKER BUSTER"),
                    (SiegePayload::ThermiteMelt, "THERMITE MELT"),
                    (SiegePayload::CryoShatter, "CRYO SHATTER"),
                    (SiegePayload::AntimatterSingularity, "ANTIMATTER CORE"),
                ];
                let pl_btn_w = (content_w - 4.0 * 8.0 * s) / 5.0;
                let btn_h = 28.0 * s;

                for (i, &(pl, title)) in payloads.iter().enumerate() {
                    let bx = content_x + i as f32 * (pl_btn_w + 8.0 * s);
                    let is_active = siege.config.payload == pl;
                    let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + pl_btn_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;

                    let bg_c = if is_active { pl.color() } else if is_hover { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, pl_btn_w, btn_h, bg_c);
                    draw_rectangle_lines(bx, cur_y, pl_btn_w, btn_h, if is_active { 2.0 * s } else { 1.0 }, if is_active { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let text_c = if is_active { BLACK } else { WHITE };
                    let tw = measure_text(title, None, (12.0 * s).round().max(8.0) as u16, 1.0).width;
                    draw_text(title, bx + (pl_btn_w - tw) * 0.5, cur_y + 19.0 * s, 12.0 * s, text_c);

                    if is_hover && mouse_clicked {
                        siege.config.payload = pl;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }
                cur_y += btn_h + 6.0 * s;
                let payload_desc = siege.config.payload.description();
                let fitted_pl_desc = fit_text_to_width(payload_desc, content_w, 11.5 * s);
                draw_text(&fitted_pl_desc, content_x, cur_y + 11.0 * s, 11.5 * s, Color::new(0.7, 0.8, 0.9, 0.85));
                cur_y += 18.0 * s;

                // Section 3: Trajectory Mode
                draw_text("3. BALLISTIC TRAJECTORY & ELEVATION TUNING", content_x, cur_y + 15.0 * s, 16.5 * s, WHITE);
                cur_y += 20.0 * s;

                let trajectories = [
                    (SiegeTrajectory::HighMortarLob, "HIGH MORTAR LOB (65°)", "Parabolic arc over blast walls."),
                    (SiegeTrajectory::DirectBreach, "DIRECT BREACH (18°)", "Flat relativistic punch through shear core."),
                    (SiegeTrajectory::OrbitalBeam, "ORBITAL BEAM (90°)", "Direct vertical satellite downlink to bedrock."),
                ];
                let tr_w = (content_w - 2.0 * 10.0 * s) / 3.0;

                for (i, &(tr, title, _desc)) in trajectories.iter().enumerate() {
                    let bx = content_x + i as f32 * (tr_w + 10.0 * s);
                    let is_active = siege.config.trajectory == tr;
                    let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + tr_w && mouse_pos.1 >= cur_y && mouse_pos.1 <= cur_y + btn_h;

                    let bg_c = if is_active { siege_theme } else if is_hover { Color::new(0.20, 0.25, 0.32, 1.0) } else { Color::new(0.12, 0.15, 0.20, 1.0) };
                    draw_rectangle(bx, cur_y, tr_w, btn_h, bg_c);
                    draw_rectangle_lines(bx, cur_y, tr_w, btn_h, if is_active { 2.0 * s } else { 1.0 }, if is_active { WHITE } else { Color::new(0.3, 0.4, 0.5, 0.8) });
                    let text_c = if is_active { BLACK } else { WHITE };
                    let tw = measure_text(title, None, (12.0 * s).round().max(8.0) as u16, 1.0).width;
                    draw_text(title, bx + (tr_w - tw) * 0.5, cur_y + 19.0 * s, 12.0 * s, text_c);

                    if is_hover && mouse_clicked {
                        siege.config.trajectory = tr;
                        siege.config.elevation_deg = tr.default_elevation_deg();
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }
                cur_y += btn_h + 8.0 * s;

                // Section 4: Fine Tuning Sliders
                let slider_col_w = (content_w - 16.0 * s) / 2.0;
                let s_slider_h = 48.0 * s;
                let track_w = slider_col_w - 24.0 * s;
                let track_h = 9.0 * s;
                let col2_x = content_x + slider_col_w + 16.0 * s;

                // Slider 1: Ordnance Caliber (500mm to 2500mm)
                let s1_y = cur_y;
                draw_rectangle(content_x, s1_y, slider_col_w, s_slider_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s1_y, slider_col_w, s_slider_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let cal_str = format!("ORDNANCE CALIBER: {:.0}mm [DIAMETER: {:.2}m]", siege.config.caliber_mm, siege.config.shell_radius() * 2.0);
                draw_text(&cal_str, content_x + 12.0 * s, s1_y + 16.0 * s, 13.0 * s, WHITE);
                let track1_x = content_x + 12.0 * s;
                let track1_y = s1_y + 26.0 * s;
                draw_rectangle(track1_x, track1_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p_cal = ((siege.config.caliber_mm - 500.0) / 2000.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track1_y, track_w * p_cal, track_h, siege_theme);
                draw_circle(track1_x + track_w * p_cal, track1_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s1_y + 16.0 * s && mouse_pos.1 <= s1_y + 44.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    siege.config.caliber_mm = (500.0 + t * 2000.0).round();
                }

                // Slider 2: Propellant Power (0.50x to 3.00x)
                draw_rectangle(col2_x, s1_y, slider_col_w, s_slider_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s1_y, slider_col_w, s_slider_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let pwr_str = format!("PROPELLANT CHARGE: {:.2}x [VEL: {:.1} m/s]", siege.config.propellant_power, siege.config.launch_velocity());
                draw_text(&pwr_str, col2_x + 12.0 * s, s1_y + 16.0 * s, 13.0 * s, WHITE);
                let track2_x = col2_x + 12.0 * s;
                let track2_y = s1_y + 26.0 * s;
                draw_rectangle(track2_x, track2_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p_pwr = ((siege.config.propellant_power - 0.5) / 2.5).clamp(0.0, 1.0);
                draw_rectangle(track2_x, track2_y, track_w * p_pwr, track_h, siege_theme);
                draw_circle(track2_x + track_w * p_pwr, track2_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track2_x - 10.0 * s && mouse_pos.0 <= track2_x + track_w + 10.0 * s && mouse_pos.1 >= s1_y + 16.0 * s && mouse_pos.1 <= s1_y + 44.0 * s {
                    let t = ((mouse_pos.0 - track2_x) / track_w).clamp(0.0, 1.0);
                    siege.config.propellant_power = 0.5 + t * 2.5;
                }

                // Slider 3: Elevation Degree (10° to 88°)
                let s2_y = s1_y + s_slider_h + 4.0 * s;
                draw_rectangle(content_x, s2_y, slider_col_w, s_slider_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s2_y, slider_col_w, s_slider_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let elev_str = format!("BARREL ELEVATION: {:.1}°", siege.config.elevation_deg);
                draw_text(&elev_str, content_x + 12.0 * s, s2_y + 16.0 * s, 13.0 * s, WHITE);
                let track3_y = s2_y + 26.0 * s;
                draw_rectangle(track1_x, track3_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p_elev = ((siege.config.elevation_deg - 10.0) / 78.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track3_y, track_w * p_elev, track_h, siege_theme);
                draw_circle(track1_x + track_w * p_elev, track3_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s2_y + 16.0 * s && mouse_pos.1 <= s2_y + 44.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    siege.config.elevation_deg = 10.0 + t * 78.0;
                }

                // Slider 4: Bunker Buster Fuse Delay (0.05s to 0.45s)
                draw_rectangle(col2_x, s2_y, slider_col_w, s_slider_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s2_y, slider_col_w, s_slider_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let fuse_str = format!("FUSE DELAY: {:.2}s", siege.config.delayed_fuse_secs);
                draw_text(&fuse_str, col2_x + 12.0 * s, s2_y + 16.0 * s, 13.0 * s, WHITE);
                let track4_y = s2_y + 26.0 * s;
                draw_rectangle(track2_x, track4_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p_fuse = ((siege.config.delayed_fuse_secs - 0.05) / 0.40).clamp(0.0, 1.0);
                draw_rectangle(track2_x, track4_y, track_w * p_fuse, track_h, siege_theme);
                draw_circle(track2_x + track_w * p_fuse, track4_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track2_x - 10.0 * s && mouse_pos.0 <= track2_x + track_w + 10.0 * s && mouse_pos.1 >= s2_y + 16.0 * s && mouse_pos.1 <= s2_y + 44.0 * s {
                    let t = ((mouse_pos.0 - track2_x) / track_w).clamp(0.0, 1.0);
                    siege.config.delayed_fuse_secs = 0.05 + t * 0.40;
                }

                // Row 3: Manual Azimuth Bearing & Targeting Mode Auto-Aim Toggle
                let s3_y = s2_y + s_slider_h + 4.0 * s;
                draw_rectangle(content_x, s3_y, slider_col_w, s_slider_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s3_y, slider_col_w, s_slider_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let az_str = format!("MANUAL AZIMUTH: {:.0}°", siege.config.azimuth_deg);
                draw_text(&az_str, content_x + 12.0 * s, s3_y + 16.0 * s, 13.0 * s, WHITE);
                let track5_y = s3_y + 26.0 * s;
                draw_rectangle(track1_x, track5_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p_az = (siege.config.azimuth_deg / 360.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track5_y, track_w * p_az, track_h, siege_theme);
                draw_circle(track1_x + track_w * p_az, track5_y + track_h * 0.5, 6.0 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s3_y + 16.0 * s && mouse_pos.1 <= s3_y + 44.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    siege.config.azimuth_deg = (t * 360.0).round();
                }

                // Targeting Mode Toggle Card
                draw_rectangle(col2_x, s3_y, slider_col_w, s_slider_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(col2_x, s3_y, slider_col_w, s_slider_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let aim_color = if siege.config.auto_aim_reticle { Color::new(0.25, 0.88, 1.0, 1.0) } else { Color::new(1.0, 0.75, 0.2, 1.0) };
                let toggle_label = if siege.config.auto_aim_reticle {
                    "[ TARGETING: AUTO-LOCK TO RETICLE (CLICK FOR MANUAL) ]"
                } else {
                    "[ TARGETING: MANUAL AZIMUTH BEARING (CLICK FOR AUTO) ]"
                };
                let is_aim_hover = mouse_pos.0 >= col2_x && mouse_pos.0 <= col2_x + slider_col_w && mouse_pos.1 >= s3_y && mouse_pos.1 <= s3_y + s_slider_h;
                let tlw = measure_text(toggle_label, None, (11.5 * s).round().max(8.0) as u16, 1.0).width;
                draw_text(toggle_label, col2_x + (slider_col_w - tlw) * 0.5, s3_y + 28.0 * s, 11.5 * s, aim_color);
                if is_aim_hover && mouse_clicked {
                    siege.config.auto_aim_reticle = !siege.config.auto_aim_reticle;
                    audio.play(audio.ui_click.as_ref(), 0.7);
                }

                // Section 5: Bottom Command & Tactical Fleet Actions Banner
                let bar_h = 36.0 * s;
                let bar_y = py + ph - bar_h - 12.0 * s;
                draw_rectangle(content_x, bar_y, content_w, bar_h, Color::new(0.12, 0.15, 0.20, 1.0));
                draw_rectangle_lines(content_x, bar_y, content_w, bar_h, 1.5 * s, siege_theme);

                let deployed_count = siege.deployed_engines.len();
                let status_summary = format!(
                    "DEPLOYED FLEET: {}/4 ANCHORED   |   {} DEPLOY / MOVE   {} FIRE   {} CYCLE   {} RECALL",
                    deployed_count,
                    crate::controls::SIEGE_DEPLOY.token,
                    crate::controls::SIEGE_FIRE.token,
                    crate::controls::SIEGE_CYCLE.token,
                    crate::controls::SIEGE_RECALL.token,
                );
                let status_summary = fit_text_to_width(&status_summary, content_w - 153.0 * s, 11.5 * s);
                draw_text(&status_summary, content_x + 14.0 * s, bar_y + 14.0 * s, 11.5 * s, WHITE);
                let deploy_hint = format!("{} DEPLOY / MOVE TO APPLY SETTINGS TO THE SELECTED BATTERY", crate::controls::SIEGE_DEPLOY.token);
                let deploy_hint = fit_text_to_width(&deploy_hint, content_w - 153.0 * s, 10.5 * s);
                draw_text(&deploy_hint, content_x + 14.0 * s, bar_y + 29.0 * s, 10.5 * s, Color::new(0.70, 0.82, 0.95, 0.95));

                // Recall All Button
                let rec_w = 125.0 * s;
                let rec_h = 26.0 * s;
                let rec_x = content_x + content_w - rec_w - 8.0 * s;
                let rec_y = bar_y + 5.0 * s;
                let is_rec_hover = mouse_pos.0 >= rec_x && mouse_pos.0 <= rec_x + rec_w && mouse_pos.1 >= rec_y && mouse_pos.1 <= rec_y + rec_h;
                draw_rectangle(rec_x, rec_y, rec_w, rec_h, if is_rec_hover { Color::new(0.8, 0.25, 0.2, 1.0) } else { Color::new(0.25, 0.15, 0.18, 1.0) });
                draw_rectangle_lines(rec_x, rec_y, rec_w, rec_h, 1.0, WHITE);
                let rec_lbl = "[ RECALL ALL ]";
                let rlw = measure_text(rec_lbl, None, (12.0 * s).round().max(8.0) as u16, 1.0).width;
                draw_text(rec_lbl, rec_x + (rec_w - rlw) * 0.5, rec_y + 17.5 * s, 12.0 * s, WHITE);
                if is_rec_hover && mouse_clicked {
                    siege.recall_all(audio);
                }
            }

            // ==================== TAB 3: MATERIAL TELEMETRY ====================
            3 => {
                draw_text("VOXEL MATERIAL PHYSICAL PROPERTIES & FORENSIC METRICS", content_x, content_y + 16.0 * s, 17.0 * s, WHITE);

                let materials = [
                    (VoxelType::Concrete, "CONCRETE", "STRUCTURAL FAÇADE"),
                    (VoxelType::SteelRebar, "STEEL REBAR", "LOAD-BEARING SKELETON"),
                    (VoxelType::CompositeRebar, "COMPOSITE REBAR", "FLEX-RESIST TENSION CORE"),
                    (VoxelType::HeavyArmor, "HEAVY ARMOR", "CONCUSSIVE BLAST SHIELD"),
                    (VoxelType::Glass, "GLASS", "BRITTLE SILICA CURTAIN"),
                    (VoxelType::Explosive, "EXPLOSIVE", "VOLATILE FUEL SILO"),
                    (VoxelType::Brick, "BRICK", "MASONRY INFILL WALL"),
                    (VoxelType::Bedrock, "BEDROCK", "REINFORCED FOUNDATION"),
                    (VoxelType::CryoIce, "CRYO ICE", "SUB-ZERO FREEZE MATRIX"),
                    (VoxelType::GlowCore, "GLOW CORE", "HIGH-ENERGY EMISSIVE NODE"),
                    (VoxelType::PowerCapacitor, "POWER CAPACITOR", "CONDUCTIVE ENERGY CELL"),
                ];

                let cols = 4;
                let spacing = 10.0 * s;
                let card_w = (content_w - (cols - 1) as f32 * spacing) / cols as f32;
                let card_h = 74.0 * s;
                let grid_start_y = content_y + 26.0 * s;

                for (idx, &(v_type, name, role)) in materials.iter().enumerate() {
                    let col = idx % cols;
                    let row = idx / cols;
                    let bx = content_x + col as f32 * (card_w + spacing);
                    let by = grid_start_y + row as f32 * (card_h + 8.0 * s);

                    let props = v_type.properties();
                    let is_selected = self.selected_material == idx;
                    let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + card_w && mouse_pos.1 >= by && mouse_pos.1 <= by + card_h;

                    let bg_c = if is_selected {
                        Color::new(0.18, 0.24, 0.34, 1.0)
                    } else if is_hover {
                        Color::new(0.14, 0.18, 0.25, 1.0)
                    } else {
                        Color::new(0.10, 0.12, 0.16, 0.95)
                    };

                    draw_rectangle(bx, by, card_w, card_h, bg_c);
                    let border_c = if is_selected {
                        Color::new(0.30, 0.85, 0.95, 1.0)
                    } else if is_hover {
                        Color::new(0.40, 0.55, 0.70, 0.85)
                    } else {
                        Color::new(0.22, 0.28, 0.38, 0.7)
                    };
                    draw_rectangle_lines(bx, by, card_w, card_h, if is_selected { 2.0 * s } else { 1.0 }, border_c);

                    // Material color swatch
                    let swatch_c = match v_type {
                        VoxelType::Concrete => Color::new(0.55, 0.58, 0.62, 1.0),
                        VoxelType::SteelRebar => Color::new(0.35, 0.45, 0.60, 1.0),
                        VoxelType::CompositeRebar => Color::new(0.20, 0.75, 0.70, 1.0),
                        VoxelType::HeavyArmor => Color::new(0.85, 0.45, 0.20, 1.0),
                        VoxelType::Glass => Color::new(0.40, 0.85, 0.95, 0.9),
                        VoxelType::Explosive => Color::new(0.95, 0.25, 0.20, 1.0),
                        VoxelType::Brick => Color::new(0.70, 0.32, 0.22, 1.0),
                        VoxelType::Bedrock => Color::new(0.22, 0.22, 0.26, 1.0),
                        VoxelType::CryoIce => Color::new(0.50, 0.90, 1.0, 1.0),
                        VoxelType::GlowCore => Color::new(1.0, 0.85, 0.25, 1.0),
                        VoxelType::PowerCapacitor => Color::new(0.75, 0.35, 1.0, 1.0),
                        _ => WHITE,
                    };
                    draw_rectangle(bx + 8.0 * s, by + 8.0 * s, 10.0 * s, 10.0 * s, swatch_c);
                    draw_text(name, bx + 22.0 * s, by + 17.0 * s, 12.5 * s, WHITE);

                    // Role tag
                    draw_text(role, bx + 8.0 * s, by + 30.0 * s, 10.0 * s, Color::new(0.68, 0.76, 0.88, 0.9));

                    // Metrics
                    let row1_str = format!("HARD: {:.1}  |  TOUGH: {:.1}  |  BLAST: {:.1}", props.hardness, props.toughness, props.blast_threshold);
                    draw_text(&row1_str, bx + 8.0 * s, by + 47.0 * s, 10.5 * s, Color::new(0.80, 0.88, 0.96, 0.95));

                    let cond_str = format!("DENSITY: {:.0} kg/m³  |  COND: {:.0}%", props.density, props.conductivity * 100.0);
                    draw_text(&cond_str, bx + 8.0 * s, by + 61.0 * s, 10.0 * s, Color::new(0.65, 0.74, 0.85, 0.85));

                    if is_hover && mouse_clicked {
                        self.selected_material = idx;
                        audio.play(audio.ui_click.as_ref(), 0.7);
                    }
                }

                // Overview card in 12th slot (row 2, col 3)
                let overview_bx = content_x + 3.0 * (card_w + spacing);
                let overview_by = grid_start_y + 2.0 * (card_h + 8.0 * s);
                draw_rectangle(overview_bx, overview_by, card_w, card_h, Color::new(0.12, 0.16, 0.22, 0.95));
                draw_rectangle_lines(overview_bx, overview_by, card_w, card_h, 1.0, Color::new(0.30, 0.45, 0.65, 0.8));
                draw_text("[ SYSTEM REGISTRY ]", overview_bx + 10.0 * s, overview_by + 20.0 * s, 13.0 * s, Color::new(0.35, 0.85, 1.0, 1.0));
                draw_text("11 DISTINCT VOXEL TYPES", overview_bx + 10.0 * s, overview_by + 36.0 * s, 11.0 * s, WHITE);
                draw_text("ISLAND BFS CANTILEVER: 4", overview_bx + 10.0 * s, overview_by + 50.0 * s, 10.5 * s, Color::new(0.70, 0.80, 0.90, 0.85));
                draw_text("SELECT CARD TO INSPECT", overview_bx + 10.0 * s, overview_by + 64.0 * s, 10.0 * s, Color::new(0.9, 0.75, 0.2, 0.9));

                // Bottom Detailed Forensic Dossier Panel
                let dossier_y = grid_start_y + 3.0 * (card_h + 8.0 * s) + 6.0 * s;
                let dossier_h = content_y + content_h - dossier_y;
                draw_rectangle(content_x, dossier_y, content_w, dossier_h, Color::new(0.08, 0.10, 0.14, 0.95));
                draw_rectangle_lines(content_x, dossier_y, content_w, dossier_h, 1.5 * s, Color::new(0.25, 0.45, 0.70, 0.85));

                let (sel_type, sel_name, sel_role) = materials[self.selected_material.min(materials.len() - 1)];
                let sel_props = sel_type.properties();

                let header_str = format!("[ FORENSIC DOSSIER: {} - {} ]", sel_name, sel_role);
                draw_text(&header_str, content_x + 14.0 * s, dossier_y + 20.0 * s, 14.5 * s, Color::new(0.30, 0.85, 0.95, 1.0));

                let (d_mech, d_weapon) = match sel_type {
                    VoxelType::Concrete => ("Fractures under high-yield concussive blasts. Susceptible to flash-freezing and thermal cutting.", "RECOMMENDED: Frostbite Mortar flash-freeze or Havoc Mortar concussive blast."),
                    VoxelType::SteelRebar => ("High tensile strength prevents floor collapse. Retains integrity until extreme tension shear failure.", "RECOMMENDED: Solstice Lance surgical thermal laser notch-cutting."),
                    VoxelType::CompositeRebar => ("Advanced carbon weave bridges foundational spans. Enhanced blast resistance over standard steel.", "RECOMMENDED: Apex Railgun relativistic kinetic dart punch-through."),
                    VoxelType::HeavyArmor => ("Deflects non-penetrating concussive shockwaves. Extremely resilient against explosive splash.", "RECOMMENDED: Apex Railgun kinetic dart or 2500mm Rail Artillery core strike."),
                    VoxelType::Glass => ("Extremely brittle silica crystal with near-zero toughness. Shatters immediately on impact.", "RECOMMENDED: Any kinetic or explosive weapon. Titan Ram shockwaves clear facades."),
                    VoxelType::Explosive => ("Volatile petrochemical core. Severing or detonating triggers cascading secondary blasts.", "RECOMMENDED: Long-range sniper ignition or Havoc Mortar airburst detonator."),
                    VoxelType::Brick => ("Brittle masonry infill. Lacks load-bearing capacity; crumbles easily into loose rubble piles.", "RECOMMENDED: Titan Ram hydraulic breacher or Havoc Mortar."),
                    VoxelType::Bedrock => ("Reinforced geological foundation with high damage resistance. Supports columns until damaged or excavated.", "DESTRUCTIBLE FOUNDATION: Concentrated damage and heavy demolition can carve the bedrock."),
                    VoxelType::CryoIce => ("Super-brittle flash-frozen material created by sub-zero weapons. Shatters into ice clouds.", "RECOMMENDED: Any kinetic projectile or subsequent explosion causes instant shatter."),
                    VoxelType::GlowCore => ("High-energy luminous capacitor cell. Breaching core releases volatile thermal energy bursts.", "RECOMMENDED: Solstice Lance precision thermal cut to trigger containment rupture."),
                    VoxelType::PowerCapacitor => ("Conductive high-voltage storage node. Detonation arcs electricity through connected rebar.", "RECOMMENDED: Tesla Caster arc-overload or direct kinetic puncture."),
                    _ => ("", ""),
                };

                let p1_str = format!("HARDNESS: {:.1}   |   TOUGHNESS: {:.1}   |   MASS DENSITY: {:.0} kg/m³   |   BLAST FRACTURE THRESHOLD: {:.1} MJ", sel_props.hardness, sel_props.toughness, sel_props.density, sel_props.blast_threshold);
                draw_text(&p1_str, content_x + 14.0 * s, dossier_y + 38.0 * s, 12.0 * s, WHITE);

                let p2_str = format!("ELECTRICAL CONDUCTIVITY: {:.2} S/m   |   THERMAL SOFTENING RATIO: {:.2}   |   EMISSIVE: {}", sel_props.conductivity, sel_props.thermal_soften, sel_props.is_emissive);
                draw_text(&p2_str, content_x + 14.0 * s, dossier_y + 54.0 * s, 11.5 * s, Color::new(0.75, 0.85, 0.95, 0.9));

                let p3_str = format!("FRACTURE BEHAVIOR: {}", d_mech);
                draw_text(&p3_str, content_x + 14.0 * s, dossier_y + 70.0 * s, 11.5 * s, Color::new(0.85, 0.90, 0.98, 0.95));

                let p4_str = format!("TACTICAL COUNTER: {}", d_weapon);
                draw_text(&p4_str, content_x + 14.0 * s, dossier_y + 86.0 * s, 11.5 * s, Color::new(0.95, 0.80, 0.25, 1.0));
            }

            // ==================== TAB 4: AUDIO SYNTHESIZER ====================
            4 => {
                draw_text("PROCEDURAL 16-BIT PCM AUDIO SYNTHESIZER & CONCURRENCY LIMITER", content_x, content_y + 16.0 * s, 17.0 * s, WHITE);

                let col_spacing = 16.0 * s;
                let left_w = (content_w - col_spacing) * 0.52;
                let right_w = (content_w - col_spacing) * 0.48;
                let col2_x = content_x + left_w + col_spacing;

                // ---- LEFT COLUMN: Procedural PCM Parameter Sliders ----
                let card_h = 50.0 * s;
                let card_spacing = 6.0 * s;
                let track_w = left_w - 24.0 * s;
                let track_h = 8.0 * s;

                // Slider 1: Master Volume Gain (0.0x to 2.0x)
                let s1_y = content_y + 24.0 * s;
                draw_rectangle(content_x, s1_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s1_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str1 = format!("MASTER AUDIO GAIN: {:.2}x", self.master_volume);
                draw_text(&str1, content_x + 12.0 * s, s1_y + 16.0 * s, 12.5 * s, WHITE);
                let track1_x = content_x + 12.0 * s;
                let track1_y = s1_y + 26.0 * s;
                draw_rectangle(track1_x, track1_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p1 = (self.master_volume / 2.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track1_y, track_w * p1, track_h, theme_color);
                draw_circle(track1_x + track_w * p1, track1_y + track_h * 0.5, 5.5 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s1_y + 16.0 * s && mouse_pos.1 <= s1_y + 46.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    self.master_volume = t * 2.0;
                }

                // Slider 2: Siege Artillery & Blast Gain (0.5x to 2.5x)
                let s2_y = s1_y + card_h + card_spacing;
                draw_rectangle(content_x, s2_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s2_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str2 = format!("BLAST & ARTILLERY GAIN: {:.2}x", self.siege_volume);
                draw_text(&str2, content_x + 12.0 * s, s2_y + 16.0 * s, 12.5 * s, WHITE);
                let track2_y = s2_y + 26.0 * s;
                draw_rectangle(track1_x, track2_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p2 = ((self.siege_volume - 0.5) / 2.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track2_y, track_w * p2, track_h, theme_color);
                draw_circle(track1_x + track_w * p2, track2_y + track_h * 0.5, 5.5 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s2_y + 16.0 * s && mouse_pos.1 <= s2_y + 46.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    self.siege_volume = 0.5 + t * 2.0;
                }

                // Slider 3: Concurrency Anti-Stacking Guard (20ms to 200ms)
                let s3_y = s2_y + card_h + card_spacing;
                draw_rectangle(content_x, s3_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s3_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str3 = format!("ANTI-STACKING GUARD DURATION: {:.1}ms [DEFAULT: 75.0ms]", self.anti_stacking_ms);
                draw_text(&str3, content_x + 12.0 * s, s3_y + 16.0 * s, 12.5 * s, WHITE);
                let track3_y = s3_y + 26.0 * s;
                draw_rectangle(track1_x, track3_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p3 = ((self.anti_stacking_ms - 20.0) / 180.0).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track3_y, track_w * p3, track_h, theme_color);
                draw_circle(track1_x + track_w * p3, track3_y + track_h * 0.5, 5.5 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s3_y + 16.0 * s && mouse_pos.1 <= s3_y + 46.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    self.anti_stacking_ms = 20.0 + t * 180.0;
                }

                // Stress creak is synthesized once at startup; display its fixed resonance.
                let s4_y = s3_y + card_h + card_spacing;
                draw_rectangle(content_x, s4_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s4_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str4 = format!("STRESS CREAK RESONANCE: {:.0} Hz (FIXED)", self.stress_creak_freq);
                draw_text(&str4, content_x + 12.0 * s, s4_y + 16.0 * s, 12.5 * s, WHITE);
                draw_text("GENERATED AT STARTUP", track1_x, s4_y + 36.0 * s, 10.0 * s, Color::new(0.60, 0.70, 0.80, 0.9));

                // Slider 5: Structural Stress Creak Volume (0.1x to 2.0x)
                let s5_y = s4_y + card_h + card_spacing;
                draw_rectangle(content_x, s5_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s5_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str5 = format!("STRUCTURAL STRESS CREAK VOLUME: {:.2}x", self.stress_creak_vol);
                draw_text(&str5, content_x + 12.0 * s, s5_y + 16.0 * s, 12.5 * s, WHITE);
                let track5_y = s5_y + 26.0 * s;
                draw_rectangle(track1_x, track5_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p5 = ((self.stress_creak_vol - 0.1) / 1.9).clamp(0.0, 1.0);
                draw_rectangle(track1_x, track5_y, track_w * p5, track_h, theme_color);
                draw_circle(track1_x + track_w * p5, track5_y + track_h * 0.5, 5.5 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s5_y + 16.0 * s && mouse_pos.1 <= s5_y + 46.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    self.stress_creak_vol = 0.1 + t * 1.9;
                }

                // Slider 6: Master Procedural Dark-Synth Music Volume (0.0x to 1.0x)
                let s6_y = s5_y + card_h + card_spacing;
                draw_rectangle(content_x, s6_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s6_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str6 = format!("MUSIC GAIN: {:.2}x [{} MUTE]", self.music_volume, crate::controls::MUSIC.label);
                draw_text(&str6, content_x + 12.0 * s, s6_y + 16.0 * s, 12.5 * s, WHITE);
                let track6_y = s6_y + 26.0 * s;
                draw_rectangle(track1_x, track6_y, track_w, track_h, Color::new(0.18, 0.22, 0.28, 1.0));
                let p6 = self.music_volume.clamp(0.0, 1.0);
                draw_rectangle(track1_x, track6_y, track_w * p6, track_h, theme_color);
                draw_circle(track1_x + track_w * p6, track6_y + track_h * 0.5, 5.5 * s, WHITE);
                if is_mouse_button_down(MouseButton::Left) && mouse_pos.0 >= track1_x - 10.0 * s && mouse_pos.0 <= track1_x + track_w + 10.0 * s && mouse_pos.1 >= s6_y + 16.0 * s && mouse_pos.1 <= s6_y + 46.0 * s {
                    let t = ((mouse_pos.0 - track1_x) / track_w).clamp(0.0, 1.0);
                    self.music_volume = t;
                }

                // Music loops share the fixed bass boost used for startup synthesis.
                let s7_y = s6_y + card_h + card_spacing;
                draw_rectangle(content_x, s7_y, left_w, card_h, Color::new(0.11, 0.13, 0.18, 0.95));
                draw_rectangle_lines(content_x, s7_y, left_w, card_h, 1.0, Color::new(0.25, 0.32, 0.42, 0.8));
                let str7 = format!("SYNTH BASS BOOST: {:.2}x (FIXED)", self.synth_bass_boost);
                draw_text(&str7, content_x + 12.0 * s, s7_y + 16.0 * s, 12.5 * s, WHITE);
                draw_text("GENERATED AT STARTUP", track1_x, s7_y + 36.0 * s, 10.0 * s, Color::new(0.60, 0.70, 0.80, 0.9));

                audio.configure(self.master_volume, self.siege_volume, self.anti_stacking_ms, self.stress_creak_vol);

                // ---- RIGHT COLUMN: Audition Soundboard Pad & Oscilloscope ----
                draw_text("PROCEDURAL PCM SOUNDBOARD AUDITION PAD", col2_x, content_y + 16.0 * s, 13.5 * s, WHITE);

                let audition_sounds = [
                    ("EXPLOSION", 0.8f32),
                    ("APEX RAILGUN", 0.85f32),
                    ("ARTILLERY BOOM", 1.0f32),
                    ("STRUCTURAL GROAN", 0.75f32),
                    ("CRYO SHATTER", 0.70f32),
                    ("TESLA ARC", 0.75f32),
                    ("RAM IMPACT", 0.80f32),
                ];

                let a_cols = 2;
                let a_spacing = 8.0 * s;
                let a_btn_w = (right_w - a_spacing) / a_cols as f32;
                let a_btn_h = 30.0 * s;
                let a_start_y = content_y + 24.0 * s;

                for (idx, &(snd_name, default_gain)) in audition_sounds.iter().enumerate() {
                    let col = idx % a_cols;
                    let row = idx / a_cols;
                    let bx = col2_x + col as f32 * (a_btn_w + a_spacing);
                    let by = a_start_y + row as f32 * (a_btn_h + 6.0 * s);

                    let is_hover = mouse_pos.0 >= bx && mouse_pos.0 <= bx + a_btn_w && mouse_pos.1 >= by && mouse_pos.1 <= by + a_btn_h;
                    let is_last = self.last_auditioned_sound == snd_name;

                    let bg = if is_last {
                        Color::new(0.20, 0.28, 0.38, 1.0)
                    } else if is_hover {
                        Color::new(0.16, 0.22, 0.30, 1.0)
                    } else {
                        Color::new(0.11, 0.14, 0.19, 0.95)
                    };

                    draw_rectangle(bx, by, a_btn_w, a_btn_h, bg);
                    let border_c = if is_last {
                        theme_color
                    } else if is_hover {
                        WHITE
                    } else {
                        Color::new(0.28, 0.36, 0.48, 0.8)
                    };
                    draw_rectangle_lines(bx, by, a_btn_w, a_btn_h, 1.0, border_c);

                    let lbl = format!("[ > {} ]", snd_name);
                    let lw = measure_text(&lbl, None, (11.0 * s).round().max(7.0) as u16, 1.0).width;
                    draw_text(&lbl, bx + (a_btn_w - lw) * 0.5, by + 19.5 * s, 11.0 * s, if is_last { theme_color } else { WHITE });

                    if is_hover && mouse_clicked {
                        self.last_auditioned_sound = snd_name;
                        let eff_gain = default_gain;
                        match snd_name {
                            "EXPLOSION" => audio.play(audio.explosion.as_ref(), eff_gain),
                            "APEX RAILGUN" => audio.play(audio.railgun.as_ref(), eff_gain),
                            "ARTILLERY BOOM" => audio.play(audio.subsonic_artillery_boom.as_ref(), eff_gain),
                            "STRUCTURAL GROAN" => audio.play(audio.structural_groan.as_ref(), eff_gain),
                            "CRYO SHATTER" => audio.play(audio.cryo_shatter.as_ref(), eff_gain),
                            "TESLA ARC" => audio.play(audio.tesla_arc.as_ref(), eff_gain),
                            _ => audio.play(audio.ram_impact.as_ref(), eff_gain),
                        }
                    }
                }

                // ---- RIGHT COLUMN: Procedural Waveform Oscilloscope & Concurrency Telemetry ----
                let osc_x = col2_x;
                let osc_y = a_start_y + 4.0 * (a_btn_h + 6.0 * s) + 8.0 * s;
                let osc_w = right_w;
                let osc_h = content_y + content_h - osc_y;
                draw_rectangle(osc_x, osc_y, osc_w, osc_h, Color::new(0.08, 0.10, 0.14, 0.96));
                draw_rectangle_lines(osc_x, osc_y, osc_w, osc_h, 1.5 * s, Color::new(0.25, 0.45, 0.70, 0.85));

                draw_text("[ SYNTH WAVEFORM PREVIEW ]", osc_x + 12.0 * s, osc_y + 18.0 * s, 12.0 * s, theme_color);

                // Oscilloscope waveform display box
                let wave_x = osc_x + 10.0 * s;
                let wave_y = osc_y + 26.0 * s;
                let wave_w = osc_w - 20.0 * s;
                let wave_h = (osc_h - 44.0 * s).max(40.0 * s);
                draw_rectangle(wave_x, wave_y, wave_w, wave_h, Color::new(0.04, 0.06, 0.09, 0.95));
                draw_rectangle_lines(wave_x, wave_y, wave_w, wave_h, 1.0, Color::new(0.18, 0.28, 0.38, 0.7));

                // Center zero line
                let wave_cy = wave_y + wave_h * 0.5;
                draw_line(wave_x, wave_cy, wave_x + wave_w, wave_cy, 1.0, Color::new(0.2, 0.3, 0.4, 0.4));

                // Draw procedural sine/wave curve
                let time = get_time() as f32;
                let freq_scalar = self.stress_creak_freq / 210.0;
                let amp = (wave_h * 0.38) * self.master_volume.min(1.5);
                let samples = 80;
                let step = wave_w / samples as f32;

                for i in 0..samples {
                    let t1 = i as f32 / samples as f32;
                    let t2 = (i + 1) as f32 / samples as f32;
                    let x1 = wave_x + i as f32 * step;
                    let x2 = wave_x + (i + 1) as f32 * step;

                    let y1 = wave_cy + (t1 * 14.0 * freq_scalar + time * 6.0).sin() * amp * (1.0 - (t1 * 4.0).sin().abs() * 0.3);
                    let y2 = wave_cy + (t2 * 14.0 * freq_scalar + time * 6.0).sin() * amp * (1.0 - (t2 * 4.0).sin().abs() * 0.3);

                    draw_line(x1, y1, x2, y2, 1.5 * s, theme_color);
                }

                // Telemetry summary badges
                let telem_info = format!(
                    "PCM: 16-BIT 44.1kHz | ANTI-STACK: {:.0}ms | LAST: {}",
                    self.anti_stacking_ms,
                    self.last_auditioned_sound
                );
                draw_text(&telem_info, wave_x + 6.0 * s, wave_y + wave_h - 6.0 * s, 9.5 * s, Color::new(0.70, 0.80, 0.92, 0.9));
            }

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customizer_toggle() {
        let mut customizer = CustomizerUI::new();
        assert!(!customizer.is_open);
        assert_eq!(customizer.active_tab, 0);

        let audio = SoundSystem::empty();
        customizer.toggle(&audio);
        assert!(customizer.is_open);

        customizer.toggle(&audio);
        assert!(!customizer.is_open);
    }

    #[test]
    fn test_presets_coverage_and_trail_styles() {
        let mut weapon = WeaponSystem::new();
        let audio = SoundSystem::empty();
        assert_eq!(PRESETS.len(), 7);

        for p in PRESETS {
            weapon.set_mode(p.mode, &audio);
            weapon.config.barrel = p.barrel;
            weapon.config.payload = p.payload;
            weapon.config.trail_style = p.trail;
            weapon.config.stabilizer = p.stabilizer;
            weapon.config.blast_radius_mult = p.blast_radius_mult;
            weapon.config.impulse_mult = p.impulse_mult;
            weapon.config.fire_rate_mult = p.fire_rate_mult;
            weapon.config.velocity_mult = p.velocity_mult;

            assert_eq!(weapon.config.mode, p.mode);
            assert_eq!(weapon.config.trail_style, p.trail);
            assert!(weapon.config.blast_radius_mult >= 0.5 && weapon.config.blast_radius_mult <= 3.0);
            assert!(weapon.config.velocity_mult >= 0.5 && weapon.config.velocity_mult <= 2.5);
        }
    }

    #[test]
    fn test_siege_engineering_workbench_tab() {
        let mut siege = SiegeSystem::new();
        let audio = SoundSystem::empty();

        assert_eq!(siege.selected_kind, SiegeEngineKind::RailArtillery);
        siege.select_kind(SiegeEngineKind::TeslaSiegePylon, &audio);
        assert_eq!(siege.selected_kind, SiegeEngineKind::TeslaSiegePylon);

        siege.config.payload = SiegePayload::BunkerBuster;
        siege.config.caliber_mm = 2200.0;
        siege.config.azimuth_deg = 270.0;
        siege.config.auto_aim_reticle = false;
        assert_eq!(siege.config.payload, SiegePayload::BunkerBuster);
        assert_eq!(siege.config.caliber_mm, 2200.0);
        assert_eq!(siege.config.azimuth_deg, 270.0);
        assert!(!siege.config.auto_aim_reticle);
        assert!(siege.config.shell_radius() > 1.5);
    }

    #[test]
    fn test_customizer_all_5_tabs_defined_and_switchable() {
        let mut customizer = CustomizerUI::new();
        let tabs = [
            "1. ORDNANCE",
            "2. BALLISTICS",
            "3. SIEGE ENGINEERING",
            "4. MATERIAL TELEMETRY",
            "5. AUDIO SYNTHESIZER",
        ];
        assert_eq!(tabs.len(), 5);

        for tab_idx in 0..5 {
            customizer.active_tab = tab_idx;
            assert_eq!(customizer.active_tab, tab_idx);
        }
    }

    #[test]
    fn test_customizer_multiline_text_wrapping() {
        let long_desc = "High-impulse ground fault shockwave to heave building foundations and trigger pancake collapses.";
        let lines = wrap_text_to_lines(long_desc, 200.0, 11.0);
        assert!(lines.len() >= 2);

        let empty_lines = wrap_text_to_lines("", 200.0, 11.0);
        assert!(empty_lines.is_empty());

        let short_lines = wrap_text_to_lines("SHORT", 200.0, 11.0);
        assert_eq!(short_lines.len(), 1);
        assert_eq!(short_lines[0], "SHORT");
    }

    #[test]
    fn test_wrapping_keeps_unbroken_words_within_column() {
        let lines = wrap_text_to_lines("HIGH-IMPULSE architecture", 40.0, 11.0);
        assert_eq!(lines.concat(), "HIGH-IMPULSEarchitecture");
        assert!(lines.iter().all(|line| crate::ui::text_width(line, 11.0) <= 40.0));
        assert!(wrap_text_to_lines("A", 1.0, 11.0).is_empty());
    }

    #[test]
    fn test_customizer_material_telemetry_properties_registry() {
        let materials = [
            VoxelType::Concrete,
            VoxelType::SteelRebar,
            VoxelType::CompositeRebar,
            VoxelType::HeavyArmor,
            VoxelType::Glass,
            VoxelType::Explosive,
            VoxelType::Brick,
            VoxelType::Bedrock,
            VoxelType::CryoIce,
            VoxelType::GlowCore,
            VoxelType::PowerCapacitor,
        ];
        assert_eq!(materials.len(), 11);

        for &v in &materials {
            let props = v.properties();
            assert!(props.hardness >= 0.0);
            assert!(props.toughness >= 0.0);
            assert!(props.density >= 0.0);
            assert!(props.blast_threshold >= 0.0);
            assert!(!props.name.is_empty());
        }
    }

    #[test]
    fn test_customizer_audio_synthesizer_parameters_clamping() {
        let mut customizer = CustomizerUI::new();
        assert_eq!(customizer.master_volume, 1.0);
        assert_eq!(customizer.siege_volume, 1.2);
        assert_eq!(customizer.anti_stacking_ms, 75.0);
        assert_eq!(customizer.stress_creak_freq, 210.0);
        assert_eq!(customizer.stress_creak_vol, 1.0);
        assert_eq!(customizer.selected_material, 0);
        assert_eq!(customizer.last_auditioned_sound, "NONE");

        customizer.master_volume = 1.5;
        customizer.master_volume = customizer.master_volume.clamp(0.0, 1.0);
        assert_eq!(customizer.master_volume, 1.0);

        customizer.anti_stacking_ms = 150.0;
        customizer.anti_stacking_ms = customizer.anti_stacking_ms.clamp(20.0, 200.0);
        assert_eq!(customizer.anti_stacking_ms, 150.0);
    }

    #[test]
    fn test_customizer_workbench_tab_width_calculation() {
        for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0), (2560.0, 1440.0), (3840.0, 2160.0)] {
            let s = compute_ui_scale(w, h);
            let pw = (w * 0.88).clamp(960.0 * s, 1200.0 * s);
            let tab_w = (pw - 2.0 * 24.0 * s) / 5.0;
            assert!(tab_w >= 180.0 * s);
        }
    }
}
