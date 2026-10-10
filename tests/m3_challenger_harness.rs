#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use macroquad::prelude::*;
use voxel::VoxelGrid;
use physics::{PhysicsSystem, SmokeParticle, ShockwaveRing};
use ui::compute_ui_scale;

#[derive(Debug, Clone, Copy)]
pub struct RectBounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectBounds {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn intersects(&self, other: &RectBounds) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }
    pub fn is_within(&self, sw: f32, sh: f32) -> bool {
        self.x >= 0.0 && self.y >= 0.0 && self.right() <= sw && self.bottom() <= sh
    }
}

pub struct HUDLayoutBounds {
    pub scale: f32,
    pub dock: RectBounds,
    pub weapon_card: RectBounds,
    pub top_visor: RectBounds,
    pub scanner_bar: RectBounds,
    pub reticle_bounds: RectBounds,
}

pub fn compute_hud_layout_bounds(sw: f32, sh: f32) -> HUDLayoutBounds {
    let s = compute_ui_scale(sw, sh);
    let cx = sw * 0.5;
    let cy = sh * 0.5;

    let max_bloom_offset = 24.0 * s;
    let bracket_len = 6.5 * s;
    let reticle = RectBounds::new(
        cx - max_bloom_offset - bracket_len,
        cy - max_bloom_offset - bracket_len,
        (max_bloom_offset + bracket_len) * 2.0,
        (max_bloom_offset + bracket_len) * 2.0,
    );

    let top_w = (sw * 0.76).clamp(560.0 * s, 1180.0 * s);
    let top_h = 58.0 * s;
    let top_x = (sw - top_w) * 0.5;
    let top_y = 10.0 * s;
    let top_visor = RectBounds::new(top_x, top_y, top_w, top_h);

    let sc_h = 24.0 * s;
    let sc_y = top_y + top_h + 4.0 * s;
    let scanner_bar = RectBounds::new(top_x, sc_y, top_w, sc_h);

    let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
    let dock_x = 16.0 * s;
    let total_dock_h = 124.0 * s;
    let dock_y = sh - total_dock_h - 16.0 * s;
    let dock = RectBounds::new(dock_x, dock_y, dock_w, total_dock_h);

    let card_w = (sw * 0.28).clamp(280.0 * s, 400.0 * s);
    let card_h = 112.0 * s;
    let card_x = sw - card_w - 16.0 * s;
    let card_y = sh - card_h - 16.0 * s;
    let weapon_card = RectBounds::new(card_x, card_y, card_w, card_h);

    HUDLayoutBounds {
        scale: s,
        dock,
        weapon_card,
        top_visor,
        scanner_bar,
        reticle_bounds: reticle,
    }
}

pub struct WorkbenchLayoutBounds {
    pub scale: f32,
    pub panel: RectBounds,
    pub content: RectBounds,
    pub tab_bar: RectBounds,
    pub tab_width: f32,
}

pub fn compute_workbench_bounds(sw: f32, sh: f32) -> WorkbenchLayoutBounds {
    let s = compute_ui_scale(sw, sh);
    let pw = (sw * 0.94).min(1260.0 * s);
    let ph = (sh * 0.92).min(780.0 * s);
    let px = (sw - pw) * 0.5;
    let py = (sh - ph) * 0.5;

    let head_h = 44.0 * s;
    let tab_y = py + head_h + 6.0 * s;
    let tab_h = 36.0 * s;
    let tab_w = (pw - 24.0 * s - 4.0 * 6.0 * s) / 5.0;

    let content_x = px + 16.0 * s;
    let content_y = tab_y + tab_h + 10.0 * s;
    let content_w = pw - 32.0 * s;
    let content_h = py + ph - content_y - 10.0 * s;

    WorkbenchLayoutBounds {
        scale: s,
        panel: RectBounds::new(px, py, pw, ph),
        content: RectBounds::new(content_x, content_y, content_w, content_h),
        tab_bar: RectBounds::new(px + 12.0 * s, tab_y, pw - 24.0 * s, tab_h),
        tab_width: tab_w,
    }
}

// ============================================================================
// CHALLENGER VERIFICATION SUITE 1: Smoke Lifetime Clamp Ceiling & Opacity Continuity
// ============================================================================

