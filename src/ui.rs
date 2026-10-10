use macroquad::prelude::*;
use crate::voxel::{VoxelGrid, RayHit, VoxelType};
use crate::weapon::WeaponSystem;
use crate::physics::PhysicsSystem;
use crate::player::Player;
use crate::world_gen::LevelPreset;
use std::fmt::{self, Write};

/// Zero-allocation fixed-capacity stack-allocated string buffer implementing `std::fmt::Write`.
#[derive(Copy, Clone)]
pub struct StackStr<const N: usize> {
    buf: [u8; N],
    len: usize,
}

#[allow(dead_code)]
impl<const N: usize> StackStr<N> {
    pub const fn new() -> Self {
        Self { buf: [0; N], len: 0 }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        // Safe: bytes written via fmt::Write and push_str are guaranteed valid UTF-8
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        N
    }

    #[inline(always)]
    pub fn remaining(&self) -> usize {
        N.saturating_sub(self.len)
    }

    #[inline(always)]
    pub fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let remaining = N.saturating_sub(self.len);
        let mut copy_len = bytes.len().min(remaining);
        while copy_len > 0 && !s.is_char_boundary(copy_len) {
            copy_len -= 1;
        }
        self.buf[self.len..self.len + copy_len].copy_from_slice(&bytes[..copy_len]);
        self.len += copy_len;
    }
}

