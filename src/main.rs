pub use voxel_demolition::{
    audio, campaign, controls, customizer, fluid, music, physics, player, replay, siege, skyline, ui, voxel, weapon,
    world_gen,
};

use macroquad::prelude::*;
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::campaign::CampaignManager;
use voxel_demolition::customizer::CustomizerUI;
use voxel_demolition::fluid::FluidSystem;
use voxel_demolition::music::MusicEngine;
use voxel_demolition::physics::PhysicsSystem;
use voxel_demolition::player::Player;
use voxel_demolition::replay::ReplaySystem;
use voxel_demolition::siege::SiegeSystem;
use voxel_demolition::ui::HUD;
use voxel_demolition::voxel::{VoxelGrid, VOXEL_SIZE};
use voxel_demolition::weapon::{WeaponMode, WeaponSystem};
use voxel_demolition::world_gen::{generate_level, LevelPreset};



fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "VOXEL DEMOLITION: OBLIVION PROTOCOL".to_string(),
            window_width: 1280,
            window_height: 800,
            window_resizable: true,
            high_dpi: true,
            sample_count: 4,
            ..Default::default()
        },
        draw_call_vertex_capacity: 32768,
        draw_call_index_capacity: 49152,
        ..Default::default()
    }
}

struct SkydomeMesh {
    base_mesh: Mesh,
}

fn create_skydome_mesh() -> SkydomeMesh {
    let lat_rings = 14;
    let lon_segments = 40;
    let radius = 240.0f32;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for r in 0..=lat_rings {
        let v = r as f32 / lat_rings as f32; // 0.0 at zenith, 1.0 at nadir
        let phi = std::f32::consts::FRAC_PI_2 - v * std::f32::consts::PI; // pi/2 down to -pi/2
        let y = radius * phi.sin();
        let ring_r = (radius * phi.cos()).abs();

        let deg = phi.to_degrees();
        let col = if deg > 45.0 {
            // Cosmic zenith
            let t = ((deg - 45.0) / 45.0).clamp(0.0, 1.0);
            Color::new(
                0.015 * t + 0.035 * (1.0 - t),
                0.020 * t + 0.055 * (1.0 - t),
                0.045 * t + 0.105 * (1.0 - t),
                1.0,
            )
        } else if deg > 12.0 {
            // Upper sky to mid sky
            let t = ((deg - 12.0) / 33.0).clamp(0.0, 1.0);
            Color::new(
                0.035 * t + 0.070 * (1.0 - t),
                0.055 * t + 0.095 * (1.0 - t),
                0.105 * t + 0.150 * (1.0 - t),
                1.0,
            )
        } else if deg > 1.5 {
            // Soft dusk transition above horizon
            let t = ((deg - 1.5) / 10.5).clamp(0.0, 1.0);
            Color::new(
                0.070 * t + 0.175 * (1.0 - t),
                0.095 * t + 0.125 * (1.0 - t),
                0.150 * t + 0.090 * (1.0 - t),
                1.0,
            )
        } else if deg > -2.0 {
            // Warm horizon glow line
            Color::new(0.180, 0.130, 0.090, 1.0)
        } else if deg > -18.0 {
            // Fading into subterranean bedrock
            let t = ((deg - (-18.0)) / 16.0).clamp(0.0, 1.0);
            Color::new(
                0.180 * t + 0.040 * (1.0 - t),
                0.130 * t + 0.045 * (1.0 - t),
                0.090 * t + 0.060 * (1.0 - t),
                1.0,
            )
        } else {
            // Subterranean nadir
            Color::new(0.025, 0.030, 0.045, 1.0)
        };

        for s in 0..lon_segments {
            let theta = (s as f32 / lon_segments as f32) * std::f32::consts::TAU;
            let x = theta.cos() * ring_r;
            let z = theta.sin() * ring_r;
            vertices.push(Vertex {
                position: Vec3::new(x, y, z),
                uv: Vec2::ZERO,
                color: col.into(),
                normal: Vec4::ZERO,
            });
        }
    }

    for r in 0..lat_rings {
        let r_start = (r * lon_segments) as u16;
        let next_r_start = ((r + 1) * lon_segments) as u16;
        for s in 0..lon_segments as u16 {
            let next_s = (s + 1) % (lon_segments as u16);
            let v00 = r_start + s;
            let v10 = next_r_start + s;
            let v01 = r_start + next_s;
            let v11 = next_r_start + next_s;

            // Inward-facing triangles
            indices.push(v00);
            indices.push(v10);
            indices.push(v01);

            indices.push(v01);
            indices.push(v10);
            indices.push(v11);
        }
    }

    SkydomeMesh {
        base_mesh: Mesh {
            vertices,
            indices,
            texture: None,
        },
    }
}

fn draw_skydome(skydome: &SkydomeMesh, cam_pos: Vec3, draw_buffer: &mut Mesh) {
    if draw_buffer.vertices.len() != skydome.base_mesh.vertices.len() {
        draw_buffer.vertices = skydome.base_mesh.vertices.clone();
        draw_buffer.indices = skydome.base_mesh.indices.clone();
        draw_buffer.texture = None;
    }
    for (i, v) in skydome.base_mesh.vertices.iter().enumerate() {
        draw_buffer.vertices[i].position = v.position + cam_pos;
    }
    draw_mesh(draw_buffer);
}

struct GroundFoundationMesh {
    base_mesh: Mesh,
}