#[test]
fn test_adversarial_smoke_lifetime_clamp_exhaustive() {
    let pool_sizes: [usize; 10] = [721, 750, 800, 899, 900, 901, 950, 1000, 1500, 2000];
    let initial_lifetimes = [0.001, 0.05, 0.10, 0.12, 0.12001, 0.15, 0.50, 1.0, 4.0, 10.0];

    for &count in &pool_sizes {
        for &init_life in &initial_lifetimes {
            let mut phys = PhysicsSystem::new();
            for _ in 0..count {
                phys.smoke.push(SmokeParticle {
                    position: Vec3::ZERO,
                    velocity: Vec3::ZERO,
                    size: 0.5,
                    start_size: 0.5,
                    max_size: 2.0,
                    color: Color::new(1.0, 1.0, 1.0, 1.0),
                    life: init_life,
                    max_life: init_life * 2.0, // initial ratio = 0.50
                });
            }

            let pre_count = phys.smoke.len();
            phys.enforce_particle_caps();
            let post_count = phys.smoke.len();

            // Zero .drain() assertion: length must be preserved in the frame caps are enforced
            assert_eq!(
                post_count, pre_count,
                "Particle count must NOT abruptly drop via drain() upon cap enforcement (count={})",
                count
            );

            // 1. Check Soft Cap excess particles
            let excess = count.saturating_sub(PhysicsSystem::SOFT_SMOKE);

            for (rank, p) in phys.smoke.iter().take(excess).enumerate() {
                // Ceiling invariant: NO smoke particle subjected to cap enforcement can exceed 0.12s
                assert!(
                    p.life <= 0.120001,
                    "Smoke particle rank {}/{} lifetime ({}) exceeded ceiling of 0.12s (pool_size={}, init_life={})",
                    rank, excess, p.life, count, init_life
                );

                // Lower bound invariant:
                // If subject to Tier 2 hard cap (rank < hard_excess), floor is 0.01s.
                // Otherwise (Tier 1 soft cap), floor is 0.02s.
                let is_hard_excess = count > PhysicsSystem::MAX_SMOKE && rank < (count - PhysicsSystem::MAX_SMOKE);
                let floor = if is_hard_excess { 0.0099 } else { 0.0199 };
                if init_life >= 0.02 {
                    assert!(
                        p.life >= floor,
                        "Smoke particle rank {}/{} lifetime ({}) dropped below floor {}",
                        rank, excess, p.life, floor
                    );
                }

                // Opacity ratio continuity invariant: R = life / max_life must be preserved
                let ratio = p.life / p.max_life;
                let expected_ratio = (init_life / (init_life * 2.0)).clamp(0.01, 1.0); // 0.5
                assert!(
                    (ratio - expected_ratio).abs() < 1e-3,
                    "Smoke opacity ratio discontinuity: expected {}, got {} at rank {}/{}",
                    expected_ratio, ratio, rank, excess
                );
            }

            // 2. Check Hard Cap excess particles if count > MAX_SMOKE (900)
            if count > PhysicsSystem::MAX_SMOKE {
                let hard_excess = count - PhysicsSystem::MAX_SMOKE;
                for (rank, p) in phys.smoke.iter().take(hard_excess).enumerate() {
                    assert!(
                        p.life <= 0.040001,
                        "Hard excess smoke particle rank {}/{} lifetime ({}) exceeded hard cap ceiling 0.04s",
                        rank, hard_excess, p.life
                    );
                }
            }
        }
    }
}

#[test]
fn test_adversarial_smoke_natural_decay_over_time() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = audio::SoundSystem::empty();
    let initial_count = 1200; // Above both SOFT_SMOKE (720) and MAX_SMOKE (900)

    for _ in 0..initial_count {
        phys.smoke.push(SmokeParticle {
            position: Vec3::new(5.0, 5.0, 5.0),
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 3.0,
            max_life: 3.0,
        });
    }

    // Step physics with dt = 0.01667 (60 FPS)
    let dt = 1.0 / 60.0;
    let mut prev_count = initial_count;

    // Simulate 15 frames (~0.25 seconds)
    for frame in 1..=15 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
        let curr_count = phys.smoke.len();
        // Count should monotonically decrease as accelerated particles reach life <= 0
        assert!(
            curr_count <= prev_count,
            "Frame {}: smoke count increased from {} to {}",
            frame, prev_count, curr_count
        );
        prev_count = curr_count;
    }

    // After 15 frames (0.25s), all hard-excess particles (capped to <=0.04s) and soft-excess particles (capped to <=0.12s)
    // should have expired naturally via swap_remove
    assert!(
        phys.smoke.len() <= PhysicsSystem::SOFT_SMOKE,
        "After 0.25s of accelerated decay, smoke count ({}) should be <= SOFT_SMOKE ({})",
        phys.smoke.len(), PhysicsSystem::SOFT_SMOKE
    );
}