impl<const N: usize> Default for StackStr<N> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for StackStr<N> {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

impl<const N: usize> std::ops::Deref for StackStr<N> {
    type Target = str;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const N: usize> AsRef<str> for StackStr<N> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> fmt::Display for StackStr<N> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize> fmt::Debug for StackStr<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl<const N: usize> PartialEq<&str> for StackStr<N> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<const N: usize> PartialEq<str> for StackStr<N> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize, const M: usize> PartialEq<StackStr<M>> for StackStr<N> {
    fn eq(&self, other: &StackStr<M>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> Eq for StackStr<N> {}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HitKind {
    Standard,
    Critical,
    CryoShatter,
}

pub struct HitMarker {
    pub kind: HitKind,
    pub timer: f32,
    pub max_time: f32,
}

#[allow(clippy::upper_case_acronyms)]
pub struct HUD {
    pub scanner_active: bool,
    pub reticle_bloom: f32,
    pub destroyed_counter: usize,
    pub collapse_notifications: Vec<(String, f32)>, // (message, timer)
    pub hit_markers: Vec<HitMarker>,
    pub help_modal_open: bool,
    /// Distance to the static voxel under the crosshair (what the grapple can anchor to), if any.
    pub aim_static_distance: Option<f32>,
}

/// Advance of every glyph in macroquad's built-in ProggyClean font, as a fraction of the
/// font size (the font is monospaced: 896 / 2048 units per em).
pub const FONT_ADVANCE_RATIO: f32 = 0.4375;
const MAX_NOTIFICATION_ROWS: usize = 4;

/// Rendered width of `text` in the default font at `font_size`. `draw_text` truncates the size
/// to whole pixels, so rounding up here gives a safe upper bound for layout and fit checks.
#[inline]
pub fn text_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * FONT_ADVANCE_RATIO * font_size.ceil().max(1.0)
}

#[inline]
pub fn compute_ui_scale(sw: f32, sh: f32) -> f32 {
    let s_w = sw / 1280.0;
    let s_h = sh / 720.0;
    s_w.min(s_h).clamp(0.65, 3.2)
}

impl HUD {
    pub fn new() -> Self {
        Self {
            scanner_active: false,
            reticle_bloom: 0.0,
            destroyed_counter: 0,
            collapse_notifications: Vec::new(),
            hit_markers: Vec::new(),
            help_modal_open: false,
            aim_static_distance: None,
        }
    }

    pub fn add_notification(&mut self, text: &str) {
        // Cascading collapses can report every frame; retain only the visible recent messages.
        if self.collapse_notifications.len() >= MAX_NOTIFICATION_ROWS {
            self.collapse_notifications.remove(0);
        }
        self.collapse_notifications.push((text.to_string(), 3.5));
    }

    pub fn register_hit(&mut self, kind: HitKind) {
        self.hit_markers.push(HitMarker {
            kind,
            timer: 0.28,
            max_time: 0.28,
        });
        self.reticle_bloom = (self.reticle_bloom + 0.35).min(1.0);
    }

    pub fn update(&mut self, dt: f32) {
        self.reticle_bloom = (self.reticle_bloom - 8.0 * dt).max(0.0);

        self.collapse_notifications.retain_mut(|(_, timer)| {
            *timer -= dt;
            *timer > 0.0
        });

        let mut hm_i = 0;
        while hm_i < self.hit_markers.len() {
            self.hit_markers[hm_i].timer -= dt;
            if self.hit_markers[hm_i].timer <= 0.0 {
                self.hit_markers.swap_remove(hm_i);
                continue;
            }
            hm_i += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        grid: &VoxelGrid,
        weapon: &WeaponSystem,
        siege: &crate::siege::SiegeSystem,
        physics: &PhysicsSystem,
        player: &Player,
        current_level: LevelPreset,
        aim_hit: Option<&RayHit>,
    ) {
        let sw = screen_width();
        let sh = screen_height();
        let cx = sw * 0.5;
        let cy = sh * 0.5;
        let s = compute_ui_scale(sw, sh);

        // 1. Dynamic Reticle / Crosshair (Scaled proportionally)
        let bloom_offset = (8.0 + self.reticle_bloom * 16.0) * s;
        let reticle_color = if let Some(hit) = aim_hit {
            match hit.voxel_type {
                VoxelType::Explosive => Color::new(1.0, 0.25, 0.2, 1.0),
                VoxelType::Glass => Color::new(0.4, 0.9, 1.0, 1.0),
                VoxelType::SteelRebar => Color::new(0.5, 0.7, 1.0, 1.0),
                VoxelType::HeavyArmor => Color::new(0.95, 0.45, 0.25, 1.0),
                VoxelType::CompositeRebar => Color::new(0.3, 0.9, 0.85, 1.0),
                VoxelType::GlowCore => Color::new(1.0, 0.85, 0.2, 1.0),
                VoxelType::PowerCapacitor => Color::new(0.75, 0.35, 1.0, 1.0),
                _ => weapon.config.mode.color(),
            }
        } else {
            Color::new(1.0, 1.0, 1.0, 0.75)
        };

        // Center dot
        draw_circle(cx, cy, 2.2 * s, reticle_color);

        // 4 Corner Brackets
        let bracket_len = 6.5 * s;
        let b = bloom_offset;

        // Top-left
        draw_line(cx - b, cy - b, cx - b + bracket_len, cy - b, 1.6 * s, reticle_color);
        draw_line(cx - b, cy - b, cx - b, cy - b + bracket_len, 1.6 * s, reticle_color);

        // Top-right
        draw_line(cx + b, cy - b, cx + b - bracket_len, cy - b, 1.6 * s, reticle_color);
        draw_line(cx + b, cy - b, cx + b, cy - b + bracket_len, 1.6 * s, reticle_color);

        // Bottom-left
        draw_line(cx - b, cy + b, cx - b + bracket_len, cy + b, 1.6 * s, reticle_color);
        draw_line(cx - b, cy + b, cx - b, cy + b - bracket_len, 1.6 * s, reticle_color);

        // Bottom-right
        draw_line(cx + b, cy + b, cx + b - bracket_len, cy + b, 1.6 * s, reticle_color);
        draw_line(cx + b, cy + b, cx + b, cy + b - bracket_len, 1.6 * s, reticle_color);

        // Animated Hit Confirm Markers
        for marker in &self.hit_markers {
            let alpha = (marker.timer / marker.max_time).clamp(0.0, 1.0);
            let (m_col, label) = match marker.kind {
                HitKind::Standard => (Color::new(1.0, 1.0, 1.0, alpha * 0.95), None),
                HitKind::Critical => (Color::new(1.0, 0.35, 0.15, alpha), Some("CRITICAL")),
                HitKind::CryoShatter => (Color::new(0.35, 0.90, 1.0, alpha), Some("SHATTER")),
            };

            let m_dist = (11.0 + (1.0 - alpha) * 8.0) * s;
            let m_len = 6.5 * s;

            // Diagonal brackets: \ / / \
            draw_line(cx - m_dist, cy - m_dist, cx - m_dist - m_len, cy - m_dist - m_len, 2.0 * s, m_col);
            draw_line(cx + m_dist, cy - m_dist, cx + m_dist + m_len, cy - m_dist - m_len, 2.0 * s, m_col);
            draw_line(cx - m_dist, cy + m_dist, cx - m_dist - m_len, cy + m_dist + m_len, 2.0 * s, m_col);
            draw_line(cx + m_dist, cy + m_dist, cx + m_dist + m_len, cy + m_dist + m_len, 2.0 * s, m_col);

            if let Some(lbl) = label {
                draw_curved_reticle_text(
                    lbl,
                    cx,
                    cy,
                    m_dist + 26.0 * s,
                    false, // top arc
                    15.0 * s,
                    m_col,
                    get_time() as f32,
                );
            }
        }

        // Aim Target Readout with hardness, toughness, and HP
        if let Some(hit) = aim_hit {
            let props = hit.voxel_type.properties();
            let hp_pct = (hit.health_ratio * 100.0).round() as i32;
            let mut mat_buf = StackStr::<128>::new();
            match hit.voxel_type {
                VoxelType::Concrete | VoxelType::Brick => {
                    let _ = write!(mat_buf, "{} [HP: {}% | HARD: {:.1} | TOUGH: {:.1}]", props.name, hp_pct, props.hardness, props.toughness);
                }
                VoxelType::SteelRebar | VoxelType::CompositeRebar => {
                    let _ = write!(mat_buf, "{} [HP: {}% | LOAD-BEARING | HARD: {:.1}]", props.name, hp_pct, props.hardness);
                }
                VoxelType::HeavyArmor => {
                    let _ = write!(mat_buf, "{} [HP: {}% | BLAST-RESIST: {:.1} | HARD: {:.1}]", props.name, hp_pct, props.blast_threshold, props.hardness);
                }
                VoxelType::Glass => {
                    let _ = write!(mat_buf, "{} [HP: {}% | BRITTLE SILICA]", props.name, hp_pct);
                }
                VoxelType::Explosive => {
                    let _ = write!(mat_buf, "{} [CRITICAL HAZARD!]", props.name);
                }
                VoxelType::Bedrock => {
                    let _ = write!(mat_buf, "{} [HP: {}% | REINFORCED GRANITE]", props.name, hp_pct);
                }
                VoxelType::Basalt => {
                    let _ = write!(mat_buf, "{} [HP: {}% | SUBTERRANEAN BASALT]", props.name, hp_pct);
                }
                VoxelType::CryoIce => {
                    let _ = write!(mat_buf, "{} [HP: {}% | SUPER-BRITTLE]", props.name, hp_pct);
                }
                VoxelType::GlowCore => {
                    let _ = write!(mat_buf, "{} [HP: {}% | UNSTABLE]", props.name, hp_pct);
                }
                VoxelType::PowerCapacitor => {
                    let _ = write!(mat_buf, "{} [HP: {}% | COND: {:.1} | HIGH-VOLTAGE]", props.name, hp_pct, props.conductivity);
                }
                VoxelType::Air => {}
            }

            let mut range_buf = StackStr::<160>::new();
            let _ = write!(range_buf, "{:.1}m | {}", hit.distance, mat_buf.as_str());
            draw_curved_reticle_text(
                range_buf.as_str(),
                cx,
                cy,
                (b + 38.0 * s).max(46.0 * s),
                true, // bottom arc
                15.5 * s,
                reticle_color,
                get_time() as f32,
            );
        }

        // 1.5 Contextual Dynamic Reticle Action Prompt (Always guides next player action)
        let prompt_buf = reticle_prompt(
            player.is_grappling,
            self.aim_static_distance,
            aim_hit.is_some(),
            weapon.c4_charges.len(),
        );
        let prompt_text = prompt_buf.as_str();
        let p_font_size = 10.5 * s;
        let pw = text_width(prompt_text, p_font_size);
        let prompt_y = cy + (b + 52.0 * s).max(62.0 * s);
        let px = cx - pw * 0.5;
        draw_rectangle(px - 8.0 * s, prompt_y - 10.0 * s, pw + 16.0 * s, 14.5 * s, Color::new(0.04, 0.06, 0.10, 0.85));
        draw_rectangle_lines(px - 8.0 * s, prompt_y - 10.0 * s, pw + 16.0 * s, 14.5 * s, 1.0 * s, Color::new(0.20, 0.45, 0.65, 0.75));
        draw_text(prompt_text, px, prompt_y + 1.0 * s, p_font_size, Color::new(0.35, 0.90, 1.0, 0.95));

        // 2. Sleek Top Cybernetic Telemetry Visor Bar (Compact, high-tech, zero-overlap)
        let top_w = (sw * 0.76).clamp(560.0 * s, 1180.0 * s);
        let top_h = 58.0 * s;
        let top_x = (sw - top_w) * 0.5;
        let top_y = 10.0 * s;

        // Background card
        draw_rectangle(top_x, top_y, top_w, top_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(top_x, top_y, top_w, top_h, 1.5 * s, Color::new(0.20, 0.32, 0.44, 0.85));

        let remaining_voxels = grid.current_voxels;
        let total_voxels = grid.total_initial_voxels.max(1);
        let integrity_pct = (remaining_voxels as f32 / total_voxels as f32).clamp(0.0, 1.0);

        let rebar_pct = if grid.initial_rebar_count > 0 {
            (grid.current_rebar_count as f32 / grid.initial_rebar_count as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Center Integrity bar geometry
        let bar_w = (top_w * 0.28).clamp(140.0 * s, 260.0 * s);
        let bar_h = 9.0 * s;
        let bar_x = top_x + (top_w - bar_w) * 0.5;
        let bar_y = top_y + 15.0 * s;

        // Row 1 Left: Level Title & Dimensions with guaranteed >= 20px clearance to central bar
        let max_left_w = (bar_x - (top_x + 14.0 * s) - 20.0 * s).max(60.0 * s);
        let title_fit = fit_text_to_width_stack::<64>(current_level.name(), max_left_w, 14.0 * s);
        draw_text(title_fit.as_str(), top_x + 14.0 * s, top_y + 18.0 * s, 14.0 * s, WHITE);

        let mut dim_buf = StackStr::<64>::new();
        let _ = write!(dim_buf, "[{}]", current_level.dimensions());
        let dim_fit = fit_text_to_width_stack::<64>(dim_buf.as_str(), max_left_w, 10.5 * s);
        draw_text(dim_fit.as_str(), top_x + 14.0 * s, top_y + 31.0 * s, 10.5 * s, Color::new(0.65, 0.75, 0.88, 0.90));

        // Center Integrity bar rendering
        draw_rectangle(bar_x, bar_y, bar_w, bar_h, Color::new(0.14, 0.16, 0.20, 1.0));
        let integrity_color = if integrity_pct > 0.6 {
            Color::new(0.2, 0.85, 0.4, 1.0)
        } else if integrity_pct > 0.3 {
            Color::new(0.95, 0.75, 0.2, 1.0)
        } else {
            Color::new(0.95, 0.25, 0.2, 1.0)
        };
        draw_rectangle(bar_x, bar_y, bar_w * integrity_pct, bar_h, integrity_color);

        let mut int_buf = StackStr::<32>::new();
        let _ = write!(int_buf, "INTEGRITY: {:.1}%", integrity_pct * 100.0);
        let iw = measure_text(int_buf.as_str(), None, (11.5 * s).round().max(8.0) as u16, 1.0).width;
        draw_text(int_buf.as_str(), bar_x + (bar_w - iw) * 0.5, bar_y - 2.5 * s, 11.5 * s, integrity_color);

        // Row 1 Right: Rebar & FPS Telemetry with guaranteed >= 20px clearance from central bar
        let rebar_color = if rebar_pct > 0.5 {
            Color::new(0.3, 0.9, 0.85, 1.0)
        } else if rebar_pct > 0.2 {
            Color::new(1.0, 0.75, 0.2, 1.0)
        } else {
            Color::new(1.0, 0.3, 0.3, 1.0)
        };
        let mut rebar_buf = StackStr::<48>::new();
        let _ = write!(rebar_buf, "REBAR: {:.0}%  |  FPS: {}", rebar_pct * 100.0, get_fps());
        let rw = measure_text(rebar_buf.as_str(), None, (13.5 * s).round().max(8.0) as u16, 1.0).width;
        let min_right_x = bar_x + bar_w + 20.0 * s;
        let rebar_x = (top_x + top_w - rw - 14.0 * s).max(min_right_x);
        draw_text(rebar_buf.as_str(), rebar_x, top_y + 22.0 * s, 13.5 * s, rebar_color);

        // Row 2: Comprehensive Telemetry Ticker (Debris, Collapses, Culling, Rubble, Altitude, Jetpack)
        let dynamic_count = physics.chunks.iter().filter(|c| !c.settled).count();
        let settled_count = physics.chunks.iter().filter(|c| c.settled).count();
        let dash_ratio = player.dash_charge_ratio();
        let dash_status = if dash_ratio >= 0.99 { "READY" } else { "CHARGING" };

        let cull_m = physics.cull_metrics.get();
        let cull_pct = (cull_m.cull_ratio() * 100.0).round() as i32;
        let drawn_count = cull_m.particles_drawn() + cull_m.chunks_drawn + cull_m.decals_drawn;
        let total_count = cull_m.particles_total() + cull_m.chunks_total + cull_m.decals_total;

        let mut ticker_buf = StackStr::<256>::new();
        let _ = write!(
            ticker_buf,
            "PULVERIZED: {}   |   COLLAPSES: {}   |   CULL: {}% ({}/{})   |   RUBBLE: {} DYN, {} SETTLED   |   ALT: {:4.1}m   |   JET: {}",
            self.destroyed_counter,
            physics.total_collapsed_chunks,
            cull_pct,
            drawn_count,
            total_count,
            dynamic_count,
            settled_count,
            player.position.y,
            dash_status
        );
        let max_ticker_w = top_w - 28.0 * s;
        let ticker_fit = fit_text_to_width_stack::<256>(ticker_buf.as_str(), max_ticker_w, 12.0 * s);
        draw_text(ticker_fit.as_str(), top_x + 14.0 * s, top_y + 46.0 * s, 12.0 * s, Color::new(0.70, 0.78, 0.88, 1.0));

        // Scanner Mode Sub-Bar (Cleanly locked below top bar)
        let sc_h = 24.0 * s;
        let sc_y = top_y + top_h + 4.0 * s;
        if self.scanner_active {
            draw_rectangle_lines(8.0 * s, 8.0 * s, sw - 16.0 * s, sh - 16.0 * s, 2.0 * s, Color::new(0.2, 0.85, 1.0, 0.45));
            draw_rectangle(top_x, sc_y, top_w, sc_h, Color::new(0.06, 0.12, 0.18, 0.92));
            draw_rectangle_lines(top_x, sc_y, top_w, sc_h, 1.2 * s, Color::new(0.25, 0.85, 1.0, 0.85));
            let sc_title = "[ SCANNER ACTIVE: TARGET HIGHLIGHTED REBAR COLUMNS TO TRIGGER CASCADING COLLAPSE ]";
            let stw = measure_text(sc_title, None, (12.5 * s).round().max(8.0) as u16, 1.0).width;
            draw_text(sc_title, top_x + (top_w - stw) * 0.5, sc_y + 16.0 * s, 12.5 * s, Color::new(0.35, 0.92, 1.0, 1.0));
        }

        // Dedicated Visor HUD Telemetry Strip: Horizontal Compass Heading Tape & Real-Time Seismic Load Meter
        let ribbon_y = if self.scanner_active {
            sc_y + sc_h + 4.0 * s
        } else {
            top_y + top_h + 4.0 * s
        };
        let ribbon_h = 18.0 * s;

        // 1. Center: Dedicated Horizontal Compass Heading Tape Widget
        let compass_w = (top_w * 0.44).clamp(240.0 * s, 400.0 * s);
        let compass_x = cx - compass_w * 0.5;
        draw_rectangle(compass_x, ribbon_y, compass_w, ribbon_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(compass_x, ribbon_y, compass_w, ribbon_h, 1.2 * s, Color::new(0.20, 0.35, 0.50, 0.85));

        // Center Boresight Caret
        draw_triangle(
            Vec2::new(cx - 3.5 * s, ribbon_y + 1.0 * s),
            Vec2::new(cx + 3.5 * s, ribbon_y + 1.0 * s),
            Vec2::new(cx, ribbon_y + 5.5 * s),
            Color::new(0.3, 0.9, 1.0, 1.0),
        );

        let heading_deg = player.compass_heading_degrees();
        let half_fov = 60.0f32;
        let half_tape_w = compass_w * 0.46;

        let min_heading_tick = ((heading_deg - half_fov) / 10.0).floor() as i32 * 10;
        let max_heading_tick = ((heading_deg + half_fov) / 10.0).ceil() as i32 * 10;

        for tick in (min_heading_tick..=max_heading_tick).step_by(10) {
            let norm_deg = ((tick % 360) + 360) % 360;
            let offset_deg = tick as f32 - heading_deg;
            if offset_deg.abs() > half_fov {
                continue;
            }
            let tick_x = cx + (offset_deg / half_fov) * half_tape_w;
            if tick_x < compass_x + 5.0 * s || tick_x > compass_x + compass_w - 5.0 * s {
                continue;
            }

            let is_major = norm_deg % 30 == 0;
            let tick_len = if is_major { 7.0 * s } else { 3.5 * s };
            let tick_col = if is_major { Color::new(0.5, 0.75, 1.0, 0.85) } else { Color::new(0.3, 0.45, 0.6, 0.55) };
            draw_line(tick_x, ribbon_y + ribbon_h - tick_len, tick_x, ribbon_y + ribbon_h, 1.0 * s, tick_col);

            if is_major {
                let lbl = match norm_deg {
                    0 => "N",
                    30 => "030",
                    60 => "060",
                    90 => "E",
                    120 => "120",
                    150 => "150",
                    180 => "S",
                    210 => "210",
                    240 => "240",
                    270 => "W",
                    300 => "300",
                    330 => "330",
                    _ => "",
                };
                let is_cardinal = norm_deg % 90 == 0;
                let lbl_col = if is_cardinal {
                    Color::new(0.3, 0.92, 1.0, 1.0)
                } else {
                    Color::new(0.70, 0.78, 0.88, 0.85)
                };
                let lw = measure_text(lbl, None, (9.5 * s).round().max(7.0) as u16, 1.0).width;
                draw_text(lbl, tick_x - lw * 0.5, ribbon_y + 9.5 * s, 9.5 * s, lbl_col);
            }
        }

        // Siege battery waypoints on compass tape
        for engine in &siege.deployed_engines {
            let dx = engine.position.x - player.position.x;
            let dz = engine.position.z - player.position.z;
            let bearing = ((-dx).atan2(-dz).to_degrees() + 360.0) % 360.0;
            let diff = (bearing - heading_deg + 540.0) % 360.0 - 180.0;
            if diff.abs() <= half_fov {
                let wp_x = cx + (diff / half_fov) * half_tape_w;
                if wp_x >= compass_x + 6.0 * s && wp_x <= compass_x + compass_w - 6.0 * s {
                    let wp_col = engine.kind.theme_color();
                    draw_circle(wp_x, ribbon_y + ribbon_h - 4.0 * s, 2.5 * s, wp_col);
                }
            }
        }

        // 2. Right: Real-Time Seismic Ground Fault Stress Meter
        let seis_w = (top_w * 0.24).clamp(140.0 * s, 220.0 * s);
        let seis_x = top_x + top_w - seis_w;
        let seis_y = ribbon_y;
        let seis_h = ribbon_h;

        let seismic_stress = compute_seismic_stress(physics);
        let stress_color = if seismic_stress > 0.70 {
            Color::new(1.0, 0.25, 0.2, 1.0)
        } else if seismic_stress > 0.35 {
            Color::new(0.95, 0.75, 0.2, 1.0)
        } else {
            Color::new(0.2, 0.85, 0.5, 1.0)
        };

        draw_rectangle(seis_x, seis_y, seis_w, seis_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(seis_x, seis_y, seis_w, seis_h, 1.2 * s, stress_color);

        let mut seis_buf = StackStr::<32>::new();
        if seismic_stress > 0.70 {
            let _ = write!(seis_buf, "FAULT: HIGH {:.0}%", seismic_stress * 100.0);
        } else {
            let _ = write!(seis_buf, "SEISMIC: {:.0}%", seismic_stress * 100.0);
        }
        draw_text(seis_buf.as_str(), seis_x + 7.0 * s, seis_y + 12.5 * s, 10.5 * s, stress_color);

        // Segmented stress bar (8 pips)
        let pip_area_x = seis_x + 88.0 * s;
        let pip_area_w = (seis_w - 94.0 * s).max(25.0 * s);
        let pips = 8;
        let pip_spacing = 2.0 * s;
        let pip_w = (pip_area_w - (pips - 1) as f32 * pip_spacing) / pips as f32;
        let active_pips = (seismic_stress * pips as f32).round() as usize;

        for p in 0..pips {
            let px = pip_area_x + p as f32 * (pip_w + pip_spacing);
            let py = seis_y + 4.0 * s;
            let ph = seis_h - 8.0 * s;
            if p < active_pips {
                draw_rectangle(px, py, pip_w, ph, stress_color);
            } else {
                draw_rectangle(px, py, pip_w, ph, Color::new(0.15, 0.18, 0.22, 0.8));
            }
        }

        // 3. Left: Boresight Heading Readout Widget for visual balance
        let hdg_w = seis_w;
        let hdg_x = top_x;
        let hdg_y = ribbon_y;
        let hdg_h = ribbon_h;
        draw_rectangle(hdg_x, hdg_y, hdg_w, hdg_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(hdg_x, hdg_y, hdg_w, hdg_h, 1.2 * s, Color::new(0.20, 0.35, 0.50, 0.85));

        let compass_cardinal = match heading_deg {
            d if !(22.5..337.5).contains(&d) => "N",
            d if (22.5..67.5).contains(&d) => "NE",
            d if (67.5..112.5).contains(&d) => "E",
            d if (112.5..157.5).contains(&d) => "SE",
            d if (157.5..202.5).contains(&d) => "S",
            d if (202.5..247.5).contains(&d) => "SW",
            d if (247.5..292.5).contains(&d) => "W",
            _ => "NW",
        };
        let mut hdg_buf = StackStr::<48>::new();
        let _ = write!(hdg_buf, "BORESIGHT: {:03.0}° [{}]", heading_deg, compass_cardinal);
        draw_text(hdg_buf.as_str(), hdg_x + 8.0 * s, hdg_y + 12.5 * s, 10.5 * s, Color::new(0.35, 0.90, 1.0, 1.0));

        // 3. Floating Collapse Notifications (Bending along dynamic holographic visor arc)
        let overhead_bottom = if self.scanner_active {
            (sc_y + sc_h).max(ribbon_y + ribbon_h) + 10.0 * s
        } else {
            ribbon_y + ribbon_h + 10.0 * s
        };
        let avail_top_space = cy - overhead_bottom;
        let use_bottom_arc = avail_top_space < 150.0 * s;

        for (idx, (msg, timer)) in self.collapse_notifications.iter().enumerate() {
            let alpha = (*timer / 3.5).min(1.0);
            let notif_color = Color::new(1.0, 0.7, 0.2, alpha);
            let (visor_radius, is_bottom) = if use_bottom_arc {
                let r = (90.0 + idx as f32 * 26.0) * s;
                (r, true)
            } else {
                let max_r = avail_top_space - 12.0 * s;
                let r = ((130.0 + idx as f32 * 26.0) * s).min(max_r);
                (r, false)
            };

            draw_curved_reticle_text(
                msg,
                cx,
                cy,
                visor_radius,
                is_bottom,
                16.0 * s,
                notif_color,
                get_time() as f32,
            );
        }

        // 4. Tactical Command Dock (Bottom-Left: Unified Siege Platform & Controls, streamlined & non-occluding)
        let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
        let dock_x = 16.0 * s;
        let ctrl_h = 44.0 * s;
        let siege_h = 74.0 * s;
        let total_dock_h = siege_h + ctrl_h + 6.0 * s;
        let dock_y = sh - total_dock_h - 16.0 * s;

        // 4.5 C4 Demolition Grid Widget (Docked directly above Tactical Command Dock)
        let c4_w = dock_w;
        let c4_h = 36.0 * s;
        let c4_x = dock_x;
        let c4_y = dock_y - c4_h - 6.0 * s;

        let charge_count = weapon.c4_charges.len();
        let has_charges = charge_count > 0;
        let c4_theme = if charge_count == crate::weapon::MAX_C4_CHARGES {
            Color::new(1.0, 0.45, 0.15, 1.0) // Amber alert when fully primed
        } else if has_charges {
            Color::new(0.20, 0.88, 1.0, 1.0) // High-tech cyan
        } else {
            Color::new(0.35, 0.45, 0.55, 0.7) // Standby slate
        };

        draw_rectangle(c4_x, c4_y, c4_w, c4_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(c4_x, c4_y, c4_w, c4_h, 1.2 * s, c4_theme);

        // Line 1: Header with charge telemetry and controls
        let c4_hdr_buf = c4_header(charge_count);
        let header_fit = fit_text_to_width_stack::<80>(c4_hdr_buf.as_str(), c4_w - 20.0 * s, 11.5 * s);
        draw_text(header_fit.as_str(), c4_x + 10.0 * s, c4_y + 14.0 * s, 11.5 * s, c4_theme);

        // Line 2: 24 Segmented LED Pips (Spanning across full card width)
        let pip_area_x = c4_x + 10.0 * s;
        let pip_area_w = c4_w - 20.0 * s;
        let total_pips = crate::weapon::MAX_C4_CHARGES;
        let pip_spacing = 2.0 * s;
        let pip_w = (pip_area_w - (total_pips - 1) as f32 * pip_spacing) / total_pips as f32;
        let pip_h = 10.0 * s;
        let pip_y = c4_y + 20.0 * s;

        let now = get_time() as f32;
        for p in 0..total_pips {
            let px = pip_area_x + p as f32 * (pip_w + pip_spacing);
            if p < charge_count {
                // Armed LED pip with subtle individual phase pulsation
                let pulse = ((now * 7.0 + p as f32 * 0.4).sin() * 0.25 + 0.75).clamp(0.5, 1.0);
                let pip_color = Color::new(c4_theme.r * pulse, c4_theme.g * pulse, c4_theme.b * pulse, 1.0);
                draw_rectangle(px, pip_y, pip_w, pip_h, pip_color);
            } else {
                // Inactive unprimed slot
                draw_rectangle(px, pip_y, pip_w, pip_h, Color::new(0.12, 0.15, 0.19, 0.85));
                draw_rectangle_lines(px, pip_y, pip_w, pip_h, 0.8 * s, Color::new(0.20, 0.25, 0.32, 0.6));
            }
        }

        // A. Siege Engine Platform Card (Top of Dock)
        let s_theme = siege.selected_kind.theme_color();
        draw_rectangle(dock_x, dock_y, dock_w, siege_h, Color::new(0.08, 0.10, 0.14, 0.94));
        draw_rectangle_lines(dock_x, dock_y, dock_w, siege_h, 1.5 * s, s_theme);

        // Line 1: Siege Platform Title
        let mut s_title_buf = StackStr::<64>::new();
        let _ = write!(s_title_buf, "[ SIEGE: {} ]", siege.selected_kind.name());
        let s_title_fit = fit_text_to_width_stack::<64>(s_title_buf.as_str(), dock_w - 24.0 * s, 14.0 * s);
        draw_text(s_title_fit.as_str(), dock_x + 12.0 * s, dock_y + 16.0 * s, 14.0 * s, s_theme);

        // Line 2: Caliber & Warhead Badge (Stacked vertically below title - guaranteed zero overlap!)
        let mut s_badge_buf = StackStr::<80>::new();
        let _ = write!(s_badge_buf, "ORDNANCE: {:.0}mm | {}", siege.config.caliber_mm, siege.config.payload.name());
        let s_badge_fit = fit_text_to_width_stack::<80>(s_badge_buf.as_str(), dock_w - 24.0 * s, 11.5 * s);
        draw_text(s_badge_fit.as_str(), dock_x + 12.0 * s, dock_y + 30.0 * s, 11.5 * s, Color::new(0.92, 0.78, 0.25, 0.95));

        // Line 3: Deployed Match & Status Bar
        let deployed_match = siege.deployed_engines.iter().find(|e| e.kind == siege.selected_kind);
        if let Some(engine) = deployed_match {
            let mut status_buf = StackStr::<64>::new();
            let status_c = if engine.is_ready() {
                let _ = write!(status_buf, "READY [BATTERY AT X:{:.0} Z:{:.0}]", engine.position.x, engine.position.z);
                Color::new(0.2, 0.9, 0.4, 1.0)
            } else {
                let _ = write!(status_buf, "CHARGING: {:.0}% ({:.1}s)", engine.ready_ratio() * 100.0, engine.cooldown_timer);
                Color::new(1.0, 0.7, 0.2, 1.0)
            };
            let status_fit = fit_text_to_width_stack::<64>(status_buf.as_str(), dock_w - 24.0 * s, 11.0 * s);
            draw_text(status_fit.as_str(), dock_x + 12.0 * s, dock_y + 44.0 * s, 11.0 * s, status_c);
            if !engine.is_ready() {
                draw_rectangle(dock_x + 12.0 * s, dock_y + 47.0 * s, (dock_w - 24.0 * s) * engine.ready_ratio(), 3.0 * s, s_theme);
            }
        } else {
            let mut standby_str = StackStr::<64>::new();
            let _ = write!(standby_str, "STANDBY: AIM AT GROUND & PRESS {} TO DEPLOY", crate::controls::SIEGE_DEPLOY.token);
            let standby_fit = fit_text_to_width_stack::<64>(standby_str.as_str(), dock_w - 24.0 * s, 11.0 * s);
            draw_text(standby_fit.as_str(), dock_x + 12.0 * s, dock_y + 44.0 * s, 11.0 * s, Color::new(0.65, 0.75, 0.88, 0.9));
        }

        // Line 4 & 5: Hotkey hints formatted into multi-row layout (strictly within card bounds!)
        for (i, row) in siege_hint_rows().iter().enumerate() {
            let row_fit = fit_text_to_width_stack::<96>(row.as_str(), dock_w - 24.0 * s, 11.0 * s);
            draw_text(row_fit.as_str(), dock_x + 12.0 * s, dock_y + (58.0 + i as f32 * 11.0) * s, 11.0 * s, Color::new(0.35, 0.85, 1.0, 0.95));
        }

        // B. Controls Helper Strip (Directly attached below siege card)
        let ctrl_y = dock_y + siege_h + 6.0 * s;
        draw_rectangle(dock_x, ctrl_y, dock_w, ctrl_h, Color::new(0.06, 0.08, 0.11, 0.88));
        draw_rectangle_lines(dock_x, ctrl_y, dock_w, ctrl_h, 1.2 * s, Color::new(0.2, 0.3, 0.4, 0.7));

        // Multi-row clean grid staying strictly inside dock_w:
        let strip_cols = [
            Color::new(0.78, 0.84, 0.92, 1.0),
            Color::new(0.35, 0.88, 1.0, 1.0),
            Color::new(0.68, 0.75, 0.85, 0.9),
        ];
        for (i, row) in control_strip_rows().iter().enumerate() {
            let row_fit = fit_text_to_width_stack::<96>(row.as_str(), dock_w - 24.0 * s, 11.0 * s);
            draw_text(row_fit.as_str(), dock_x + 12.0 * s, ctrl_y + (14.0 + i as f32 * 12.5) * s, 11.0 * s, strip_cols[i]);
        }

        // 5. Weapon Ordnance Card (Bottom-Right: Clean, styled, non-occluding)
        let card_w = (sw * 0.28).clamp(280.0 * s, 400.0 * s);
        let card_h = 112.0 * s;
        let card_x = sw - card_w - 16.0 * s;
        let card_y = sh - card_h - 16.0 * s;

        // C. Center Tactical Hotkey Dock (Comprehensive on-screen hotkey guidance)
        let center_x = dock_x + dock_w + 10.0 * s;
        let center_w = card_x - center_x - 10.0 * s;
        if center_w >= 180.0 * s {
            let center_h = CENTER_DOCK_H * s;
            let center_y = sh - center_h - 16.0 * s;
            draw_rectangle(center_x, center_y, center_w, center_h, Color::new(0.06, 0.08, 0.11, 0.90));
            draw_rectangle_lines(center_x, center_y, center_w, center_h, 1.2 * s, Color::new(0.20, 0.35, 0.50, 0.75));

            let rows = center_dock_rows();
            for (i, row) in rows.iter().enumerate() {
                let row_fit = fit_text_to_width_stack::<96>(row.as_str(), center_w - 16.0 * s, 10.5 * s);
                let row_col = if i % 2 == 0 { Color::new(0.80, 0.88, 0.95, 1.0) } else { Color::new(0.38, 0.86, 1.0, 1.0) };
                draw_text(row_fit.as_str(), center_x + 8.0 * s, center_y + (14.0 + i as f32 * 13.0) * s, 10.5 * s, row_col);
            }

            // Interactive Button: [F1] CONTROLS MANUAL
            let mut f1_txt = StackStr::<48>::new();
            let _ = write!(f1_txt, "{} FULL CONTROLS MANUAL", crate::controls::MANUAL.token);
            let f1_w = (center_w - 16.0 * s).min(320.0 * s);
            let f1_x = center_x + (center_w - f1_w) * 0.5;
            let f1_y = center_y + (14.0 + rows.len() as f32 * 13.0 - 6.0) * s;
            let f1_h = 13.0 * s;
            let (mx, my) = mouse_position();
            // FPS mouse motion still updates hidden cursor coordinates; a fire click
            // must not activate a UI button under that invisible pointer.
            let f1_hovered = !player.is_mouse_locked && mx >= f1_x && mx <= f1_x + f1_w && my >= f1_y && my <= f1_y + f1_h;
            if f1_hovered && is_mouse_button_pressed(MouseButton::Left) {
                self.help_modal_open = true;
            }
            let f1_col = if f1_hovered { Color::new(1.0, 0.90, 0.30, 1.0) } else { Color::new(0.95, 0.75, 0.25, 0.95) };
            draw_rectangle_lines(f1_x, f1_y, f1_w, f1_h, 1.0 * s, f1_col);
            let f1_fit = fit_text_to_width_stack::<48>(f1_txt.as_str(), f1_w - 8.0 * s, 10.5 * s);
            let f1_tx = f1_x + (f1_w - text_width(f1_fit.as_str(), 10.5 * s)).max(8.0 * s) * 0.5;
            draw_text(f1_fit.as_str(), f1_tx, f1_y + 10.0 * s, 10.5 * s, f1_col);
        }

        draw_rectangle(card_x, card_y, card_w, card_h, Color::new(0.08, 0.10, 0.14, 0.92));
        let w_color = weapon.config.mode.color();
        draw_rectangle_lines(card_x, card_y, card_w, card_h, 1.8 * s, w_color);

        // Top accent line
        draw_rectangle(card_x, card_y, card_w, 3.0 * s, w_color);

        // Weapon Name
        let full_name = weapon.config.mode.name();
        let name_src = if text_width(full_name, 21.0 * s) <= card_w - 28.0 * s {
            full_name
        } else {
            crate::controls::short_weapon_name(weapon.config.mode)
        };
        let name_fit = fit_text_to_width_stack::<64>(name_src, card_w - 28.0 * s, 21.0 * s);
        draw_text(name_fit.as_str(), card_x + 14.0 * s, card_y + 24.0 * s, 21.0 * s, w_color);

        // Badges: Barrel & Payload
        let mut bp_buf = StackStr::<64>::new();
        let _ = write!(bp_buf, "{}  |  {}", weapon.config.barrel.name(), weapon.config.payload.name());
        let bp_fit = fit_text_to_width_stack::<64>(bp_buf.as_str(), card_w - 28.0 * s, 14.0 * s);
        draw_text(bp_fit.as_str(), card_x + 14.0 * s, card_y + 48.0 * s, 14.0 * s, WHITE);

        // Trigger & Alt-Fire Action summary
        let mut trig_buf = StackStr::<64>::new();
        if !weapon.sticky_charges.is_empty() {
            let _ = write!(trig_buf, "{} DETONATE {} STICKY CHARGES", crate::controls::ALT_FIRE.token, weapon.sticky_charges.len());
        } else {
            let _ = write!(trig_buf, "{} ALT-FIRE: {}", crate::controls::ALT_FIRE.token, weapon.config.mode.alt_fire_name());
        }
        let trig_c = if !weapon.sticky_charges.is_empty() { Color::new(1.0, 0.8, 0.2, 1.0) } else { Color::new(0.35, 0.90, 1.0, 1.0) };
        let trig_fit = fit_text_to_width_stack::<64>(trig_buf.as_str(), card_w - 28.0 * s, 12.0 * s);
        draw_text(trig_fit.as_str(), card_x + 14.0 * s, card_y + 70.0 * s, 12.0 * s, trig_c);

        let mut tune_buf = StackStr::<64>::new();
        let _ = write!(tune_buf, "RAD: {:.1}x  |  IMP: {:.1}x  |  VEL: {:.1}x", weapon.config.blast_radius_mult, weapon.config.impulse_mult, weapon.config.velocity_mult);
        draw_text(tune_buf.as_str(), card_x + 14.0 * s, card_y + 90.0 * s, 12.5 * s, Color::new(0.68, 0.75, 0.84, 1.0));

        // Cooldown bar along bottom
        if weapon.fire_cooldown > 0.0 {
            let cd_pct = (weapon.fire_cooldown / 0.5).min(1.0);
            draw_rectangle(card_x + 14.0 * s, card_y + 98.0 * s, (card_w - 28.0 * s) * (1.0 - cd_pct), 4.5 * s, w_color);
        }

        // 6. Demolition Challenge Telemetry & Scoring Card (Docked right above weapon card)
        let total_voxels = grid.total_initial_voxels.max(1);
        let remaining_voxels = grid.current_voxels;
        let mass_down_pct = ((total_voxels.saturating_sub(remaining_voxels)) as f32 / total_voxels as f32 * 100.0).clamp(0.0, 100.0);
        let peak_m = grid.max_solid_y as f32 * crate::voxel::VOXEL_SIZE;

        let (rank_str, rank_color) = if mass_down_pct >= 90.0 {
            ("[RANK: S]", Color::new(1.0, 0.84, 0.0, 1.0)) // Gold
        } else if mass_down_pct >= 75.0 {
            ("[RANK: A]", Color::new(0.2, 0.9, 1.0, 1.0)) // Cyan
        } else if mass_down_pct >= 50.0 {
            ("[RANK: B]", Color::new(0.2, 0.85, 0.4, 1.0)) // Emerald
        } else if mass_down_pct >= 30.0 {
            ("[RANK: C]", Color::new(1.0, 0.6, 0.1, 1.0)) // Orange
        } else {
            ("[RANK: D]", Color::new(0.95, 0.25, 0.2, 1.0)) // Red
        };

        let score_h = 52.0 * s;
        let score_y = card_y - score_h - 8.0 * s;
        let score_x = card_x;
        let score_w = card_w;

        draw_rectangle(score_x, score_y, score_w, score_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(score_x, score_y, score_w, score_h, 1.5 * s, rank_color);
        draw_rectangle(score_x, score_y, score_w, 2.5 * s, rank_color);

        let mut telem_buf = StackStr::<48>::new();
        let _ = write!(telem_buf, "MASS DOWN: {:4.1}%  |  PEAK: {:4.1}m", mass_down_pct, peak_m);
        draw_text(telem_buf.as_str(), score_x + 12.0 * s, score_y + 20.0 * s, 12.5 * s, WHITE);

        let rw = measure_text(rank_str, None, (13.5 * s).round().max(8.0) as u16, 1.0).width;
        draw_text(rank_str, score_x + score_w - rw - 12.0 * s, score_y + 20.0 * s, 13.5 * s, rank_color);

        let pbar_x = score_x + 12.0 * s;
        let pbar_y = score_y + 30.0 * s;
        let pbar_w = score_w - 24.0 * s;
        let pbar_h = 8.0 * s;
        draw_rectangle(pbar_x, pbar_y, pbar_w, pbar_h, Color::new(0.12, 0.14, 0.18, 1.0));
        draw_rectangle(pbar_x, pbar_y, pbar_w * (mass_down_pct / 100.0), pbar_h, rank_color);
        draw_rectangle_lines(pbar_x, pbar_y, pbar_w, pbar_h, 1.0 * s, Color::new(0.3, 0.35, 0.45, 0.8));

        // 7. Screen-Space Shockwave Distortion & Chromatic Rings
        if player.trauma > 0.04 || !physics.shockwaves.is_empty() {
            let shock_intensity = (player.trauma * 1.5).max(if !physics.shockwaves.is_empty() { 0.3 } else { 0.0 }).min(1.0);
            let base_radius = sw.min(sh) * 0.32;
            let time = macroquad::time::get_time() as f32;
            for i in 0..3 {
                let wave_phase = (time * 5.0 + i as f32 * 0.85).fract();
                let r = base_radius * (0.35 + wave_phase * 0.75);
                let alpha = (1.0 - wave_phase) * shock_intensity * 0.45;
                let thick = (2.0 + shock_intensity * 2.5) * s;
                draw_circle_lines(cx - 2.5 * s * shock_intensity, cy, r, thick, Color::new(0.0, 0.9, 1.0, alpha));
                draw_circle_lines(cx + 2.5 * s * shock_intensity, cy, r, thick, Color::new(1.0, 0.4, 0.1, alpha));
                draw_circle_lines(cx, cy, r * 1.01, thick * 0.7, Color::new(1.0, 1.0, 1.0, alpha * 0.7));
            }
        }

        // 8. The full-screen controls manual is drawn by the caller after every other overlay
        //    (see `draw_controls_manual`) so nothing can cover it.
    }

    /// Renders the complete, interactive Full-Screen Holographic Keybindings Manual Modal.
    pub fn draw_controls_manual(&mut self, sw: f32, sh: f32, s: f32) {
        let modal_w = (sw * 0.94).clamp(700.0 * s, 1240.0 * s);
        let modal_h = (sh * 0.92).clamp(520.0 * s, 760.0 * s);
        let modal_x = (sw - modal_w) * 0.5;
        let modal_y = (sh - modal_h) * 0.5;

        // Dark dimming underlay covering whole screen
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.03, 0.06, 0.85));

        // Modal container
        draw_rectangle(modal_x, modal_y, modal_w, modal_h, Color::new(0.06, 0.08, 0.12, 0.98));
        draw_rectangle_lines(modal_x, modal_y, modal_w, modal_h, 2.0 * s, Color::new(0.25, 0.85, 1.0, 0.95));
        draw_rectangle(modal_x, modal_y, modal_w, 4.0 * s, Color::new(0.25, 0.85, 1.0, 1.0));

        // Header Title
        draw_text(
            "VOXEL DEMOLITION // TACTICAL KEYBINDINGS MANUAL & FIELD GUIDE",
            modal_x + 18.0 * s,
            modal_y + 24.0 * s,
            16.0 * s,
            Color::new(0.3, 0.9, 1.0, 1.0),
        );
        draw_text(
            "COMPLETE OPERATIONAL REFERENCE | ALL CONTROLS & BINDINGS | VERIFIED INPUT TABLE",
            modal_x + 18.0 * s,
            modal_y + 39.0 * s,
            10.5 * s,
            Color::new(0.65, 0.75, 0.85, 0.9),
        );

        // Close Button in header
        let (mx, my) = mouse_position();
        let btn_w = 120.0 * s;
        let btn_h = 26.0 * s;
        let btn_x = modal_x + modal_w - btn_w - 18.0 * s;
        let btn_y = modal_y + 14.0 * s;
        let hovered = mx >= btn_x && mx <= btn_x + btn_w && my >= btn_y && my <= btn_y + btn_h;
        let btn_bg = if hovered { Color::new(0.85, 0.25, 0.20, 0.95) } else { Color::new(0.14, 0.18, 0.25, 0.90) };
        draw_rectangle(btn_x, btn_y, btn_w, btn_h, btn_bg);
        draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 1.2 * s, if hovered { WHITE } else { Color::new(0.3, 0.6, 0.8, 0.8) });
        let mut btn_txt = StackStr::<32>::new();
        let _ = write!(btn_txt, "{} CLOSE", crate::controls::ESCAPE.token);
        let btn_fit = fit_text_to_width_stack::<32>(btn_txt.as_str(), btn_w - 8.0 * s, 11.5 * s);
        let btw = text_width(btn_fit.as_str(), 11.5 * s);
        let btx = btn_x + (btn_w - btw) * 0.5;
        draw_text(btn_fit.as_str(), btx, btn_y + 17.5 * s, 11.5 * s, WHITE);
        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            self.help_modal_open = false;
        }

        // 8 Categorized Cards Layout (2 rows x 4 columns)
        let header_h = 50.0 * s;
        let footer_h = 30.0 * s;
        let content_x = modal_x + 14.0 * s;
        let content_y = modal_y + header_h;
        let content_w = modal_w - 28.0 * s;
        let content_h = modal_h - header_h - footer_h;

        let cols = 4;
        let gap_x = 8.0 * s;
        let gap_y = 8.0 * s;
        let col_w = (content_w - (cols - 1) as f32 * gap_x) / cols as f32;
        let row_h = (content_h - gap_y) * 0.5;

        let panels = manual_panels();

        for (idx, panel) in panels.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let px = content_x + col as f32 * (col_w + gap_x);
            let py = content_y + row as f32 * (row_h + gap_y);

            draw_rectangle(px, py, col_w, row_h, Color::new(0.08, 0.10, 0.15, 0.92));
            draw_rectangle_lines(px, py, col_w, row_h, 1.2 * s, panel.color);
            draw_rectangle(px, py, col_w, 2.5 * s, panel.color);

            // Card title
            let title_fit = fit_text_to_width_stack::<64>(panel.title, col_w - 14.0 * s, 12.0 * s);
            draw_text(title_fit.as_str(), px + 8.0 * s, py + 16.0 * s, 12.0 * s, panel.color);

            // Card lines
            let lines = panel.lines();
            let line_step = ((row_h - 26.0 * s) / lines.len().max(1) as f32).clamp(10.5 * s, 16.0 * s);
            for (l_idx, line) in lines.iter().enumerate() {
                let ly = py + 28.0 * s + l_idx as f32 * line_step;
                if ly + 2.0 * s > py + row_h {
                    break;
                }
                let is_bullet = line.starts_with("- ") || line.starts_with("  - ");
                let text_col = if is_bullet {
                    Color::new(0.70, 0.80, 0.90, 0.90)
                } else if line.contains(':') {
                    Color::new(0.92, 0.94, 0.97, 1.0)
                } else {
                    WHITE
                };
                let fit = fit_text_to_width_stack::<80>(line.as_str(), col_w - 14.0 * s, 10.5 * s);
                draw_text(fit.as_str(), px + 8.0 * s, ly, 10.5 * s, text_col);
            }
        }

        // Footer Guidance Banner
        let footer_y = modal_y + modal_h - footer_h + 4.0 * s;
        let mut footer_buf = StackStr::<128>::new();
        let _ = write!(
            footer_buf,
            "DEMOLITION FIELD PROTOCOL: OTHER HOTKEYS PAUSED. PRESS {} OR {} TO RESUME.",
            crate::controls::MANUAL.token,
            crate::controls::ESCAPE.token,
        );
        let footer_txt = footer_buf.as_str();
        let fw = text_width(footer_txt, 10.5 * s);
        let fx = modal_x + (modal_w - fw) * 0.5;
        draw_text(footer_txt, fx, footer_y + 12.0 * s, 10.5 * s, Color::new(0.40, 0.85, 1.0, 0.95));
    }
}

#[inline]
pub fn char_glyph_width(ch: char, font_size: f32) -> f32 {
    match ch {
        'i' | 'l' | '!' | ':' | '|' | '.' | ',' | '\'' | ';' | '[' | ']' => font_size * 0.28,
        'w' | 'm' | 'W' | 'M' => font_size * 0.72,
        ' ' => font_size * 0.35,
        _ => font_size * 0.52,
    }
}

/// How many leading characters of `text` to keep (before appending `...`) so it fits in
/// `max_width`, or `None` if the whole text already fits.
#[inline]
fn fit_char_count(text: &str, max_width: f32, font_size: f32) -> Option<(usize, usize)> {
    if text_width(text, font_size) <= max_width {
        return None;
    }
    let glyph = FONT_ADVANCE_RATIO * font_size.ceil().max(1.0);
    let max_chars = (max_width / glyph).floor().max(0.0) as usize;
    let dots = max_chars.min(3);
    Some((max_chars.saturating_sub(dots), dots))
}

/// Truncate text with ellipsis if it exceeds max_width into a zero-allocation StackStr.
#[inline]
pub fn fit_text_to_width_stack<const N: usize>(text: &str, max_width: f32, font_size: f32) -> StackStr<N> {
    let mut result = StackStr::<N>::new();
    match fit_char_count(text, max_width, font_size) {
        None => result.push_str(text),
        Some((keep, dots)) => {
            for ch in text.chars().take(keep) {
                let mut char_buf = [0u8; 4];
                result.push_str(ch.encode_utf8(&mut char_buf));
            }
            result.push_str(&"..."[..dots]);
        }
    }
    result
}

/// Truncate text with ellipsis if it exceeds max_width, ensuring zero layout overflow.
#[inline]
pub fn fit_text_to_width(text: &str, max_width: f32, font_size: f32) -> String {
    match fit_char_count(text, max_width, font_size) {
        None => text.to_string(),
        Some((keep, dots)) => {
            let mut fit_chars: String = text.chars().take(keep).collect();
            fit_chars.push_str(&"..."[..dots]);
            fit_chars
        }
    }
}

// ---------------------------------------------------------------------------------------------
// On-screen key guidance. Every string below is built from `crate::controls`, so the HUD can
// never advertise a key that does something else.
// ---------------------------------------------------------------------------------------------

/// Height of the bottom-center hotkey dock, in unscaled HUD pixels.
pub const CENTER_DOCK_H: f32 = 92.0;

/// Context prompt under the reticle: what Q / E / F / B do right now.
pub fn reticle_prompt(
    is_grappling: bool,
    static_aim_distance: Option<f32>,
    has_aim_surface: bool,
    c4_count: usize,
) -> StackStr<128> {
    use crate::controls as c;
    let mut buf = StackStr::<128>::new();
    if is_grappling {
        let _ = write!(buf, "{} RELEASE WINCH | {} SLINGSHOT", c::GRAPPLE.token, c::JUMP.token);
    } else if static_aim_distance.is_some_and(|d| d <= c::GRAPPLE_RANGE) {
        let _ = write!(buf, "{} GRAPPLE", c::GRAPPLE.token);
    } else {
        let _ = write!(buf, "{} JET-DASH", c::GRAPPLE.token);
    }
    if has_aim_surface {
        if c4_count < crate::weapon::MAX_C4_CHARGES {
            let _ = write!(buf, " | {} PLANT C4", c::C4_PLANT.token);
        } else {
            buf.push_str(" | C4 FULL");
        }
    }
    if c4_count > 0 {
        let _ = write!(buf, " | {} DETONATE {}", c::C4_DETONATE.token, c4_count);
    }
    let _ = write!(buf, " | {} DEPLOY SIEGE", c::SIEGE_DEPLOY.token);
    buf
}

/// Header line of the C4 grid widget.
pub fn c4_header(charge_count: usize) -> StackStr<80> {
    use crate::controls as c;
    let max = crate::weapon::MAX_C4_CHARGES;
    let mut buf = StackStr::<80>::new();
    if charge_count > 0 {
        let _ = write!(buf, "C4 DEMOLITION GRID: [{}/{} ARMED] | {} DETONATE", charge_count, max, c::C4_DETONATE.token);
    } else {
        let _ = write!(buf, "C4 DEMOLITION GRID: [0/{}] | {} PLANT ON AIM", max, c::C4_PLANT.token);
    }
    buf
}

/// The two key-hint rows at the bottom of the siege card.
pub fn siege_hint_rows() -> [StackStr<96>; 2] {
    use crate::controls as c;
    let mut rows = [StackStr::<96>::new(); 2];
    let _ = write!(rows[0], "{} DEPLOY / MOVE | {} FIRE SALVO", c::SIEGE_DEPLOY.token, c::SIEGE_FIRE.token);
    let _ = write!(rows[1], "{} CYCLE TYPE | {} RECALL ALL", c::SIEGE_CYCLE.token, c::SIEGE_RECALL.token);
    rows
}

/// The three rows of the bottom-left controls strip (movement, firing, gadgets).
pub fn control_strip_rows() -> [StackStr<96>; 3] {
    use crate::controls as c;
    let mut rows = [StackStr::<96>::new(); 3];
    let _ = write!(rows[0], "{} MOVE | {} JUMP | {} SPRINT", c::MOVE.token, c::JUMP.token, c::SPRINT.token);
    let _ = write!(rows[1], "{} FIRE | {} ALT-FIRE | {} WEAPONS", c::FIRE.token, c::ALT_FIRE.token, c::WEAPON_SELECT.token);
    let _ = write!(rows[2], "{} GRAPPLE/DASH | {} C4 | {} DETONATE", c::GRAPPLE.token, c::C4_PLANT.token, c::C4_DETONATE.token);
    rows
}

/// Rows of the bottom-center dock: every remaining tactical / system key.
pub fn center_dock_rows() -> [StackStr<96>; 5] {
    use crate::controls as c;
    let mut rows = [StackStr::<96>::new(); 5];
    let _ = write!(rows[0], "{} SLOW-MO | {} SLOW-MO SPEED | {} HAZARD DISCHARGE", c::SLOWMO.token, c::SLOWMO_RATE.token, c::PURGE.token);
    let _ = write!(rows[1], "{} STRESS SCANNER | {} MUSIC | {} WORKBENCH", c::SCANNER.token, c::MUSIC.token, c::WORKBENCH.token);
    let _ = write!(rows[2], "{} / {} PREV/NEXT LEVEL | {} JUMP | {} RESET", c::LEVEL_PREV.token, c::LEVEL_NEXT.token, c::LEVEL_JUMP.token, c::RESET.token);
    let _ = write!(rows[3], "{} CAMPAIGN | {} REPLAY | {} FLIGHT CAM", c::CAMPAIGN.token, c::REPLAY.token, c::FLIGHT_CAM.token);
    let _ = write!(rows[4], "{} INVERT PITCH | {} FREE CURSOR", c::INVERT_PITCH.token, c::ESCAPE.token);
    rows
}

/// Maximum lines in one F1 manual panel.
pub const MANUAL_PANEL_LINES: usize = 13;

/// One panel of the F1 controls manual.
pub struct ManualPanel {
    pub title: &'static str,
    pub color: Color,
    pub lines: [StackStr<64>; MANUAL_PANEL_LINES],
    pub len: usize,
}

impl ManualPanel {
    fn new(title: &'static str, color: Color) -> Self {
        Self { title, color, lines: [StackStr::new(); MANUAL_PANEL_LINES], len: 0 }
    }

    fn next_line(&mut self) -> &mut StackStr<64> {
        assert!(self.len < MANUAL_PANEL_LINES, "manual panel '{}' has too many lines", self.title);
        self.len += 1;
        &mut self.lines[self.len - 1]
    }

    fn binding(&mut self, b: &crate::controls::Binding) {
        let _ = write!(self.next_line(), "{} {}", b.token, b.action);
    }

    fn note(&mut self, text: &str) {
        let _ = write!(self.next_line(), "- {}", text);
    }

    pub fn lines(&self) -> &[StackStr<64>] {
        &self.lines[..self.len]
    }
}

/// Builds the eight F1 manual panels from the binding table.
pub fn manual_panels() -> [ManualPanel; 8] {
    use crate::controls as c;

    let mut movement = ManualPanel::new("1. MOVEMENT", Color::new(0.30, 0.90, 1.0, 1.0));
    for b in [&c::MOVE, &c::SPRINT, &c::JUMP, &c::GRAPPLE, &c::INVERT_PITCH] {
        movement.binding(b);
    }
    movement.note("GRAPPLE ANCHORS TO STRUCTURE IN RANGE");
    movement.note("NOTHING IN RANGE: JET-DASH INSTEAD");
    movement.note("PRESS GRAPPLE AGAIN TO LET GO");

    let mut select = ManualPanel::new("2. WEAPON SELECT", Color::new(1.0, 0.80, 0.20, 1.0));
    select.binding(&c::WEAPON_SELECT);
    for (key, mode) in c::WEAPON_KEYS {
        let _ = write!(select.next_line(), "[{}] {}", c::key_name(key), c::short_weapon_name(mode));
    }

    let mut firing = ManualPanel::new("3. FIRING & ALT-FIRES", Color::new(1.0, 0.50, 0.15, 1.0));
    firing.binding(&c::FIRE);
    firing.binding(&c::ALT_FIRE);
    for (_, mode) in c::WEAPON_KEYS {
        let _ = write!(firing.next_line(), "- {}: {}", c::short_weapon_name(mode), mode.alt_fire_name());
    }

    let mut demo = ManualPanel::new("4. C4 & SIEGE BATTERY", Color::new(1.0, 0.30, 0.25, 1.0));
    demo.binding(&c::C4_PLANT);
    demo.binding(&c::C4_DETONATE);
    let _ = write!(demo.next_line(), "- UP TO {} C4 CHARGES AT ONCE", crate::weapon::MAX_C4_CHARGES);
    for b in [&c::SIEGE_DEPLOY, &c::SIEGE_FIRE, &c::SIEGE_CYCLE, &c::SIEGE_RECALL] {
        demo.binding(b);
    }
    demo.note("ONE BATTERY PER TYPE");
    demo.note("DEPLOYING AGAIN MOVES IT");

    let mut tactical = ManualPanel::new("5. TACTICAL", Color::new(0.25, 0.90, 0.50, 1.0));
    for b in [&c::SLOWMO, &c::SLOWMO_RATE, &c::SCANNER, &c::PURGE, &c::MUSIC] {
        tactical.binding(b);
    }

    let mut campaign = ManualPanel::new("6. CONTRACTOR CAMPAIGN", Color::new(0.85, 0.40, 1.0, 1.0));
    for b in [&c::CAMPAIGN, &c::CONTRACT_START, &c::CONTRACT_NEXT, &c::CONTRACT_PREV] {
        campaign.binding(b);
    }
    campaign.note("CONTRACT KEYS WORK IN CAMPAIGN MODE");
    campaign.note("STAY UNDER THE COLLATERAL BUDGET");

    let mut replay = ManualPanel::new("7. REPLAY & FLIGHT CAM", Color::new(0.40, 0.80, 1.0, 1.0));
    replay.binding(&c::REPLAY);
    replay.binding(&c::FLIGHT_CAM);
    replay.note("WHILE A REPLAY IS PLAYING:");
    for b in [&c::REPLAY_PAUSE, &c::REPLAY_SCRUB, &c::REPLAY_SPEED, &c::REPLAY_ORBIT, &c::REPLAY_ZOOM] {
        replay.binding(b);
    }

    let mut system = ManualPanel::new("8. SYSTEM", Color::new(0.95, 0.85, 0.25, 1.0));
    for b in [&c::MANUAL, &c::ESCAPE, &c::WORKBENCH, &c::LEVEL_PREV, &c::LEVEL_NEXT, &c::RESET, &c::LEVEL_JUMP] {
        system.binding(b);
    }
    system.note("OTHER KEYS PAUSE WHILE THIS IS OPEN");

    [movement, select, firing, demo, tactical, campaign, replay, system]
}

/// Computes real-time seismic ground fault stress from active physics energy and shockwaves.
#[inline]
pub fn compute_seismic_stress(physics: &PhysicsSystem) -> f32 {
    let active_chunk_ke: f32 = physics.chunks.iter()
        .filter(|c| !c.settled)
        .map(|c| c.mass * (c.velocity.length() + c.angular_velocity.length() * 0.3))
        .sum();
    let shock_e: f32 = physics.shockwaves.iter()
        .map(|sw| sw.max_radius * (1.0f32 - sw.radius / sw.max_radius.max(0.01)).max(0.0f32))
        .sum();
    let debris_factor = physics.debris.len() as f32 * 0.0004;
    (active_chunk_ke * 0.00006 + shock_e * 0.04 + debris_factor).clamp(0.0, 1.0)
}

/// Computes (char, x_rel, y_rel, angle_rad) for characters curved along an arc around (0, 0).
#[allow(dead_code)]
pub fn compute_curved_text_layout(
    text: &str,
    radius: f32,
    is_bottom: bool,
    font_size: f32,
) -> Vec<(char, f32, f32, f32)> {
    if text.is_empty() || radius <= 0.001 {
        return Vec::new();
    }

    let kerning = 1.6f32;
    let mut total_pixel_width = 0.0f32;
    let mut char_count = 0usize;
    for ch in text.chars() {
        total_pixel_width += char_glyph_width(ch, font_size);
        char_count += 1;
    }
    if char_count == 0 {
        return Vec::new();
    }

    let total_arc_len = total_pixel_width + kerning * (char_count.saturating_sub(1) as f32);
    // Ensure effective radius is large enough that angular span does not exceed ~135 degrees (2.35 rad)
    let effective_radius = radius.max(total_arc_len / 2.35);
    let total_angle = total_arc_len / effective_radius;
    let mut cur_angle = -total_angle * 0.5;

    let mut result = Vec::with_capacity(char_count);
    for ch in text.chars() {
        let w = char_glyph_width(ch, font_size);
        let half_glyph_angle = (w * 0.5) / effective_radius;
        let glyph_angle = cur_angle + half_glyph_angle;

        // In screen coordinates (y is down):
        // is_bottom: angle 0 is +Y (down), x = r * sin(angle), y = r * cos(angle)
        // is_top: angle 0 is -Y (up), x = r * sin(angle), y = -r * cos(angle)
        let (rx, ry) = if is_bottom {
            (
                effective_radius * glyph_angle.sin() - w * 0.5,
                effective_radius * glyph_angle.cos() - font_size * 0.35,
            )
        } else {
            (
                effective_radius * glyph_angle.sin() - w * 0.5,
                -effective_radius * glyph_angle.cos() - font_size * 0.35,
            )
        };

        result.push((ch, rx, ry, glyph_angle));
        cur_angle += (w + kerning) / effective_radius;
    }

    result
}

/// Draw holographic status text curved along an orbital visor arc around (cx, cy).
/// Supports sci-fi melting / energy visor chromatic glow and radial pulse.
/// Performs zero heap allocations during the render loop.
#[allow(clippy::too_many_arguments)]
pub fn draw_curved_reticle_text(
    text: &str,
    cx: f32,
    cy: f32,
    radius: f32,
    is_bottom: bool,
    font_size: f32,
    color: Color,
    melt_time: f32,
) {
    if text.is_empty() || radius <= 0.001 {
        return;
    }

    let kerning = (font_size * 0.10).max(1.2);
    let mut total_pixel_width = 0.0f32;
    let mut char_count = 0usize;
    for ch in text.chars() {
        total_pixel_width += char_glyph_width(ch, font_size);
        char_count += 1;
    }
    if char_count == 0 {
        return;
    }

    let total_arc_len = total_pixel_width + kerning * (char_count.saturating_sub(1) as f32);
    let effective_radius = radius.max(total_arc_len / 2.35);
    let total_angle = total_arc_len / effective_radius;
    let mut cur_angle = -total_angle * 0.5;

    let glow_alpha = (color.a * 0.35).clamp(0.0, 1.0);
    let chromatic_aura = if is_bottom {
        Color::new(
            (color.r * 0.35 + 0.15).min(1.0),
            (color.g * 0.75 + 0.25).min(1.0),
            1.0,
            glow_alpha,
        )
    } else {
        Color::new(
            1.0,
            (color.g * 0.70 + 0.20).min(1.0),
            0.20,
            glow_alpha,
        )
    };

    for ch in text.chars() {
        let w = char_glyph_width(ch, font_size);
        let half_glyph_angle = (w * 0.5) / effective_radius;
        let glyph_angle = cur_angle + half_glyph_angle;

        let (rx, ry) = if is_bottom {
            (
                effective_radius * glyph_angle.sin() - w * 0.5,
                effective_radius * glyph_angle.cos() - font_size * 0.35,
            )
        } else {
            (
                effective_radius * glyph_angle.sin() - w * 0.5,
                -effective_radius * glyph_angle.cos() - font_size * 0.35,
            )
        };

        // Melting energy visor ripple: subtle radial vibration and wave
        let wave = (melt_time * 3.6 + glyph_angle * 4.0).sin();
        let wave_offset = wave * (font_size * 0.08).max(0.8);
        let normal_dir = if is_bottom {
            Vec2::new(glyph_angle.sin(), glyph_angle.cos())
        } else {
            Vec2::new(glyph_angle.sin(), -glyph_angle.cos())
        };

        let gx = cx + rx + normal_dir.x * wave_offset;
        let gy = cy + ry + normal_dir.y * wave_offset;

        let mut buf = [0u8; 4];
        let glyph_str: &str = ch.encode_utf8(&mut buf);

        // 1. Sci-fi Holographic Melting Chromatic Glow / Energy Halo
        draw_text(glyph_str, gx + 1.2, gy + 0.8, font_size, chromatic_aura);
        draw_text(
            glyph_str,
            gx - 1.0,
            gy - 0.6,
            font_size,
            Color::new(chromatic_aura.r, chromatic_aura.g, chromatic_aura.b, glow_alpha * 0.55),
        );

        // 2. Crisp Primary Glyph
        draw_text(glyph_str, gx, gy, font_size, color);

        cur_angle += (w + kerning) / effective_radius;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hud_hit_marker_registration_and_decay() {
        let mut hud = HUD::new();
        assert!(hud.hit_markers.is_empty());

        hud.register_hit(HitKind::Standard);
        assert_eq!(hud.hit_markers.len(), 1);
        assert_eq!(hud.hit_markers[0].kind, HitKind::Standard);

        hud.register_hit(HitKind::Critical);
        assert_eq!(hud.hit_markers.len(), 2);
        assert_eq!(hud.hit_markers[1].kind, HitKind::Critical);

        // Update with full timer should remove markers
        hud.update(0.35);
        assert!(hud.hit_markers.is_empty());
    }

    #[test]
    fn test_hud_notification_lifecycle() {
        let mut hud = HUD::new();
        hud.add_notification("TEST NOTIFICATION");
        assert_eq!(hud.collapse_notifications.len(), 1);

        hud.update(4.0);
        assert!(hud.collapse_notifications.is_empty());
    }

    #[test]
    fn test_cascade_notifications_keep_recent_visible_messages() {
        let mut hud = HUD::new();
        for index in 0..100 {
            hud.add_notification(&format!("COLLAPSE {index}"));
        }
        assert_eq!(hud.collapse_notifications.len(), MAX_NOTIFICATION_ROWS);
        assert_eq!(hud.collapse_notifications[0].0, "COLLAPSE 96");
        assert_eq!(hud.collapse_notifications[3].0, "COLLAPSE 99");
    }

    #[test]
    fn test_curved_reticle_text_layout_edge_cases_and_symmetry() {
        // 1. Empty string returns empty layout
        let empty_layout = compute_curved_text_layout("", 50.0, true, 12.0);
        assert!(empty_layout.is_empty());

        // 2. Zero or negative radius returns empty layout
        let zero_r_layout = compute_curved_text_layout("TEST", 0.0, true, 12.0);
        assert!(zero_r_layout.is_empty());

        // 3. Single character is centered near x = 0
        let single_layout = compute_curved_text_layout("A", 50.0, true, 12.0);
        assert_eq!(single_layout.len(), 1);
        let (_, rx, ry, angle) = single_layout[0];
        assert!(rx.abs() < 10.0, "Single character should be near center");
        assert!(ry > 30.0, "Bottom arc should place text below center");
        assert!(angle.abs() < 0.1, "Single character center angle should be near 0");

        // 4. Multi-character string symmetry
        let multi_layout = compute_curved_text_layout("HELLO", 60.0, true, 12.0);
        assert_eq!(multi_layout.len(), 5);
        let first_angle = multi_layout.first().unwrap().3;
        let last_angle = multi_layout.last().unwrap().3;
        assert!((first_angle + last_angle).abs() < 0.25, "Text layout should be symmetric across center");

        // 5. Top arc puts y in negative relative coords
        let top_layout = compute_curved_text_layout("CRITICAL", 60.0, false, 12.0);
        assert_eq!(top_layout.len(), 8);
        for (_, _, ry, _) in &top_layout {
            assert!(*ry < 0.0, "Top arc characters must be above center (negative relative y)");
        }
    }

    #[test]
    fn test_ui_fit_text_to_width() {
        let short = "SHORT";
        assert_eq!(fit_text_to_width(short, 100.0, 14.0), "SHORT");

        let long = "THIS IS A VERY LONG STRING THAT WILL DEFINITELY EXCEED THE ALLOWED BOUNDING WIDTH";
        let fitted = fit_text_to_width(long, 80.0, 14.0);
        assert!(fitted.ends_with("..."));
        assert!(fitted.len() < long.len());
    }

    #[test]
    fn test_fit_text_handles_width_smaller_than_ellipsis() {
        for width in [0.0, 1.0, 7.0, 12.0, 18.0] {
            let fitted = fit_text_to_width("LONG LABEL", width, 14.0);
            assert!(text_width(&fitted, 14.0) <= width);
            assert_eq!(fit_text_to_width_stack::<32>("LONG LABEL", width, 14.0).as_str(), fitted);
        }
    }

    #[test]
    fn test_ui_top_visor_inner_elements_zero_text_collision() {
        let test_resolutions = [
            (800.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
        ];

        for &(sw, sh) in &test_resolutions {
            let s = compute_ui_scale(sw, sh);
            let top_w = (sw * 0.76).clamp(560.0 * s, 1180.0 * s);
            let top_x = (sw - top_w) * 0.5;
            let bar_w = (top_w * 0.28).clamp(140.0 * s, 260.0 * s);
            let bar_x = top_x + (top_w - bar_w) * 0.5;
            let bar_right = bar_x + bar_w;

            let max_left_w = (bar_x - (top_x + 14.0 * s) - 20.0 * s).max(60.0 * s);

            let levels = LevelPreset::all();

            for level in levels {
                let name = level.name();
                let fit_name = fit_text_to_width(name, max_left_w, 14.0 * s);
                let name_w = text_width(&fit_name, 14.0 * s);
                let name_right = top_x + 14.0 * s + name_w;

                assert!(
                    name_right <= bar_x - 18.0 * s,
                    "Level {} title crosses into central bar clearance on {}x{}! right={}, bar_x={}",
                    level.name(), sw, sh, name_right, bar_x
                );

                let dim_str = format!("[{}]", level.dimensions());
                let fit_dim = fit_text_to_width(&dim_str, max_left_w, 10.5 * s);
                let dim_w = text_width(&fit_dim, 10.5 * s);
                let dim_right = top_x + 14.0 * s + dim_w;

                assert!(
                    dim_right <= bar_x - 18.0 * s,
                    "Dimensions string crosses into central bar clearance on {}x{}!", sw, sh
                );
            }

            let rebar_str = "REBAR: 100%  |  FPS: 144";
            let rw = text_width(rebar_str, 13.5 * s);
            let min_right_x = bar_right + 20.0 * s;
            let rebar_x = (top_x + top_w - rw - 14.0 * s).max(min_right_x);

            assert!(
                rebar_x >= bar_right + 19.0 * s,
                "Right telemetry crosses into central bar clearance on {}x{}! rebar_x={}, bar_right={}",
                sw, sh, rebar_x, bar_right
            );
        }
    }

    #[test]
    fn test_ui_tactical_dock_inner_text_fits_within_card_bounds() {
        let test_resolutions = [
            (800.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
        ];

        for &(sw, sh) in &test_resolutions {
            let s = compute_ui_scale(sw, sh);
            let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
            let avail_w = dock_w - 24.0 * s;

            for (i, row) in siege_hint_rows().iter().enumerate() {
                let w = text_width(row.as_str(), 11.0 * s);
                assert!(w <= avail_w, "Siege hint row {} overflows dock on {}x{}! w={}, avail={}", i, sw, sh, w, avail_w);
            }

            for (i, row) in control_strip_rows().iter().enumerate() {
                let w = text_width(row.as_str(), 11.0 * s);
                assert!(w <= avail_w, "Control strip row {} overflows dock on {}x{}! w={}, avail={}", i, sw, sh, w, avail_w);
            }
        }
    }

    #[test]
    fn test_ui_compass_ribbon_and_seismic_meter_bounds() {
        let test_resolutions = [
            (800.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (3840.0, 2160.0),
        ];

        for &(sw, sh) in &test_resolutions {
            let s = compute_ui_scale(sw, sh);
            let top_w = (sw * 0.76).clamp(560.0 * s, 1180.0 * s);
            let top_x = (sw - top_w) * 0.5;
            let cx = sw * 0.5;

            let compass_w = (top_w * 0.44).clamp(240.0 * s, 400.0 * s);
            let compass_x = cx - compass_w * 0.5;
            let compass_right = compass_x + compass_w;

            let seis_w = (top_w * 0.24).clamp(140.0 * s, 220.0 * s);
            let seis_x = top_x + top_w - seis_w;

            assert!(
                seis_x >= compass_right,
                "Seismic meter collides with compass ribbon on {}x{}! seis_x={}, compass_right={}",
                sw, sh, seis_x, compass_right
            );
        }
    }

    #[test]
    fn test_ui_seismic_stress_computation() {
        let mut physics = PhysicsSystem::new();
        let base_stress = compute_seismic_stress(&physics);
        assert_eq!(base_stress, 0.0);

        for i in 0..500 {
            physics.debris.push(crate::physics::DebrisParticle {
                position: Vec3::new(i as f32 * 0.1, 5.0, 0.0),
                velocity: Vec3::new(0.0, -1.0, 0.0),
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                size: 0.2,
                color: WHITE,
                life: 3.0,
                max_life: 3.0,
                bounces: 0,
            });
        }
        let debris_stress = compute_seismic_stress(&physics);
        assert!(debris_stress > 0.1);
        assert!(debris_stress <= 1.0);
    }

    #[test]
    fn test_ui_c4_grid_indicator_bounds() {
        let test_resolutions = [
            (800.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
        ];

        for &(sw, sh) in &test_resolutions {
            let s = compute_ui_scale(sw, sh);
            let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
            let c4_w = dock_w;
            let pip_area_w = c4_w - 20.0 * s;
            let total_pips = crate::weapon::MAX_C4_CHARGES;
            let pip_spacing = 2.0 * s;
            let pip_w = (pip_area_w - (total_pips - 1) as f32 * pip_spacing) / total_pips as f32;

            assert!(pip_w > 1.0, "Pip width must be > 1.0px at {}x{}, got {}", sw, sh, pip_w);
            let total_spanned = total_pips as f32 * pip_w + (total_pips - 1) as f32 * pip_spacing;
            assert!((total_spanned - pip_area_w).abs() < 0.01, "Pips must exactly span pip_area_w");

            // Verify header text fit
            let header = c4_header(24);
            let header_fit = fit_text_to_width(header.as_str(), c4_w - 20.0 * s, 11.5 * s);
            assert_eq!(header_fit, header.as_str());
        }
    }

    #[test]
    fn test_stack_str_formatting_and_reuse() {
        let mut s = StackStr::<32>::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.capacity(), 32);
        assert_eq!(s.remaining(), 32);

        write!(s, "INTEGRITY: {:.1}%", 84.5).unwrap();
        assert_eq!(s.as_str(), "INTEGRITY: 84.5%");
        assert_eq!(&*s, "INTEGRITY: 84.5%");
        assert_eq!(s.len(), 16);
        assert_eq!(s.remaining(), 16);

        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.as_str(), "");

        write!(s, "FPS: {}", 60).unwrap();
        assert_eq!(s.as_str(), "FPS: 60");
    }

    #[test]
    fn test_stack_str_capacity_truncation_and_utf8_safety() {
        let mut s = StackStr::<10>::new();
        write!(s, "0123456789EXTRA").unwrap();
        assert_eq!(s.len(), 10);
        assert_eq!(s.as_str(), "0123456789");

        // Multi-byte UTF-8 character boundary safety
        let mut s2 = StackStr::<5>::new();
        s2.push_str("ABC");
        s2.push_str("⚡"); // glyph-check: allow: 3-byte char with 2 bytes remaining: must not split char
        assert_eq!(s2.len(), 3);
        assert_eq!(s2.as_str(), "ABC");
    }

    #[test]
    fn test_stack_str_traits_and_display() {
        let mut s = StackStr::<48>::new();
        write!(s, "TEST").unwrap();
        assert_eq!(format!("{}", s), "TEST");
        assert_eq!(format!("{:?}", s), "\"TEST\"");
        assert_eq!(s, "TEST");
        assert_eq!(s.as_str(), "TEST");
    }

    #[test]
    fn test_fit_text_to_width_stack_equivalence() {
        let short = "SHORT";
        let long = "VERY_LONG_STRUCTURAL_ELEMENT_NAME_EXCEEDING_LIMIT";

        let stack_short = fit_text_to_width_stack::<64>(short, 100.0, 14.0);
        assert_eq!(stack_short.as_str(), fit_text_to_width(short, 100.0, 14.0));

        let stack_long = fit_text_to_width_stack::<64>(long, 80.0, 14.0);
        assert_eq!(stack_long.as_str(), fit_text_to_width(long, 80.0, 14.0));
        assert!(stack_long.as_str().ends_with("..."));
    }

    #[test]
    fn test_onscreen_controls_guidance_and_help_modal() {
        let mut hud = HUD::new();
        assert!(!hud.help_modal_open, "Help modal should default to closed");
        hud.help_modal_open = true;
        assert!(hud.help_modal_open, "Help modal toggle state preserved");

        // Verify all 10 weapon modes define valid, non-empty alt-fire guidance names
        let modes = [
            crate::weapon::WeaponMode::TitanRam,
            crate::weapon::WeaponMode::HavocMortar,
            crate::weapon::WeaponMode::EventHorizon,
            crate::weapon::WeaponMode::SolsticeLance,
            crate::weapon::WeaponMode::ApexRailgun,
            crate::weapon::WeaponMode::FrostbiteMortar,
            crate::weapon::WeaponMode::SeismicDisrupter,
            crate::weapon::WeaponMode::TeslaCaster,
            crate::weapon::WeaponMode::PlasmaFlamer,
            crate::weapon::WeaponMode::PlasmaWireSaw,
        ];
        for mode in &modes {
            let alt_name = mode.alt_fire_name();
            assert!(!alt_name.is_empty(), "Weapon mode {:?} must have non-empty alt-fire name", mode);
        }

        // Test center dock guidance fit across standard resolutions
        let test_resolutions = [
            (800.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
        ];

        for &(sw, sh) in &test_resolutions {
            let s = compute_ui_scale(sw, sh);
            let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
            let dock_x = 16.0 * s;
            let card_w = (sw * 0.28).clamp(280.0 * s, 400.0 * s);
            let card_x = sw - card_w - 16.0 * s;
            let center_x = dock_x + dock_w + 10.0 * s;
            let center_w = card_x - center_x - 10.0 * s;

            if center_w >= 180.0 * s {
                let rows = center_dock_rows();
                for (i, row) in rows.iter().enumerate() {
                    let fit = fit_text_to_width_stack::<96>(row.as_str(), center_w - 16.0 * s, 10.5 * s);
                    assert!(!fit.as_str().is_empty(), "Center dock row {} empty at {}x{}", i, sw, sh);
                }
            }
        }
    }
}