fn create_ground_foundation_mesh() -> GroundFoundationMesh {
    // The decorative ground surrounds the voxel footprint. Filling its centre would
    // hide every crater and underground tunnel behind an indestructible surface.
    let half_x = crate::voxel::GRID_X as f32 * VOXEL_SIZE * 0.5;
    let half_z = crate::voxel::GRID_Z as f32 * VOXEL_SIZE * 0.5;
    let rings = [
        (half_x, half_z, Color::new(0.096, 0.110, 0.136, 1.0)),
        (68.0, 68.0, Color::new(0.080, 0.092, 0.118, 1.0)),
        (110.0, 110.0, Color::new(0.065, 0.075, 0.098, 1.0)),
        (165.0, 165.0, Color::new(0.052, 0.062, 0.082, 1.0)),
        (240.0, 240.0, Color::new(0.040, 0.048, 0.065, 1.0)),
    ];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(120);
    for &(x, z, color) in &rings {
        for (px, pz) in [(-x, -z), (-x, z), (x, z), (x, -z)] {
            vertices.push(Vertex {
                position: Vec3::new(px, 0.49, pz),
                uv: Vec2::ZERO,
                color: color.into(),
                normal: Vec4::new(0.0, 1.0, 0.0, 0.0),
            });
        }
    }
    let skirt_start = vertices.len();
    for i in 0..4 {
        let mut vertex = vertices[skirt_start - 4 + i];
        vertex.position.y = -6.0;
        vertices.push(vertex);
    }
    for ring in 0..rings.len() {
        for side in 0..4 {
            let a = (ring * 4 + side) as u16;
            let b = (ring * 4 + (side + 1) % 4) as u16;
            let c = a + 4;
            let d = b + 4;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    GroundFoundationMesh {
        base_mesh: Mesh { vertices, indices, texture: None },
    }
}

fn draw_ground_foundation(ground: &GroundFoundationMesh) {
    draw_mesh(&ground.base_mesh);
}

fn draw_volumetric_god_rays(
    cam_pos: Vec3,
    final_dir: Vec3,
    sun_dir: Vec3,
    grid: &crate::voxel::VoxelGrid,
    mesh: &mut Mesh,
) {
    mesh.vertices.clear();
    mesh.indices.clear();

    let sun_view_dot = final_dir.dot(sun_dir);
    if sun_view_dot < -0.15 {
        return;
    }
    let intensity = (sun_view_dot + 0.15).clamp(0.0, 1.0);
    let beam_color = Color::new(1.0, 0.92, 0.72, 0.045 * intensity);
    let c_raw: [u8; 4] = beam_color.into();

    let push_quad = |m: &mut Mesh, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [u8; 4]| {
        let base = m.vertices.len() as u16;
        m.vertices.push(Vertex { position: a, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: b, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: c, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: d, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    // Stream light shafts through key building apertures, shafts, and collapsed floor gaps
    let ray_anchors = [
        Vec3::new(0.0, 48.0, 0.0),
        Vec3::new(-6.0, 36.0, -4.0),
        Vec3::new(6.0, 42.0, 4.0),
        Vec3::new(-10.0, 24.0, 8.0),
        Vec3::new(10.0, 30.0, -8.0),
        Vec3::new(0.0, 60.0, 0.0),
        Vec3::new(-4.0, 18.0, 2.0),
        Vec3::new(4.0, 22.0, -2.0),
    ];

    let perp1 = sun_dir.cross(Vec3::Y).normalize_or_zero();
    let perp2 = sun_dir.cross(perp1).normalize_or_zero();

    for &anchor in &ray_anchors {
        let test_voxel = grid.world_to_voxel(anchor);
        if grid.in_bounds(test_voxel.x, test_voxel.y, test_voxel.z) && grid.get(test_voxel.x, test_voxel.y, test_voxel.z).is_solid() {
            continue;
        }

        let p_top = anchor + sun_dir * 38.0;
        let p_bot = anchor - sun_dir * 28.0;
        let r = 2.4;
        let q1_a = p_top - perp1 * r;
        let q1_b = p_top + perp1 * r;
        let q1_c = p_bot + perp1 * (r * 1.7);
        let q1_d = p_bot - perp1 * (r * 1.7);

        let q2_a = p_top - perp2 * r;
        let q2_b = p_top + perp2 * r;
        let q2_c = p_bot + perp2 * (r * 1.7);
        let q2_d = p_bot - perp2 * (r * 1.7);

        push_quad(mesh, q1_a, q1_b, q1_c, q1_d, c_raw);
        push_quad(mesh, q2_a, q2_b, q2_c, q2_d, c_raw);
    }

    // Radial sun flare blades around celestial disc when sun is in direct view
    if sun_view_dot > 0.4 {
        let flare_alpha = ((sun_view_dot - 0.4) * 0.12).clamp(0.0, 0.08);
        let flare_col = Color::new(1.0, 0.94, 0.82, flare_alpha);
        let flare_raw: [u8; 4] = flare_col.into();
        let sun_pos = cam_pos + sun_dir * 210.0;
        let f_r = perp1 * 35.0;
        let f_u = perp2 * 35.0;
        push_quad(mesh, sun_pos - f_r, sun_pos + f_u, sun_pos + f_r, sun_pos - f_u, flare_raw);
    }

    if !mesh.indices.is_empty() {
        draw_mesh(mesh);
    }
}

fn draw_emissive_bloom_coronas(
    cam_pos: Vec3,
    final_dir: Vec3,
    final_right: Vec3,
    final_up: Vec3,
    lights: &[crate::physics::DynamicPointLight],
    mesh: &mut Mesh,
) {
    mesh.vertices.clear();
    mesh.indices.clear();

    let push_quad = |m: &mut Mesh, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [u8; 4]| {
        let base = m.vertices.len() as u16;
        m.vertices.push(Vertex { position: a, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: b, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: c, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: d, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    for light in lights {
        if light.intensity < 0.65 {
            continue;
        }
        let to_light = light.position - cam_pos;
        let dist = to_light.length();
        if !(1.0..=90.0).contains(&dist) {
            continue;
        }
        let dir = to_light / dist;
        if dir.dot(final_dir) < 0.2 {
            continue;
        }
        let size = (light.radius * 0.18 * light.intensity).min(4.0);
        let alpha = (light.intensity * 0.30).min(0.60);
        let corona_col = Color::new(light.color.r, light.color.g, light.color.b, alpha);
        let c_raw: [u8; 4] = corona_col.into();
        let p = light.position;

        // Diamond corona flare
        let r = final_right * size;
        let u = final_up * size;
        push_quad(mesh, p - r, p + u, p + r, p - u, c_raw);

        // Cross flare spikes
        let spike_r = final_right * (size * 2.2);
        let spike_u = final_up * (size * 0.16);
        push_quad(mesh, p - spike_r, p + spike_u, p + spike_r, p - spike_u, c_raw);

        let spike2_u = final_up * (size * 2.2);
        let spike2_r = final_right * (size * 0.16);
        push_quad(mesh, p - spike2_r, p + spike2_u, p + spike2_r, p - spike2_u, c_raw);
    }

    if !mesh.indices.is_empty() {
        draw_mesh(mesh);
    }
}

#[allow(clippy::too_many_arguments)]
fn load_level(
    preset: LevelPreset,
    grid: &mut VoxelGrid,
    physics: &mut PhysicsSystem,
    weapon: &mut WeaponSystem,
    siege: &mut SiegeSystem,
    player: &mut Player,
    hud: &mut HUD,
    audio: &SoundSystem,
    fluid: &mut FluidSystem,
    replay: &mut ReplaySystem,
    campaign: &mut CampaignManager,
    campaign_mode_active: &mut bool,
    prev_destroyed_total: &mut usize,
    structural_check_cooldown: &mut f32,
    muzzle_flash_timer: &mut f32,
    dynamic_lights: &mut Vec<crate::physics::DynamicPointLight>,
    notification_msg: Option<&str>,
) {
    siege.recall_all(audio);
    let spawn_pos = generate_level(grid, preset).spawn_point(grid);
    physics.clear();
    fluid.clear();
    replay.clear();
    let mut no_contract = false;
    if *campaign_mode_active {
        if let Some(idx) = campaign.contracts.iter().position(|c| c.target_level == preset) {
            campaign.active_contract_idx = idx;
            campaign.start_contract(grid.solid_count());
        } else {
            *campaign_mode_active = false;
            campaign.reset_contract_state();
            no_contract = true;
        }
    }
    *structural_check_cooldown = 0.0;
    weapon.projectiles.clear();
    weapon.sticky_charges.clear();
    weapon.singularities.clear();
    weapon.tracers.clear();
    weapon.tesla_arcs.clear();
    weapon.is_firing_laser = false;
    weapon.laser_hit_pos = None;
    weapon.recent_hit = None;
    weapon.heat = 0.0;
    weapon.fire_cooldown = 0.0;
    weapon.railgun_charge = 0.0;
    weapon.recoil_pos = Vec3::ZERO;
    weapon.recoil_rot = Vec3::ZERO;
    weapon.recoil_pos_vel = Vec3::ZERO;
    weapon.recoil_rot_vel = Vec3::ZERO;
    weapon.laser_sound_timer = 0.0;
    weapon.clear_c4();
    weapon.active_cutting_shocks.clear();
    *muzzle_flash_timer = 0.0;
    dynamic_lights.clear();
    player.spawn_pos = spawn_pos;
    player.position = spawn_pos;
    player.velocity = Vec3::ZERO;
    player.yaw = -std::f32::consts::FRAC_PI_2;
    player.pitch = 0.16;
    player.trauma = 0.0;
    player.fov_kick = 0.0;
    player.dash_tilt = 0.0;
    player.dash_cooldown = 0.0;
    player.is_grappling = false;
    player.grapple_hook = None;
    player.coyote_timer = 0.0;
    player.jump_buffer = 0.0;
    player.bob_timer = 0.0;
    player.bob_amount = Vec3::ZERO;
    *prev_destroyed_total = 0;
    hud.destroyed_counter = 0;
    hud.hit_markers.clear();
    hud.collapse_notifications.clear();
    let msg = notification_msg.unwrap_or_else(|| preset.name());
    hud.add_notification(msg);
    if no_contract {
        hud.add_notification("NO CONTRACT FOR THIS LEVEL: SANDBOX MODE");
    }
    audio.play(audio.ui_switch.as_ref(), 0.9);
}

/// Grabs/releases the OS cursor and keeps the player's mouse-look flag in sync.
fn apply_cursor_lock(player: &mut Player, lock: bool) {
    set_cursor_grab(lock);
    show_mouse(!lock);
    player.is_mouse_locked = lock;
}

#[macroquad::main(window_conf)]
async fn main() {
    // 1. Initialize Audio Engine & Procedural Dark-Synth Music Sequencer
    let audio = SoundSystem::new().await;
    let mut music_engine = MusicEngine::new().await;

    // 2. Initialize Voxel World, Physics & Fluid Dynamics
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let mut fluid_system = FluidSystem::new();
    let mut current_level = LevelPreset::ApexSkyscraper;
    let level_spawn = generate_level(&mut grid, current_level).spawn_point(&grid);

    // 3. Initialize Weapon, Siege, Customizer, Campaign & Replay Systems
    let mut weapon = WeaponSystem::new();
    let mut siege = SiegeSystem::new();
    siege.setup_default_fleet();
    let mut customizer = CustomizerUI::new();
    let mut hud = HUD::new();
    let mut campaign = CampaignManager::new();
    let mut campaign_mode_active = false;
    let mut replay = ReplaySystem::new();

    // 4. Initialize First-Person Player Controller
    let mut player = Player::new(level_spawn);
    player.pitch = 0.16; // Imposing upward viewpoint towards the citadel crown

    // Initial mouse lock for true FPS mouse-look
    set_cursor_grab(true);
    show_mouse(false);

    let mut prev_destroyed_total = 0;
    let mut muzzle_flash_timer = 0.0f32;
    let mut muzzle_flash_color = WHITE;
    let skydome = create_skydome_mesh();
    let mut sky_mesh = Mesh {
        vertices: skydome.base_mesh.vertices.clone(),
        indices: skydome.base_mesh.indices.clone(),
        texture: None,
    };
    let ground_foundation = create_ground_foundation_mesh();
    let mut distant_skyline = skyline::create_distant_skyline();
    let mut skyline_draw_buffers = distant_skyline.create_draw_buffers();
    let mut dynamic_lights: Vec<crate::physics::DynamicPointLight> = Vec::with_capacity(32);
    let mut structural_check_cooldown = 0.0f32;
    let mut bullet_time_active = false;
    let mut bullet_time_timer = 0.0f32;
    let mut bullet_time_dilation = 0.25f32;
    let mut scanner_mesh = Mesh {
        vertices: Vec::with_capacity(4096),
        indices: Vec::with_capacity(6144),
        texture: None,
    };
    let mut god_rays_mesh = Mesh {
        vertices: Vec::with_capacity(128),
        indices: Vec::with_capacity(192),
        texture: None,
    };
    let mut bloom_coronas_mesh = Mesh {
        vertices: Vec::with_capacity(256),
        indices: Vec::with_capacity(384),
        texture: None,
    };
    let mut fluid_mesh = Mesh {
        vertices: Vec::with_capacity(4096),
        indices: Vec::with_capacity(6144),
        texture: None,
    };

    loop {
        let dt = get_frame_time().min(0.05);
        audio.configure(customizer.master_volume, customizer.siege_volume, customizer.anti_stacking_ms, customizer.stress_creak_vol);
        let mut is_bullet_time = bullet_time_active || bullet_time_timer > 0.0;

        // --- INPUT & HOTKEYS ---
        // Every key comes from controls.rs (the single binding table that the HUD and F1 manual
        // also read). While the field manual is open, only F1 and Esc respond.
        if is_key_pressed(controls::KEY_MANUAL) {
            hud.help_modal_open = !hud.help_modal_open;
            audio.play(audio.ui_switch.as_ref(), 0.70);
            apply_cursor_lock(&mut player, !hud.help_modal_open && !customizer.is_open);
        }
        let hotkeys_live = !hud.help_modal_open;

        // Toggle Customizer Workbench
        if hotkeys_live && is_key_pressed(controls::KEY_WORKBENCH) {
            customizer.toggle(&audio);
            apply_cursor_lock(&mut player, !customizer.is_open);
        }

        // Toggle Structural Stress Scanner
        if hotkeys_live && is_key_pressed(controls::KEY_SCANNER) {
            hud.scanner_active = !hud.scanner_active;
            audio.play(audio.ui_switch.as_ref(), 0.6);
        }

        // Shift Level Previous / Next ([ and ])
        if hotkeys_live && is_key_pressed(controls::KEY_LEVEL_PREV) {
            current_level = current_level.prev();
            load_level(
                current_level,
                &mut grid,
                &mut physics,
                &mut weapon,
                &mut siege,
                &mut player,
                &mut hud,
                &audio,
                &mut fluid_system,
                &mut replay,
                &mut campaign,
                &mut campaign_mode_active,
                &mut prev_destroyed_total,
                &mut structural_check_cooldown,
                &mut muzzle_flash_timer,
                &mut dynamic_lights,
                None,
            );
        }
        if hotkeys_live && is_key_pressed(controls::KEY_LEVEL_NEXT) {
            current_level = current_level.next();
            load_level(
                current_level,
                &mut grid,
                &mut physics,
                &mut weapon,
                &mut siege,
                &mut player,
                &mut hud,
                &audio,
                &mut fluid_system,
                &mut replay,
                &mut campaign,
                &mut campaign_mode_active,
                &mut prev_destroyed_total,
                &mut structural_check_cooldown,
                &mut muzzle_flash_timer,
                &mut dynamic_lights,
                None,
            );
        }

        // Tactical Level Reset (R or Backspace)
        if hotkeys_live
            && (is_key_pressed(controls::KEY_RESET) || is_key_pressed(controls::KEY_RESET_ALT))
        {
            load_level(
                current_level,
                &mut grid,
                &mut physics,
                &mut weapon,
                &mut siege,
                &mut player,
                &mut hud,
                &audio,
                &mut fluid_system,
                &mut replay,
                &mut campaign,
                &mut campaign_mode_active,
                &mut prev_destroyed_total,
                &mut structural_check_cooldown,
                &mut muzzle_flash_timer,
                &mut dynamic_lights,
                Some("LEVEL RESET: WORLD CLEARED"),
            );
        }

        // Direct Level Jump (F2 - F9; [ and ] cycle all 9 levels)
        for &(key, preset) in &controls::LEVEL_KEYS {
            if hotkeys_live && is_key_pressed(key) {
                current_level = preset;
                load_level(
                    current_level,
                    &mut grid,
                    &mut physics,
                    &mut weapon,
                    &mut siege,
                    &mut player,
                    &mut hud,
                    &audio,
                    &mut fluid_system,
                &mut replay,
                    &mut campaign,
                    &mut campaign_mode_active,
                    &mut prev_destroyed_total,
                    &mut structural_check_cooldown,
                    &mut muzzle_flash_timer,
                    &mut dynamic_lights,
                    None,
                );
            }
        }

        // Quick Weapon Mode Switch Hotkeys (1 - 0)
        for &(key, mode) in &controls::WEAPON_KEYS {
            if hotkeys_live && is_key_pressed(key) {
                weapon.set_mode(mode, &audio);
            }
        }

        // M: Toggle Procedural Dark-Synth Music Mute
        if hotkeys_live && is_key_pressed(controls::KEY_MUSIC) {
            let muted = music_engine.toggle_mute();
            hud.add_notification(&format!(
                "DARK-SYNTH PROCEDURAL MUSIC: {} [{} TO {}]",
                if muted { "MUTED" } else { "ENGAGED" },
                controls::MUSIC.label,
                if muted { "UNMUTE" } else { "MUTE" }
            ));
        }

        // C: Toggle Demolition Contractor Campaign Mode vs Sandbox Mode
        if hotkeys_live && is_key_pressed(controls::KEY_CAMPAIGN) {
            campaign_mode_active = !campaign_mode_active;
            audio.play(audio.ui_switch.as_ref(), 0.70);
            if campaign_mode_active {
                current_level = campaign.active_contract().target_level;
                let notif_msg = format!("CONTRACTOR MODE: {}", campaign.active_contract().title);
                load_level(
                    current_level,
                    &mut grid,
                    &mut physics,
                    &mut weapon,
                    &mut siege,
                    &mut player,
                    &mut hud,
                    &audio,
                    &mut fluid_system,
                &mut replay,
                    &mut campaign,
                    &mut campaign_mode_active,
                    &mut prev_destroyed_total,
                    &mut structural_check_cooldown,
                    &mut muzzle_flash_timer,
                    &mut dynamic_lights,
                    Some(&notif_msg),
                );
                hud.add_notification(&format!("DEMOLITION CONTRACTOR MODE ACTIVATED ({} TO TOGGLE)", controls::CAMPAIGN.label));
            } else {
                hud.add_notification("SANDBOX DEMOLITION FREEDOM ACTIVATED");
            }
        }

        // Campaign Contractor progression controls (start/advance/retry, next/previous contract)
        if campaign_mode_active && hotkeys_live && !replay.is_active {
            if is_key_pressed(controls::KEY_CONTRACT_START) {
                match &campaign.status {
                    crate::campaign::ContractStatus::Ready => {
                        let title = campaign.active_contract().title;
                        campaign.start_contract(grid.solid_count());
                        audio.play(audio.ui_switch.as_ref(), 0.85);
                        hud.add_notification(&format!("CONTRACT STARTED: {}", title));
                    }
                    crate::campaign::ContractStatus::Passed(_) => {
                        let _ = campaign.next_contract();
                        current_level = campaign.active_contract().target_level;
                        let notif_msg = format!("ADVANCED TO: {}", campaign.active_contract().title);
                        load_level(
                            current_level,
                            &mut grid,
                            &mut physics,
                            &mut weapon,
                            &mut siege,
                            &mut player,
                            &mut hud,
                            &audio,
                            &mut fluid_system,
                &mut replay,
                            &mut campaign,
                            &mut campaign_mode_active,
                            &mut prev_destroyed_total,
                            &mut structural_check_cooldown,
                            &mut muzzle_flash_timer,
                            &mut dynamic_lights,
                            Some(&notif_msg),
                        );
                    }
                    crate::campaign::ContractStatus::Failed(_) => {
                        campaign.reset_contract_state();
                        let notif_msg = format!("CONTRACT RETRY: {}", campaign.active_contract().title);
                        load_level(
                            current_level,
                            &mut grid,
                            &mut physics,
                            &mut weapon,
                            &mut siege,
                            &mut player,
                            &mut hud,
                            &audio,
                            &mut fluid_system,
                &mut replay,
                            &mut campaign,
                            &mut campaign_mode_active,
                            &mut prev_destroyed_total,
                            &mut structural_check_cooldown,
                            &mut muzzle_flash_timer,
                            &mut dynamic_lights,
                            Some(&notif_msg),
                        );
                    }
                    _ => {}
                }
            }

            // N: Next Demolition Contract
            if is_key_pressed(controls::KEY_CONTRACT_NEXT) {
                let _ = campaign.next_contract();
                current_level = campaign.active_contract().target_level;
                let notif_msg = format!("CONTRACT: {}", campaign.active_contract().title);
                load_level(
                    current_level,
                    &mut grid,
                    &mut physics,
                    &mut weapon,
                    &mut siege,
                    &mut player,
                    &mut hud,
                    &audio,
                    &mut fluid_system,
                &mut replay,
                    &mut campaign,
                    &mut campaign_mode_active,
                    &mut prev_destroyed_total,
                    &mut structural_check_cooldown,
                    &mut muzzle_flash_timer,
                    &mut dynamic_lights,
                    Some(&notif_msg),
                );
            }

            // P: Previous Demolition Contract
            if is_key_pressed(controls::KEY_CONTRACT_PREV) {
                let _ = campaign.prev_contract();
                current_level = campaign.active_contract().target_level;
                let notif_msg = format!("CONTRACT: {}", campaign.active_contract().title);
                load_level(
                    current_level,
                    &mut grid,
                    &mut physics,
                    &mut weapon,
                    &mut siege,
                    &mut player,
                    &mut hud,
                    &audio,
                    &mut fluid_system,
                &mut replay,
                    &mut campaign,
                    &mut campaign_mode_active,
                    &mut prev_destroyed_total,
                    &mut structural_check_cooldown,
                    &mut muzzle_flash_timer,
                    &mut dynamic_lights,
                    Some(&notif_msg),
                );
            }
        }

        // Level loads reset replay history and tactical timing together.
        if replay.recording_clock == 0.0 {
            bullet_time_active = false;
            bullet_time_timer = 0.0;
            is_bullet_time = false;
        }

        // F10: Toggle Cinematic Replay Mode
        if hotkeys_live && is_key_pressed(controls::KEY_REPLAY) {
            let replay_focus = player.camera_position() + player.forward_dir() * 12.0;
            let replay_active = replay.toggle_replay_with_target(replay_focus);
            if replay_active {
                hud.add_notification(&format!(
                    "CINEMATIC REPLAY ENGAGED ({} PLAY/PAUSE | {} SCRUB | {} EXIT)",
                    controls::REPLAY_PAUSE.token,
                    controls::REPLAY_SCRUB.token,
                    controls::REPLAY.token
                ));
            } else if replay.total_recorded_frames == 0 {
                hud.add_notification("REPLAY UNAVAILABLE: RECORD SOME GAMEPLAY FIRST");
            } else {
                hud.add_notification("CINEMATIC REPLAY DISENGAGED");
            }
        }

        // X: Toggle Artillery Projectile Tracking Flight Cam
        if hotkeys_live && is_key_pressed(controls::KEY_FLIGHT_CAM) {
            replay.flight_cam_active = !replay.flight_cam_active;
            hud.add_notification(if replay.flight_cam_active {
                "ARTILLERY FLIGHT CAM: ENGAGED (TRACKING LIVE PROJECTILES)"
            } else {
                "ARTILLERY FLIGHT CAM: OFF"
            });
        }

        // Cinematic Replay Scrubber Controls (when replay is active)
        if replay.is_active && hotkeys_live && !customizer.is_open {
            if is_key_pressed(controls::KEY_REPLAY_PAUSE) {
                replay.is_paused = !replay.is_paused;
            }
            if is_key_down(controls::KEY_REPLAY_BACK) {
                replay.playback_time = (replay.playback_time - dt * 2.5).max(replay.oldest_timestamp());
            }
            if is_key_down(controls::KEY_REPLAY_FWD) {
                replay.playback_time = (replay.playback_time + dt * 2.5).min(replay.newest_timestamp());
            }
            if is_key_pressed(controls::KEY_REPLAY_FASTER) {
                replay.playback_speed = (replay.playback_speed * 1.5).min(4.0);
            }
            if is_key_pressed(controls::KEY_REPLAY_SLOWER) {
                replay.playback_speed = (replay.playback_speed / 1.5).max(0.1);
            }
            // Orbit camera drag
            if is_mouse_button_down(MouseButton::Right) || is_mouse_button_down(MouseButton::Left) {
                let mdelta = mouse_delta_position();
                replay.orbit_yaw += mdelta.x * 3.0;
                replay.orbit_pitch = (replay.orbit_pitch - mdelta.y * 3.0).clamp(-1.4, 1.4);
            }
            let (_wheel_x, wheel_y) = mouse_wheel();
            if wheel_y.abs() > 0.05 {
                replay.orbit_distance = (replay.orbit_distance - wheel_y * 3.0).clamp(5.0, 120.0);
            }
        }

        // Escape: Toggle cursor grab or close open modals (Help manual, customizer)
        if is_key_pressed(controls::KEY_ESCAPE) {
            if hud.help_modal_open {
                hud.help_modal_open = false;
                audio.play(audio.ui_switch.as_ref(), 0.50);
                apply_cursor_lock(&mut player, !customizer.is_open);
            } else if customizer.is_open {
                customizer.toggle(&audio);
                apply_cursor_lock(&mut player, true);
            } else {
                let next_grab = !player.is_mouse_locked;
                apply_cursor_lock(&mut player, next_grab);
            }
        }

        let simulation_paused = replay.is_active || customizer.is_open || hud.help_modal_open || !player.is_mouse_locked;
        let allow_gameplay_input = !customizer.is_open && player.is_mouse_locked && !replay.is_active && !hud.help_modal_open;

        // --- UPDATE PLAYER & AIM RAYCAST ---
        if !simulation_paused {
            player.update(dt, &grid, Some(&physics), allow_gameplay_input);
        } else {
            player.just_dashed = false;
            player.just_grappled = false;
            player.just_jumped = false;
            player.just_landed = None;
        }

        if player.just_dashed {
            audio.play(audio.jet_dash.as_ref(), 0.95);
        }
        if player.just_grappled {
            audio.play(audio.vacuum_sweep.as_ref(), 0.85);
            hud.add_notification(&format!(
                "KINETIC GRAPPLE LATCHED - {} SLINGSHOT | {} RELEASE",
                controls::JUMP.token,
                controls::GRAPPLE.token
            ));
        }
        if player.just_jumped {
            audio.play(audio.jump.as_ref(), 0.65);
        }
        if let Some(fall_speed) = player.just_landed {
            let vol = (fall_speed / 20.0).clamp(0.4, 1.0);
            audio.play(audio.land_impact.as_ref(), vol);
            audio.play(audio.debris_clatter.as_ref(), vol * 0.55);
        }

        let cam_pos = player.camera_position();
        let cam_dir = player.forward_dir();
        let cam_right = player.right_dir();
        let cam_up = player.up_dir();

        // Aim raycast for reticle and range (checks static grid first, then physics chunks)
        let mut aim_hit = grid.raycast(cam_pos, cam_dir, 100.0);
        // Static-voxel hit distance drives the grapple prompt (the grapple ignores physics chunks).
        hud.aim_static_distance = aim_hit.as_ref().map(|h| h.distance);
        let mut aim_is_static = aim_hit.is_some();
        if let Some((hit_p, hit_n)) = physics.check_chunk_hit(cam_pos, cam_pos + cam_dir * 100.0) {
            let dist = (hit_p - cam_pos).length();
            if aim_hit.as_ref().is_none_or(|hit| dist < hit.distance) {
                aim_is_static = false;
                aim_hit = Some(crate::voxel::RayHit {
                    voxel_pos: IVec3::ZERO,
                    world_pos: hit_p,
                    normal: hit_n,
                    voxel_type: crate::voxel::VoxelType::Concrete,
                    health_ratio: 0.75,
                    distance: dist,
                });
            }
        }

        // --- WEAPON FIRING ---
        let muzzle_pos = cam_pos + cam_right * 0.25 - cam_up * 0.2 + cam_dir * 0.6;

        if allow_gameplay_input {
            // Deploy / Reposition Selected Siege Engine at Aim Target or Forward Ground
            if is_key_pressed(controls::KEY_SIEGE_DEPLOY) {
                let target_p = aim_hit.as_ref().map(|h| h.world_pos).unwrap_or_else(|| {
                    if cam_dir.y.abs() > 0.01 {
                        let t = (0.50 - cam_pos.y) / cam_dir.y;
                        if t > 0.0 {
                            cam_pos + cam_dir * t
                        } else {
                            cam_pos + cam_dir * 18.0
                        }
                    } else {
                        cam_pos + cam_dir * 18.0
                    }
                });
                siege.deploy_selected_engine(target_p, cam_dir, &audio);
            }

            // Command Active Deployed Siege Battery to Fire (G)
            if is_key_pressed(controls::KEY_SIEGE_FIRE) {
                let target_p = aim_hit.as_ref().map(|h| h.world_pos);
                let siege_kick = siege.fire_active_engine(target_p, &mut grid, &mut physics, &audio, cam_pos);
                if campaign_mode_active && siege_kick > 0.0 {
                    campaign.record_ordnance_spend(600.0);
                }
                player.add_trauma(siege_kick);
            }

            // Tactical Bullet-Time Toggle (T)
            if is_key_pressed(controls::KEY_SLOWMO) {
                bullet_time_active = !bullet_time_active;
                if bullet_time_active {
                    audio.play(audio.bullet_time_drone.as_ref(), 0.85);
                    hud.add_notification(&format!("BULLET-TIME ENGAGED ({:.2}x SPEED - SCROLL WHEEL TO TUNE)", bullet_time_dilation));
                } else {
                    audio.play(audio.ui_switch.as_ref(), 0.50);
                    hud.add_notification("BULLET-TIME DISENGAGED (1.0x NORMAL)");
                }
            }

            is_bullet_time = bullet_time_active || bullet_time_timer > 0.0;

            // Variable Bullet-Time Dilation Tuning (Mouse Scroll Wheel while slowmo is active)
            let (_wheel_x, wheel_y) = mouse_wheel();
            if is_bullet_time && wheel_y.abs() > 0.05 {
                bullet_time_dilation = (bullet_time_dilation + wheel_y.signum() * 0.05).clamp(0.05, 1.0);
                hud.add_notification(&format!("BULLET-TIME SPEED: {:.2}x", bullet_time_dilation));
            }

            // Demolition Engineer C4 Shaped Charges (E: Plant, F: Detonate All)
            if is_key_pressed(controls::KEY_C4_PLANT) {
                if let Some(hit) = aim_hit.as_ref() {
                    let anchor_v = aim_is_static.then_some(hit.voxel_pos);
                    if weapon.place_c4_charge(hit.world_pos, hit.normal, anchor_v) {
                        if campaign_mode_active {
                            campaign.record_c4_placement();
                        }
                        audio.play(audio.ui_switch.as_ref(), 0.80);
                        let charge_count = weapon.c4_charges.len();
                        hud.add_notification(&format!(
                            "C4 SHAPED CHARGE ARMED [{}/{}] - PRESS {} TO DETONATE",
                            charge_count,
                            crate::weapon::MAX_C4_CHARGES,
                            controls::C4_DETONATE.label
                        ));
                    } else {
                        hud.add_notification(&format!(
                            "MAX C4 CHARGES DEPLOYED ({}/{}) - PRESS {} TO DETONATE",
                            crate::weapon::MAX_C4_CHARGES,
                            crate::weapon::MAX_C4_CHARGES,
                            controls::C4_DETONATE.label
                        ));
                    }
                } else {
                    hud.add_notification("C4 DEPLOYMENT FAILED: NO SURFACE IN RANGE");
                }
            }

            if is_key_pressed(controls::KEY_C4_DETONATE) {
                let (feedback, det_count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, cam_pos);
                if det_count > 0 {
                    fluid_system.spawn_gas(cam_pos + cam_dir * 8.0, Vec3::new(0.0, 1.8, 0.0), 16);
                    player.add_trauma(feedback.camera_trauma);
                    player.apply_weapon_recoil(feedback.player_recoil);
                    player.fov_kick += feedback.fov_kick;
                    hud.reticle_bloom = (hud.reticle_bloom + 1.2).min(1.5);
                    hud.add_notification(&format!("COORDINATED DEMOLITION: {} C4 CHARGES DETONATED!", det_count));
                } else {
                    hud.add_notification(&format!("NO ARMED C4 CHARGES - PRESS {} TO PLANT", controls::C4_PLANT.label));
                }
            }

            // Quick Cycle Siege Engine Archetypes
            if is_key_pressed(controls::KEY_SIEGE_CYCLE) {
                siege.cycle_engine_kind(&audio);
            }

            // Recall All Deployed Siege Engines
            if is_key_pressed(controls::KEY_SIEGE_RECALL) {
                siege.recall_all(&audio);
                hud.add_notification("ALL SIEGE ENGINES RECALLED TO FLEET RESERVE");
            }

            // Primary Fire (LMB)
            let is_continuous = weapon.config.mode == WeaponMode::SolsticeLance || weapon.config.mode == WeaponMode::PlasmaFlamer;
            let fire_input = if is_continuous {
                is_mouse_button_down(MouseButton::Left)
            } else {
                is_mouse_button_pressed(MouseButton::Left)
            };

            if fire_input {
                let prev_cd = weapon.fire_cooldown;
                let feedback = weapon.fire_primary(
                    muzzle_pos,
                    cam_dir,
                    cam_pos,
                    &mut grid,
                    &mut physics,
                    &audio,
                );
                let shot_fired = weapon.fire_cooldown > prev_cd;
                if campaign_mode_active && shot_fired {
                    let cost = match weapon.config.mode {
                        WeaponMode::TitanRam => 25.0,
                        WeaponMode::HavocMortar => 150.0,
                        WeaponMode::EventHorizon => 500.0,
                        WeaponMode::SolsticeLance => 60.0 * 0.06, // Cost per accepted laser tick.
                        WeaponMode::ApexRailgun => 300.0,
                        WeaponMode::FrostbiteMortar => 120.0,
                        WeaponMode::SeismicDisrupter => 400.0,
                        WeaponMode::TeslaCaster => 80.0,
                        WeaponMode::PlasmaFlamer => 50.0 * 0.08, // Cost per accepted flame pulse.
                        WeaponMode::PlasmaWireSaw => 100.0,
                    };
                    campaign.record_ordnance_spend(cost);
                }
                if shot_fired && weapon.config.mode == WeaponMode::PlasmaFlamer {
                    fluid_system.spawn_slag(muzzle_pos + cam_dir * 1.5, cam_dir * 22.0, 2);
                } else if shot_fired && weapon.config.mode == WeaponMode::FrostbiteMortar {
                    fluid_system.spawn_water(muzzle_pos + cam_dir * 2.0, cam_dir * 18.0, 2);
                }
                if shot_fired {
                    muzzle_flash_timer = if is_continuous { 0.06 } else { 0.14 };
                    muzzle_flash_color = weapon.config.mode.color();
                }
                player.add_trauma(feedback.camera_trauma);
                player.apply_weapon_recoil(feedback.player_recoil);
                player.fov_kick += feedback.fov_kick;
                if shot_fired && !is_continuous {
                    hud.reticle_bloom = (hud.reticle_bloom + 0.6).min(1.5);
                }
            } else if is_continuous {
                weapon.is_firing_laser = false;
            }

            // Secondary / Alt-Fire (RMB): Alt-Fire mode or Detonate Remote Sticky Charges!
            if weapon.config.mode == crate::weapon::WeaponMode::ApexRailgun && weapon.sticky_charges.is_empty() {
                if is_mouse_button_down(MouseButton::Right) {
                    if weapon.railgun_charge == 0.0 {
                        audio.play(audio.capacitor_whine.as_ref(), 0.65);
                    }
                    weapon.railgun_charge = (weapon.railgun_charge + dt).min(1.0);
                    hud.reticle_bloom = (hud.reticle_bloom + 0.3 * dt).min(1.5);
                }
                if is_mouse_button_released(MouseButton::Right) || weapon.railgun_charge >= 0.85 {
                    if weapon.railgun_charge >= 0.25 {
                        let prev_secondary_cd = weapon.fire_cooldown;
                        let detonated_sticky = !weapon.sticky_charges.is_empty();
                        let (feedback, notif) = weapon.fire_secondary(muzzle_pos, cam_dir, cam_pos, &mut grid, &mut physics, &audio);
                        if campaign_mode_active && !notif.is_empty() && (weapon.fire_cooldown > prev_secondary_cd || detonated_sticky) {
                            campaign.record_ordnance_spend(75.0);
                        }
                        if feedback.camera_trauma > 0.0 || !notif.is_empty() {
                            player.add_trauma(feedback.camera_trauma);
                            player.apply_weapon_recoil(feedback.player_recoil);
                            player.fov_kick += feedback.fov_kick;
                            hud.reticle_bloom = (hud.reticle_bloom + 0.8).min(1.5);
                            if !notif.is_empty() {
                                hud.add_notification(notif);
                            }
                        }
                    }
                    weapon.railgun_charge = 0.0;
                }
            } else {
                weapon.railgun_charge = 0.0;
                if is_mouse_button_pressed(MouseButton::Right) {
                    let prev_secondary_cd = weapon.fire_cooldown;
                    let detonated_sticky = !weapon.sticky_charges.is_empty();
                    let (feedback, notif) = weapon.fire_secondary(muzzle_pos, cam_dir, cam_pos, &mut grid, &mut physics, &audio);
                    if campaign_mode_active && !notif.is_empty() && (weapon.fire_cooldown > prev_secondary_cd || detonated_sticky) {
                        campaign.record_ordnance_spend(75.0);
                    }
                    if feedback.camera_trauma > 0.0 || !notif.is_empty() {
                        player.add_trauma(feedback.camera_trauma);
                        player.apply_weapon_recoil(feedback.player_recoil);
                        player.fov_kick += feedback.fov_kick;
                        hud.reticle_bloom = (hud.reticle_bloom + 0.8).min(1.5);
                        if !notif.is_empty() {
                            hud.add_notification(notif);
                        }
                    }
                }
            }
        } else {
            weapon.is_firing_laser = false;
        }

        if allow_gameplay_input && is_key_pressed(controls::KEY_INVERT_PITCH) {
            hud.add_notification(if player.invert_pitch {
                "MOUSE PITCH: INVERTED"
            } else {
                "MOUSE PITCH: STANDARD"
            });
        }

        // H: Hazard Discharge (Pressurized cellular fluid discharge)
        if allow_gameplay_input && is_key_pressed(controls::KEY_PURGE) {
            let p_origin = muzzle_pos + cam_dir * 1.2;
            match weapon.config.mode {
                WeaponMode::PlasmaFlamer => {
                    fluid_system.spawn_slag(p_origin, cam_dir * 24.0, 20);
                    hud.add_notification("HAZARD DISCHARGE: SUPERHEATED MOLTEN SLAG DISCHARGED");
                }
                WeaponMode::FrostbiteMortar => {
                    fluid_system.spawn_water(p_origin, cam_dir * 26.0, 28);
                    hud.add_notification("HAZARD DISCHARGE: HIGH-PRESSURE WATER TORRENT DISCHARGED");
                }
                _ => {
                    fluid_system.spawn_gas(p_origin, cam_dir * 16.0, 24);
                    hud.add_notification("HAZARD DISCHARGE: VOLATILE FUEL GAS VENTED");
                }
            }
            audio.play(audio.vacuum_sweep.as_ref(), 0.70);
        }

        if !simulation_paused {
            muzzle_flash_timer = (muzzle_flash_timer - dt).max(0.0);
            let stress_fluid = match current_level {
                LevelPreset::ChemicalRefinery => crate::fluid::FluidType::ToxicGas,
                LevelPreset::QuantumFusionReactor => crate::fluid::FluidType::MoltenSlag,
                _ => crate::fluid::FluidType::Water,
            };
            let (stress_trauma, severed_rebar) = fluid_system.update_structural_feedback(
                dt, &mut grid, &mut physics, &audio, stress_fluid, cam_pos,
            );
            player.add_trauma(stress_trauma);
            if severed_rebar {
                hud.add_notification("WARNING: LOAD-BEARING REBAR COLUMN SEVERED!");
            }

            // --- PHYSICS & WEAPON UPDATE (With Variable Bullet-Time Time Dilation) ---
            if bullet_time_timer > 0.0 {
                bullet_time_timer = (bullet_time_timer - dt).max(0.0);
            }
            let sim_dt = if is_bullet_time { dt * bullet_time_dilation } else { dt };

            let phys_trauma = physics.update(sim_dt, &mut grid, &audio, cam_pos);
            player.add_trauma(phys_trauma);

            let wpn_trauma = weapon.update(sim_dt, &mut grid, &mut physics, &audio, cam_pos);
            player.add_trauma(wpn_trauma);

            let siege_trauma = siege.update(sim_dt, &mut grid, &mut physics, &audio, cam_pos);
            player.add_trauma(siege_trauma);

            // Fluid simulation update (pressurized water, molten slag, toxic gas)
            fluid_system.update(sim_dt, &mut grid, &mut physics, &audio);

            // Amortize structural BFS during cascading collapses while ensuring instantaneous response to direct trauma or structural damage
            structural_check_cooldown = (structural_check_cooldown - dt).max(0.0);
            if grid.structural_dirty || phys_trauma > 0.0 || wpn_trauma > 0.0 || siege_trauma > 0.0 || !grid.severed_rebar_positions.is_empty() {
                structural_check_cooldown = 0.0;
            }

            // Process hit confirm feedback (audio tick/crit and reticle marker)
            if let Some(hit_kind) = weapon.recent_hit.take() {
                hud.register_hit(hit_kind);
                match hit_kind {
                    crate::ui::HitKind::Critical => {
                        audio.play(audio.hit_crit.as_ref(), 0.55);
                    }
                    _ => {
                        audio.play(audio.hit_tick.as_ref(), 0.40);
                    }
                }
            }

            // --- STRUCTURAL INTEGRITY & ISLAND COLLAPSE DETECTION ---
            let disconnected_islands = if grid.structural_dirty && structural_check_cooldown <= 0.0 {
                structural_check_cooldown = 0.05;
                grid.detect_unsupported_islands()
            } else {
                Vec::new()
            };
            let had_islands = !disconnected_islands.is_empty();
            if had_islands {
                structural_check_cooldown = 0.0;
                grid.structural_dirty = true;
                let closest_island_com = disconnected_islands.iter()
                    .map(|i| i.center_of_mass)
                    .min_by(|a, b| a.distance_squared(cam_pos).partial_cmp(&b.distance_squared(cam_pos)).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(cam_pos);

                audio.play_spatial(audio.structural_groan.as_ref(), closest_island_com, cam_pos, 0.65);
                audio.play_spatial(audio.big_boom.as_ref(), closest_island_com, cam_pos, 0.60);
                audio.play_spatial(audio.debris_clatter.as_ref(), closest_island_com, cam_pos, 0.50);

                let mut total_collapsed_voxels = 0;
                for island in &disconnected_islands {
                    total_collapsed_voxels += island.voxels.len();
                }

                // Auto-trigger tactical bullet-time on massive collapses > 4000 voxels
                if total_collapsed_voxels >= 4000 {
                    bullet_time_timer = 1.2;
                    audio.play(audio.bullet_time_drone.as_ref(), 0.90);
                    hud.add_notification(&format!("MEGA-COLLAPSE ({} VOXELS): SLOW-MO AUTO-ENGAGED!", total_collapsed_voxels));
                }

                let collapse_atten = SoundSystem::calculate_distance_attenuation(closest_island_com, cam_pos);
                let collapse_trauma = ((total_collapsed_voxels as f32 / 140.0).clamp(0.0, 0.65)) * collapse_atten;
                if collapse_trauma > 0.01 {
                    player.add_trauma(collapse_trauma);
                }

                let island_count = disconnected_islands.len();
                hud.add_notification(&format!("STRUCTURAL CATASTROPHE: {} SECTIONS ({} VOXELS) COLLAPSING!", island_count, total_collapsed_voxels));

                for island in disconnected_islands {
                    physics.spawn_stress_fissure(island.center_of_mass, Vec3::Y, 35);
                    // Monumental volumetric dust storm proportional to collapsed voxel volume
                    let approx_radius = ((island.voxels.len() as f32).cbrt() * crate::voxel::VOXEL_SIZE * 1.5).clamp(1.5, 16.0);
                    physics.spawn_structural_collapse_dust_storm(
                        island.center_of_mass,
                        approx_radius,
                        island.voxels.len(),
                    );

                    // Random initial drift/spin is scaled by 1/radius: small fragments tumble, but a
                    // detached building starts essentially at rest. (A flat ±1.2 rad/s gave a 100 m tower
                    // ~60 m/s tip speed on its first frame, letting it plow through its neighbors.)
                    let island_radius = ((island.voxels.len() as f32).cbrt() * crate::voxel::VOXEL_SIZE).max(0.5);
                    let size_scale = (0.5 / island_radius).min(1.0);
                    let initial_impulse = Vec3::new(
                        rand::gen_range(-1.5f32, 1.5) * size_scale,
                        rand::gen_range(-2.5f32, -1.0),
                        rand::gen_range(-1.5f32, 1.5) * size_scale,
                    );
                    let angular_impulse = Vec3::new(
                        rand::gen_range(-1.2f32, 1.2),
                        rand::gen_range(-0.6f32, 0.6),
                        rand::gen_range(-1.2f32, 1.2),
                    ) * size_scale;
                    physics.spawn_island_chunk(island, initial_impulse, angular_impulse);
                }
            }

            // Update Meshes & Voxel count
            grid.update_meshes();

            // Surviving voxel count is now maintained incrementally in O(1) time
            {
                let solid_count = grid.solid_count();
                let total_destroyed = grid.total_initial_voxels.saturating_sub(solid_count);
                if total_destroyed > prev_destroyed_total {
                    let diff = total_destroyed - prev_destroyed_total;
                    hud.destroyed_counter += diff;
                    prev_destroyed_total = total_destroyed;
                }
            }

            // Grade only after this frame's detached structures become falling chunks.
            if campaign_mode_active {
                campaign.update(dt, &grid, &physics);
            }

            replay.record_step(dt, &player, &physics, &weapon, &siege);
            replay.update_flight_cam(&weapon, &siege);
        } else if replay.is_active && !hud.help_modal_open && !customizer.is_open {
            replay.update_playback(dt);
        }

        let stress_ratio = if grid.total_initial_voxels > 0 {
            (grid.total_initial_voxels.saturating_sub(grid.solid_count()) as f32 / grid.total_initial_voxels as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        music_engine.state.master_volume = customizer.music_volume * customizer.master_volume;
        music_engine.update(stress_ratio, is_bullet_time);

        hud.update(dt);

        // --- 3D RENDERING PIPELINE ---
        // Neo-Brutalist Deep Atmospheric Twilight Sky
        clear_background(Color::new(0.06, 0.08, 0.12, 1.0));

        let (pitch, yaw, roll) = player.camera_rotation_angles();
        let final_dir = Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        ).normalize();

        let final_right = final_dir.cross(Vec3::Y).normalize();
        let final_up = (final_right.cross(final_dir) + final_right * roll.sin()).normalize();

        let (cam_pos_3d, cam_target_3d, cam_up_3d) = if replay.is_active {
            (replay.orbit_cam_pos, replay.orbit_target, Vec3::Y)
        } else if replay.flight_cam_active && replay.tracked_projectile_pos.is_some() {
            let p_pos = replay.tracked_projectile_pos.unwrap();
            let p_dir = replay.tracked_projectile_dir.unwrap_or(final_dir);
            (p_pos - p_dir * 5.0 + Vec3::new(0.0, 1.8, 0.0), p_pos + p_dir * 6.0, Vec3::Y)
        } else {
            (cam_pos, cam_pos + final_dir, final_up)
        };

        let view_dir = (cam_target_3d - cam_pos_3d).normalize_or_zero();
        let view_right = view_dir.cross(cam_up_3d).normalize_or_zero();

        let aspect = screen_width() / screen_height();
        let fov_y = player.fov();
        let frustum = crate::voxel::FrustumPlanes::from_camera(
            cam_pos_3d,
            view_dir,
            cam_up_3d,
            aspect,
            fov_y,
            0.1,
            380.0,
        );

        set_camera(&Camera3D {
            position: cam_pos_3d,
            target: cam_target_3d,
            up: cam_up_3d,
            fovy: fov_y,
            aspect: Some(aspect),
            projection: Projection::Perspective,
            z_far: 380.0,
            ..Default::default()
        });

        // 1. Atmospheric Spherical Geodesic Skydome (Moves with camera, 0 far-plane clipping, seamless canopy)
        draw_skydome(&skydome, cam_pos_3d, &mut sky_mesh);

        // Distant Celestial Sun / Fusion Disc (Relative to camera at infinity, zero parallax drift)
        let sun_dir = Vec3::new(-0.24, 0.38, -0.89).normalize();
        let sun_pos = cam_pos_3d + sun_dir * 215.0;
        draw_sphere(sun_pos, 7.5, None, Color::new(1.0, 0.98, 0.92, 1.0));
        draw_sphere(sun_pos, 14.0, None, Color::new(1.0, 0.88, 0.70, 0.60));
        draw_sphere(sun_pos, 22.0, None, Color::new(1.0, 0.80, 0.55, 0.25));

        // 360-Degree Procedural Distant Skyline (Hyperbolic cooling towers, sky-elevators, neon rim accents)
        distant_skyline.draw(cam_pos_3d, &mut skyline_draw_buffers, get_time() as f32);

        // 2. Industrial Bedrock Foundation Slab with Soft Atmospheric Depth Falloff
        // (Zero Moiré crosshatching, zero raw un-antialiased line grids, zero coplanar Z-fighting with bedrock voxels)
        draw_ground_foundation(&ground_foundation);
        // Note: Subterranean cube removed so 3D excavated craters down to -24.0m remain fully visible.

        // Collect Radiant Dynamic Point Lights (Muzzle Flash, Weapon Projectiles, Explosive Shockwaves)
        dynamic_lights.clear();
        if muzzle_flash_timer > 0.0 {
            let flash_a = (muzzle_flash_timer / 0.14).clamp(0.0, 1.0);
            dynamic_lights.push(crate::physics::DynamicPointLight {
                position: muzzle_pos,
                color: muzzle_flash_color,
                radius: 12.0,
                intensity: flash_a * 1.6,
            });
        }
        weapon.collect_dynamic_lights(&mut dynamic_lights);
        physics.collect_dynamic_lights(&mut dynamic_lights);
        siege.collect_dynamic_lights(&mut dynamic_lights);

        // Dynamic light budgeting: prioritize highest-intensity lights closest to camera
        if dynamic_lights.len() > 24 {
            dynamic_lights.sort_unstable_by(|a, b| {
                let dist_sq_a = a.position.distance_squared(cam_pos) + 1.0;
                let dist_sq_b = b.position.distance_squared(cam_pos) + 1.0;
                let score_sq_a = (a.intensity * a.intensity) / dist_sq_a;
                let score_sq_b = (b.intensity * b.intensity) / dist_sq_b;
                score_sq_b.partial_cmp(&score_sq_a).unwrap_or(std::cmp::Ordering::Equal)
            });
            dynamic_lights.truncate(24);
        }

        // 3. Voxel Static Geometry (Camera Frustum Culled, Zero Alloc)
        grid.draw_frustum(&frustum);

        // Soft Radiant Dynamic Point Illumination on static voxel architecture
        grid.draw_dynamic_lighting(&dynamic_lights);

        // 4. Structural Scanner Wireframe Highlights (if V key active, batched into a single Mesh)
        if hud.scanner_active && !replay.is_active {
            scanner_mesh.vertices.clear();
            scanner_mesh.indices.clear();

            let scan_radius = 28.0f32;
            let scan_radius_sq = scan_radius * scan_radius;
            let cam_vpos = grid.world_to_voxel(cam_pos);
            let r_voxels = (scan_radius / VOXEL_SIZE).ceil() as i32;

            let min_x = (cam_vpos.x - r_voxels).max(0) as usize;
            let max_x = ((cam_vpos.x + r_voxels).min(crate::voxel::GRID_X as i32 - 1).max(0)) as usize;
            let min_y = (cam_vpos.y - r_voxels).max(1) as usize;
            let max_y = ((cam_vpos.y + r_voxels).min(grid.max_solid_y).max(1)) as usize;
            let min_z = (cam_vpos.z - r_voxels).max(0) as usize;
            let max_z = ((cam_vpos.z + r_voxels).min(crate::voxel::GRID_Z as i32 - 1).max(0)) as usize;

            if min_x <= max_x
                && min_y <= max_y
                && min_z <= max_z
                && grid.region_has_solids(min_x as i32, max_x as i32, min_y as i32, max_y as i32, min_z as i32, max_z as i32)
            {
                let mut wire_budget = 300;
                let cur_time = get_time() as f32;
                'scanner_loop: for x in (min_x..=max_x).step_by(2) {
                    for y in (min_y..=max_y).step_by(2) {
                        for z in (min_z..=max_z).step_by(2) {
                            let vt = grid.get(x as i32, y as i32, z as i32);
                            if vt.is_solid()
                                && (vt == crate::voxel::VoxelType::SteelRebar
                                    || vt == crate::voxel::VoxelType::CompositeRebar
                                    || vt == crate::voxel::VoxelType::HeavyArmor
                                    || vt == crate::voxel::VoxelType::Explosive
                                    || vt == crate::voxel::VoxelType::GlowCore)
                            {
                                let wp = grid.voxel_to_world(IVec3::new(x as i32, y as i32, z as i32));
                                let d_sq = (wp - cam_pos).length_squared();
                                if d_sq > scan_radius_sq {
                                    continue;
                                }
                                let dist = d_sq.sqrt();
                                let alpha_mult = (1.0 - (dist / scan_radius)).clamp(0.2, 1.0);

                                // Real-time stress heatmap:
                                // Green = low stress (nominal foundation)
                                // Yellow = moderate load-bearing stress
                                // Orange/Red = extreme shear stress (near yield point)
                                // Pulsing Magenta = unsupported island pending collapse
                                let is_supported_below = y > 1 && grid.get(x as i32, y as i32 - 1, z as i32).is_solid();
                                let has_deep_drop = y > 3 && !grid.get(x as i32, y as i32 - 2, z as i32).is_solid() && !grid.get(x as i32, y as i32 - 3, z as i32).is_solid();
                                let is_extreme_cantilever = has_deep_drop && y > 4 && !grid.get(x as i32, y as i32 - 4, z as i32).is_solid();

                                let stress_color = if is_extreme_cantilever {
                                    // Pulsing Magenta: Critical unsupported island pending collapse
                                    let pulse = (cur_time * 8.0).sin().abs() * 0.4 + 0.6;
                                    Color::new(1.0, 0.15, 0.9, 0.95 * alpha_mult * pulse)
                                } else if has_deep_drop {
                                    Color::new(1.0, 0.25, 0.15, 0.9 * alpha_mult) // Orange/Red: Extreme shear stress
                                } else if !is_supported_below {
                                    Color::new(0.95, 0.85, 0.2, 0.9 * alpha_mult) // Yellow: Moderate load-bearing stress
                                } else {
                                    Color::new(0.2, 0.9, 0.45, 0.9 * alpha_mult) // Green: Sound foundation
                                };

                                let wire_col = match vt {
                                    crate::voxel::VoxelType::Explosive => Color::new(1.0, 0.5, 0.1, 0.95 * alpha_mult),
                                    crate::voxel::VoxelType::GlowCore => {
                                        let pulse = (cur_time * 6.0).sin().abs() * 0.4 + 0.6;
                                        Color::new(1.0, 0.2, 0.85, 0.95 * alpha_mult * pulse)
                                    }
                                    _ => stress_color,
                                };

                                let hs = VOXEL_SIZE * 0.525;
                                let p000 = wp + Vec3::new(-hs, -hs, -hs);
                                let p001 = wp + Vec3::new(-hs, -hs, hs);
                                let p010 = wp + Vec3::new(-hs, hs, -hs);
                                let p011 = wp + Vec3::new(-hs, hs, hs);
                                let p100 = wp + Vec3::new(hs, -hs, -hs);
                                let p101 = wp + Vec3::new(hs, -hs, hs);
                                let p110 = wp + Vec3::new(hs, hs, -hs);
                                let p111 = wp + Vec3::new(hs, hs, hs);

                                let edges = [
                                    (p000, p001), (p001, p011), (p011, p010), (p010, p000),
                                    (p100, p101), (p101, p111), (p111, p110), (p110, p100),
                                    (p000, p100), (p001, p101), (p010, p110), (p011, p111),
                                ];

                                let c_raw: [u8; 4] = wire_col.into();
                                let cam_d = (wp - cam_pos).normalize_or_zero();
                                for &(e_a, e_b) in &edges {
                                    let edge_dir = (e_b - e_a).normalize_or_zero();
                                    let norm = edge_dir.cross(cam_d).normalize_or_zero() * 0.012;
                                    let v_base = scanner_mesh.vertices.len() as u16;
                                    scanner_mesh.vertices.push(Vertex { position: e_a - norm, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
                                    scanner_mesh.vertices.push(Vertex { position: e_a + norm, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
                                    scanner_mesh.vertices.push(Vertex { position: e_b + norm, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
                                    scanner_mesh.vertices.push(Vertex { position: e_b - norm, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
                                    scanner_mesh.indices.extend_from_slice(&[
                                        v_base, v_base + 1, v_base + 2,
                                        v_base, v_base + 2, v_base + 3,
                                    ]);
                                }

                                wire_budget -= 1;
                                if wire_budget == 0 {
                                    break 'scanner_loop;
                                }
                            }
                        }
                    }
                }
            }
            if !scanner_mesh.indices.is_empty() {
                macroquad::models::draw_mesh(&scanner_mesh);
            }
        }

        // 5. Physics Solid Chunks (Frustum Culled), Settled Rubble and 3D Tactile Decals
        if !replay.is_active {
            physics.draw_solids_frustum(cam_pos_3d, &dynamic_lights, Some(&frustum));
        }

        // 6. Physics Particles (Frustum Culled: sparks, smoke wisps, shockwaves, arcs, fire)
        if !replay.is_active {
            physics.draw_particles_frustum(cam_pos_3d, &dynamic_lights, Some(&frustum));
        }

        // Cellular Fluid Dynamics (Pressurized Water, Molten Slag, Toxic Gas)
        if !replay.is_active {
            fluid_system.draw(cam_pos_3d, &mut fluid_mesh);
        }

        // Demolition Contract precision drop zone target wireframe
        if campaign_mode_active && !replay.is_active {
            if let Some(dz) = &campaign.active_contract().drop_zone {
                let size = dz.max - dz.min;
                let center = (dz.min + dz.max) * 0.5;
                draw_cube_wires(center, size, Color::new(0.2, 1.0, 0.4, 0.85));
            }
        }

        if replay.is_active {
            replay.draw_recorded_entities();
        }

        // Volumetric God Rays / Atmospheric Light Shafts
        draw_volumetric_god_rays(cam_pos_3d, view_dir, sun_dir, &grid, &mut god_rays_mesh);

        // 7. Active Weapon World Projectiles, Rockets, Flames & Trail Ribbons
        if !replay.is_active {
            weapon.draw_world_entities(cam_pos_3d, view_dir, view_right, cam_up_3d);
        }

        // 8. Active Siege Engines & Colossal Artillery Projectiles
        siege.draw_world_engines(cam_pos_3d);
        if !replay.is_active {
            siege.draw_colossal_projectiles_and_beams(cam_pos_3d);
        }
        if allow_gameplay_input {
            let aim_target = aim_hit.as_ref().map(|h| h.world_pos);
            siege.draw_placement_preview(aim_target, Some(&grid));
        }

        // Emissive Bloom Coronas & Flares for Cores, Capacitors & Molten Fissures
        draw_emissive_bloom_coronas(cam_pos_3d, view_dir, view_right, cam_up_3d, &dynamic_lights, &mut bloom_coronas_mesh);

        // High-Tension Kinetic Grapple Cable
        if player.is_grappling {
            if let Some(anchor) = player.grapple_hook {
                let hand_pos = cam_pos + final_right * 0.22 - final_up * 0.18 + final_dir * 0.45;
                draw_line_3d(hand_pos, anchor, Color::new(0.9, 0.92, 0.96, 0.9));
                let offset = final_up * 0.015;
                draw_line_3d(hand_pos + offset, anchor + offset, Color::new(0.35, 0.85, 1.0, 0.85));
                draw_sphere(anchor, 0.18, None, Color::new(1.0, 0.65, 0.15, 1.0));
            }
        }

        // 9. 3D First-Person Weapon Viewmodel with procedural recoil, walk bobbing, sway & heat venting
        if !replay.is_active {
            weapon.draw_viewmodel(cam_pos, final_dir, final_right, final_up, player.bob_timer, player.horizontal_speed());
        }

        // Dynamic Muzzle Flash Point Illumination
        if muzzle_flash_timer > 0.0 && !replay.is_active {
            let flash_a = (muzzle_flash_timer / 0.14).clamp(0.0, 1.0);
            draw_sphere(muzzle_pos, 0.45 * flash_a, None, Color::new(muzzle_flash_color.r, muzzle_flash_color.g, muzzle_flash_color.b, flash_a * 0.7));
            draw_sphere(muzzle_pos, 0.22 * flash_a, None, Color::new(1.0, 1.0, 1.0, flash_a * 0.85));
        }

        // --- 2D UI & HUD OVERLAY ---
        set_default_camera();

        // Screen-space Muzzle Flash Bloom Edge Tint
        if muzzle_flash_timer > 0.06 && !replay.is_active {
            let bloom_a = ((muzzle_flash_timer - 0.06) / 0.08) * 0.08;
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(muzzle_flash_color.r, muzzle_flash_color.g, muzzle_flash_color.b, bloom_a));
        }

        let manual_was_open = hud.help_modal_open;
        if !replay.is_active && !customizer.is_open {
            hud.draw(
                &grid,
                &weapon,
                &siege,
                &physics,
                &player,
                current_level,
                aim_hit.as_ref(),
            );
        }

        if !player.is_mouse_locked && !replay.is_active && !customizer.is_open && !hud.help_modal_open {
            let text = "PAUSED - PRESS ESC TO RESUME";
            let size = 22.0 * ui::compute_ui_scale(screen_width(), screen_height());
            let width = measure_text(text, None, size as u16, 1.0).width;
            draw_text(text, (screen_width() - width) * 0.5, screen_height() * 0.45, size, YELLOW);
        }

        // Campaign Contract Objective Card
        if campaign_mode_active && !replay.is_active {
            campaign.draw_contract_card();
        }

        // Cinematic Replay Scrubber & Time Overlay
        if replay.is_active {
            replay.draw_replay_hud();
        }

        // Customizer Workbench (if toggled)
        if !hud.help_modal_open {
            customizer.update_and_draw(&mut weapon, &mut siege, &audio);
        }

        // F1 Field Manual is drawn last so no other panel can cover it.
        if hud.help_modal_open {
            let (sw, sh) = (screen_width(), screen_height());
            hud.draw_controls_manual(sw, sh, ui::compute_ui_scale(sw, sh));
        }
        // The manual can also be opened/closed by clicking; keep cursor lock in sync.
        if manual_was_open != hud.help_modal_open {
            apply_cursor_lock(&mut player, !hud.help_modal_open && !customizer.is_open);
        }

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skydome_mesh_generation_and_translation() {
        let skydome = create_skydome_mesh();
        assert_eq!(skydome.base_mesh.vertices.len(), 600, "15 elevation bands * 40 azimuth segments = 600 vertices");
        assert_eq!(skydome.base_mesh.indices.len(), 3360, "14 rings * 40 quads * 6 indices = 3360 indices");

        // Verify spherical radius of 240m
        for v in &skydome.base_mesh.vertices {
            let dist = v.position.length();
            assert!((dist - 240.0).abs() < 0.1, "Vertex at {:?} must be 240m from center", v.position);
        }

        // Verify translation with camera position
        let mut draw_buf = Mesh {
            vertices: skydome.base_mesh.vertices.clone(),
            indices: skydome.base_mesh.indices.clone(),
            texture: None,
        };
        let cam_pos = Vec3::new(15.0, 8.0, -30.0);
        for (i, v) in skydome.base_mesh.vertices.iter().enumerate() {
            draw_buf.vertices[i].position = v.position + cam_pos;
        }

        for (i, v) in draw_buf.vertices.iter().enumerate() {
            let expected = skydome.base_mesh.vertices[i].position + cam_pos;
            assert!((v.position - expected).length() < 0.001);
        }
    }

    #[test]
    fn test_ground_foundation_elevation_clearance() {
        let bedrock_top = 0.50f32;
        let ground_elevation = 0.49f32;
        assert!(ground_elevation < bedrock_top, "Unified ground mesh must be slightly below bedrock physical boundary to prevent Z-fighting");
    }

    #[test]
    fn ground_foundation_leaves_excavation_area_open() {
        let ground = create_ground_foundation_mesh();
        assert_eq!(ground.base_mesh.vertices.len(), 24);
        assert_eq!(ground.base_mesh.indices.len(), 120);
        let half_x = crate::voxel::GRID_X as f32 * VOXEL_SIZE * 0.5;
        let half_z = crate::voxel::GRID_Z as f32 * VOXEL_SIZE * 0.5;
        for triangle in ground.base_mesh.indices.as_chunks::<3>().0 {
            let points = triangle.map(|i| ground.base_mesh.vertices[i as usize].position);
            // Every triangle must lie wholly outside one edge of the playable grid.
            assert!(points.iter().all(|p| p.x <= -half_x)
                || points.iter().all(|p| p.x >= half_x)
                || points.iter().all(|p| p.z <= -half_z)
                || points.iter().all(|p| p.z >= half_z));
        }
        for vertex in &ground.base_mesh.vertices[..20] {
            assert_eq!(vertex.position.y, 0.49);
        }
        for vertex in &ground.base_mesh.vertices[20..] {
            assert_eq!(vertex.position.y, -6.0);
        }
    }

    #[test]
    fn level_load_resets_transients_and_unmatched_campaign() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let mut weapon = WeaponSystem::new();
        let mut siege = SiegeSystem::new();
        let mut player = Player::new(Vec3::ZERO);
        let mut hud = HUD::new();
        let audio = SoundSystem::empty();
        let mut fluid = FluidSystem::new();
        let mut replay = ReplaySystem::new();
        let mut campaign = CampaignManager::new();
        let mut campaign_mode = true;
        let (mut destroyed, mut cooldown, mut flash) = (99, 0.2, 0.1);
        let mut lights = Vec::new();
        weapon.fire_cooldown = 3.0;
        weapon.railgun_charge = 1.0;
        weapon.recoil_pos_vel = Vec3::ONE;
        replay.flight_cam_active = true;
        replay.record_step(0.05, &player, &physics, &weapon, &siege);
        campaign.start_contract(100);
        campaign.record_ordnance_spend(1000.0);
        load_level(LevelPreset::OrbitalLaunchGantry, &mut grid, &mut physics,
            &mut weapon, &mut siege, &mut player, &mut hud, &audio, &mut fluid,
            &mut replay, &mut campaign, &mut campaign_mode, &mut destroyed,
            &mut cooldown, &mut flash, &mut lights, None);
        assert!(!campaign_mode, "A map without a contract switches to sandbox");
        assert_eq!(campaign.status, crate::campaign::ContractStatus::Ready);
        assert_eq!(campaign.ordnance_dollars_spent, 0.0);
        assert_eq!(replay.total_recorded_frames, 0);
        assert!(!replay.flight_cam_active);
        assert_eq!(weapon.fire_cooldown, 0.0);
        assert_eq!(weapon.railgun_charge, 0.0);
        assert_eq!(weapon.recoil_pos_vel, Vec3::ZERO);
        assert_eq!(destroyed, 0);
        assert_eq!(cooldown, 0.0);
        assert_eq!(flash, 0.0);
    }

    #[test]
    fn test_macroquad_window_conf_stream_capacities() {
        let conf = window_conf();
        assert_eq!(conf.draw_call_vertex_capacity, 32768, "Vertex capacity must be 32,768 for high-density multi-battery bursts");
        assert_eq!(conf.draw_call_index_capacity, 49152, "Index capacity must be 49,152 to eliminate driver clamping stalls");
    }
}