// ============================================================================
// CHALLENGER VERIFICATION SUITE 2: Dust Storm Volume Scaling & Monotonicity
// ============================================================================

#[test]
fn test_adversarial_dust_storm_scaling_dense_sweep() {
    let test_volumes = [
        0, 1, 2, 5, 10, 17, 18, 19, 36, 54, 100, 200, 323, 324, 325,
        400, 500, 750, 1000, 1151, 1152, 1153, 1500, 2000, 5000, 50000,
    ];

    let mut last_dust_count = 0;
    let mut last_spall_count = 0;

    for &vol in &test_volumes {
        let mut phys = PhysicsSystem::new();
        phys.smoke.clear();
        phys.debris.clear();

        let center = Vec3::new(15.0, 8.0, 15.0);
        let radius = 6.0;
        phys.spawn_structural_collapse_dust_storm(center, radius, vol);

        let dust_count = phys.smoke.len();
        let spall_count = phys.debris.len();

        let expected_dust = (vol / 18).clamp(18, 64);
        let expected_spall = (expected_dust / 2).clamp(12, 32);

        // Exact formula matching
        assert_eq!(
            dust_count, expected_dust,
            "Volume {vol}: expected {expected_dust} smoke particles, got {dust_count}"
        );
        assert_eq!(
            spall_count, expected_spall,
            "Volume {vol}: expected {expected_spall} spall debris particles, got {spall_count}"
        );

        // Clamp bounds verification
        assert!(dust_count >= 18, "Dust count {dust_count} below minimum clamp 18 for vol {vol}");
        assert!(dust_count <= 64, "Dust count {dust_count} above maximum clamp 64 for vol {vol}");
        assert!(spall_count >= 12, "Spall count {spall_count} below minimum clamp 12 for vol {vol}");
        assert!(spall_count <= 32, "Spall count {spall_count} above maximum clamp 32 for vol {vol}");

        // Monotonicity verification
        assert!(
            dust_count >= last_dust_count,
            "Dust count decreased from {last_dust_count} to {dust_count} as volume increased to {vol}"
        );
        assert!(
            spall_count >= last_spall_count,
            "Spall count decreased from {last_spall_count} to {spall_count} as volume increased to {vol}"
        );

        // Verify particle attributes
        for sm in &phys.smoke {
            assert!(sm.position.is_finite(), "Smoke particle position must be finite");
            assert!(sm.velocity.is_finite(), "Smoke particle velocity must be finite");
            assert!(sm.life > 0.0, "Smoke particle life must be positive");
            assert!(sm.size > 0.0, "Smoke particle size must be positive");
        }
        for d in &phys.debris {
            assert!(d.position.is_finite(), "Debris particle position must be finite");
            assert!(d.velocity.is_finite(), "Debris particle velocity must be finite");
            assert!(d.life > 0.0, "Debris particle life must be positive");
        }

        last_dust_count = dust_count;
        last_spall_count = spall_count;
    }
}

#[test]
fn test_adversarial_dust_storm_pool_capacity_saturation() {
    let mut phys = PhysicsSystem::new();
    // Fill smoke pool to 895 (5 slots below MAX_SMOKE = 900)
    for _ in 0..895 {
        phys.smoke.push(SmokeParticle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 3.0,
            max_life: 3.0,
        });
    }

    // Try to spawn large dust storm (requires 64 particles)
    phys.spawn_structural_collapse_dust_storm(Vec3::new(10.0, 5.0, 10.0), 5.0, 2000);

    // Assert pool was safely bounded at MAX_SMOKE (900)
    assert_eq!(
        phys.smoke.len(),
        PhysicsSystem::MAX_SMOKE,
        "Dust storm spawning must respect MAX_SMOKE headroom and not exceed 900"
    );
}

// ============================================================================
// CHALLENGER VERIFICATION SUITE 3: Shockwave Speed Acceleration & Expiration
// ============================================================================

#[test]
fn test_adversarial_shockwave_speed_acceleration() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 56; // SOFT_SHOCKWAVES = 48, excess = 8

    for i in 0..initial_count {
        phys.shockwaves.push(ShockwaveRing {
            position: Vec3::new(i as f32, 0.0, 0.0),
            normal: Vec3::Y,
            radius: 1.0,
            max_radius: 10.0,
            speed: 5.0, // baseline speed
            thickness: 0.4,
            color: Color::new(1.0, 0.5, 0.0, 1.0),
        });
    }

    let excess = initial_count - PhysicsSystem::SOFT_SHOCKWAVES;
    phys.enforce_particle_caps();

    // Check that all excess shockwaves had their expansion speed accelerated
    for (rank, sw) in phys.shockwaves.iter().take(excess).enumerate() {
        let rank_factor = (1.0 - (rank as f32 / excess as f32)).max(0.05);
        let rem = (sw.max_radius - sw.radius).max(0.05);
        let target_time = (0.12 * rank_factor).max(0.02);
        let min_expected_speed = rem / target_time;

        assert!(
            sw.speed >= min_expected_speed - 1e-3,
            "Shockwave rank {}/{} speed ({}) was not accelerated to expected speed ({})",
            rank, excess, sw.speed, min_expected_speed
        );
        assert!(
            sw.speed > 5.0,
            "Shockwave speed ({}) must exceed initial unaccelerated speed 5.0",
            sw.speed
        );
    }
}

#[test]
fn test_adversarial_shockwave_smooth_expiration_via_step() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = audio::SoundSystem::empty();
    let initial_count = 72; // Above SOFT (48) and MAX (64)

    for i in 0..initial_count {
        phys.shockwaves.push(ShockwaveRing {
            position: Vec3::new(i as f32, 0.0, 0.0),
            normal: Vec3::Y,
            radius: 2.0,
            max_radius: 12.0,
            speed: 6.0,
            thickness: 0.4,
            color: Color::new(1.0, 0.5, 0.0, 1.0),
        });
    }

    let dt = 1.0 / 60.0;
    let mut prev_count = initial_count;

    // Simulate 15 frames (~0.25 seconds)
    for frame in 1..=15 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
        let curr_count = phys.shockwaves.len();

        // Ensure count never increases and transitions smoothly
        assert!(
            curr_count <= prev_count,
            "Frame {}: shockwave count unexpectedly increased from {} to {}",
            frame, prev_count, curr_count
        );
        prev_count = curr_count;
    }

    // All accelerated shockwaves must have expanded past max_radius and cleanly expired
    assert!(
        phys.shockwaves.len() <= PhysicsSystem::SOFT_SHOCKWAVES,
        "After 15 frames of accelerated expansion, shockwaves count ({}) must settle to <= SOFT_SHOCKWAVES ({})",
        phys.shockwaves.len(), PhysicsSystem::SOFT_SHOCKWAVES
    );
}

// ============================================================================
// CHALLENGER VERIFICATION SUITE 4: Cross-Resolution UI Invariants & Collision
// ============================================================================

#[test]
fn test_adversarial_cross_resolution_ui_invariants_exhaustive() {
    let test_viewports = [
        // Standard Desktop & Laptop Resolutions
        ("720p HD", 1280.0, 720.0),
        ("1080p FHD", 1920.0, 1080.0),
        ("1440p QHD", 2560.0, 1440.0),
        ("4K UHD", 3840.0, 2160.0),
        ("5K Retina", 5120.0, 2880.0),
        ("8K UHD", 7680.0, 4320.0),

        // Ultrawide Viewports (21:9, 24:10, 32:9)
        ("21:9 UW-FHD", 2560.0, 1080.0),
        ("21:9 UW-QHD", 3440.0, 1440.0),
        ("24:10 UW", 3840.0, 1600.0),
        ("32:9 Super UW", 5120.0, 1440.0),
        ("21:9 5K2K", 5120.0, 2160.0),

        // Productivity 16:10 and 3:2 Ratios
        ("16:10 WXGA", 1280.0, 800.0),
        ("16:10 WSXGA+", 1680.0, 1050.0),
        ("16:10 WUXGA", 1920.0, 1200.0),
        ("16:10 WQXGA", 2560.0, 1600.0),
        ("16:10 Retina", 2880.0, 1800.0),
        ("3:2 Surface", 3000.0, 2000.0),
        ("16:10 4K+", 3840.0, 2400.0),

        // Legacy & Low-Res Viewports
        ("Low-Res 16:9", 640.0, 360.0),
        ("4:3 VGA", 640.0, 480.0),
        ("4:3 SVGA", 800.0, 600.0),
        ("4:3 XGA", 1024.0, 768.0),
        ("5:4 SXGA", 1280.0, 1024.0),
        ("16:9 HD+", 1600.0, 900.0),
    ];

    for &(name, sw, sh) in &test_viewports {
        let s = compute_ui_scale(sw, sh);
        let layout = compute_hud_layout_bounds(sw, sh);
        let wb = compute_workbench_bounds(sw, sh);

        // 1. Scale Clamp Invariant
        assert!(
            (0.65..=3.20).contains(&s),
            "Resolution {name} ({sw}x{sh}): scale {s} out of bounds [0.65, 3.20]"
        );

        // 2. Tactical Dock Boundary Invariant
        assert!(
            layout.dock.is_within(sw, sh),
            "Resolution {name} ({sw}x{sh}): Tactical Dock outside viewport: {:?}",
            layout.dock
        );

        // 3. Weapon Card Boundary Invariant
        assert!(
            layout.weapon_card.is_within(sw, sh),
            "Resolution {name} ({sw}x{sh}): Weapon Card outside viewport: {:?}",
            layout.weapon_card
        );

        // 4. Guaranteed Zero Collision Invariant between Dock and Weapon Card
        assert!(
            !layout.dock.intersects(&layout.weapon_card),
            "Resolution {name} ({sw}x{sh}): Dock collides with Weapon Card! Dock={:?}, Card={:?}",
            layout.dock, layout.weapon_card
        );

        // 5. Positive Horizontal Clearance Gap
        let gap = layout.weapon_card.x - layout.dock.right();
        assert!(
            gap > 0.0,
            "Resolution {name} ({sw}x{sh}): Clearance gap non-positive: {gap} px"
        );
        if sw >= 1280.0 {
            assert!(
                gap >= 200.0,
                "Resolution {name} ({sw}x{sh}): Expected >= 200px gap for widescreen, got {gap} px"
            );
        }

        // 6. Top Visor Bar Invariants
        assert!(
            layout.top_visor.is_within(sw, sh),
            "Resolution {name} ({sw}x{sh}): Top Visor outside viewport: {:?}",
            layout.top_visor
        );
        let expected_top_x = (sw - layout.top_visor.w) * 0.5;
        assert!(
            (layout.top_visor.x - expected_top_x).abs() < 0.1,
            "Resolution {name} ({sw}x{sh}): Top Visor not centered: got {}, expected {}",
            layout.top_visor.x, expected_top_x
        );

        // 7. Scanner Bar Invariants
        assert!(
            layout.scanner_bar.is_within(sw, sh),
            "Resolution {name} ({sw}x{sh}): Scanner bar outside viewport: {:?}",
            layout.scanner_bar
        );
        assert!(
            layout.scanner_bar.y >= layout.top_visor.bottom(),
            "Resolution {name} ({sw}x{sh}): Scanner bar overlaps Top Visor: scanner.y={}, visor.bottom={}",
            layout.scanner_bar.y, layout.top_visor.bottom()
        );

        // 8. Reticle Invariant
        assert!(
            layout.reticle_bounds.is_within(sw, sh),
            "Resolution {name} ({sw}x{sh}): Reticle outside viewport: {:?}",
            layout.reticle_bounds
        );

        // 9. Workbench Panel Invariants
        assert!(
            wb.panel.is_within(sw, sh),
            "Resolution {name} ({sw}x{sh}): Workbench panel outside viewport: {:?}",
            wb.panel
        );
        assert!(
            wb.tab_bar.y >= wb.panel.y && wb.tab_bar.bottom() <= wb.panel.bottom(),
            "Resolution {name} ({sw}x{sh}): Tab bar outside workbench panel"
        );
        assert!(
            wb.tab_width > 0.0,
            "Resolution {name} ({sw}x{sh}): Tab width non-positive: {}",
            wb.tab_width
        );
    }
}
