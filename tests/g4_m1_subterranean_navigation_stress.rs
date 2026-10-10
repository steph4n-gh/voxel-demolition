#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DebrisParticle, DynamicChunk, PhysicsSystem, SparkParticle, SmokeParticle};
use player::{Player, PLAYER_RADIUS};
use voxel::{CarveResult, DamageType, VoxelGrid, VoxelType, MIN_VOXEL_Y, MAX_VOXEL_Y, VOID_BOUNDARY_Y, VOXEL_SIZE, GRID_X, GRID_Z};

// ============================================================================
// CHALLENGER 2 EMPIRICAL HARNESS: SUBTERRANEAN NAVIGATION & VOID RESETS
// ============================================================================

/// Helper to create a grid with solid ground from MIN_VOXEL_Y up to ground_vy
fn create_test_grid_with_floor_at(ground_vy: i32) -> VoxelGrid {
    let mut grid = VoxelGrid::new();
    for x in 0..GRID_X {
        for z in 0..GRID_Z {
            // Fill bottom bedrock up to MIN_VOXEL_Y + 16, then basalt up to ground_vy
            for vy in MIN_VOXEL_Y..=ground_vy {
                let vt = if vy < MIN_VOXEL_Y + 16 {
                    VoxelType::Bedrock
                } else {
                    VoxelType::Basalt
                };
                grid.set(x as i32, vy, z as i32, vt);
            }
            // Ensure space above ground_vy is clear
            for vy in (ground_vy + 1)..MAX_VOXEL_Y {
                grid.set(x as i32, vy, z as i32, VoxelType::Air);
            }
        }
    }
    grid
}

/// Helper to calculate the expected ground world Y for a given voxel Y
fn voxel_top_world_y(grid: &VoxelGrid, vx: i32, vy: i32, vz: i32) -> f32 {
    let vw = grid.voxel_to_world(IVec3::new(vx, vy, vz));
    vw.y + VOXEL_SIZE * 0.5
}

// ----------------------------------------------------------------------------
// 1. SUBTERRANEAN CRATER GROUNDING SWEEP (Y in [-23.5, 0.0])
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_subterranean_crater_depth_grounding_sweep() {
    // Test across a wide range of subterranean voxel heights:
    // MIN_VOXEL_Y is -48. Voxel -48 top is (-48 + 1) * 0.5 = -23.5m.
    // Voxel -1 top is 0.0m.
    let test_vys = [
        -48, // -23.5m (deepest solid layer above void boundary)
        -44, // -21.5m
        -40, // -19.5m
        -32, // -15.5m
        -24, // -11.5m
        -16, // -7.5m
        -10, // -4.5m
        -5,  // -2.0m
        -2,  // -0.5m
        -1,  // 0.0m (subterranean ceiling)
    ];

    for &vy in &test_vys {
        let grid = create_test_grid_with_floor_at(vy);
        let center_x = 48;
        let center_z = 48;
        let expected_ground = voxel_top_world_y(&grid, center_x, vy, center_z);
        let wpos = grid.voxel_to_world(IVec3::new(center_x, vy, center_z));

        let mut p = Player::new(Vec3::new(wpos.x, expected_ground + 0.04, wpos.z));
        p.velocity = Vec3::ZERO;
        p.is_grounded = false;

        // Step simulation
        p.update(0.016, &grid, None, false);

        assert!(
            p.is_grounded,
            "Player must be grounded at subterranean vy = {} (expected Y = {:.2}), got grounded = {}, pos.y = {:.2}",
            vy, expected_ground, p.is_grounded, p.position.y
        );

        assert!(
            (p.position.y - expected_ground).abs() < 0.005,
            "Player at subterranean vy = {} must rest precisely at {:.3}, got {:.3}",
            vy, expected_ground, p.position.y
        );

        assert!(
            p.position.y < 0.49 || vy >= 0,
            "Subterranean player must NEVER snap back to surface +0.50m! vy={}, pos.y={:.2}",
            vy, p.position.y
        );
    }
}

// ----------------------------------------------------------------------------
// 2. SUBTERRANEAN SLOPING TRENCH WALKING CONTINUITY
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_subterranean_crater_slope_and_walking_continuity() {
    let mut grid = VoxelGrid::new();
    let center_z = 48;

    // Build a stepped trench along X from x=20 to x=60 stepping down from vy=-2 down to vy=-30
    for x in 20..=60 {
        let step_down = (x - 20) / 2; // Drops 1 voxel every 2 blocks
        let floor_vy = (-2 - step_down).max(MIN_VOXEL_Y);
        for vy in MIN_VOXEL_Y..=floor_vy {
            grid.set(x, vy, center_z - 1, VoxelType::Basalt);
            grid.set(x, vy, center_z, VoxelType::Basalt);
            grid.set(x, vy, center_z + 1, VoxelType::Basalt);
        }
        for vy in (floor_vy + 1)..MAX_VOXEL_Y {
            grid.set(x, vy, center_z - 1, VoxelType::Air);
            grid.set(x, vy, center_z, VoxelType::Air);
            grid.set(x, vy, center_z + 1, VoxelType::Air);
        }
    }

    let start_wpos = grid.voxel_to_world(IVec3::new(21, -2, center_z));
    let start_ground = start_wpos.y + VOXEL_SIZE * 0.5;

    let mut p = Player::new(Vec3::new(start_wpos.x, start_ground, start_wpos.z));
    p.velocity = Vec3::new(3.0, 0.0, 0.0); // Moving forward along +X into trench

    let mut min_observed_y = p.position.y;
    let mut max_observed_y = p.position.y;

    for frame in 0..120 {
        p.velocity.x = 3.0; // Maintain walking speed
        p.update(0.016, &grid, None, false);

        assert!(p.position.y.is_finite(), "Frame {}: Player Y must not be NaN/Inf", frame);
        assert!(p.position.x.is_finite(), "Frame {}: Player X must not be NaN/Inf", frame);

        min_observed_y = min_observed_y.min(p.position.y);
        max_observed_y = max_observed_y.max(p.position.y);

        // Player must never snap to surface 0.50m once descending
        if p.position.x > start_wpos.x + 1.0 {
            assert!(
                p.position.y < 0.1,
                "Frame {}: Player at x={:.2} snapped to surface Y={:.2}!",
                frame, p.position.x, p.position.y
            );
        }
    }

    assert!(
        p.position.x > start_wpos.x + 4.0,
        "Player must advance through subterranean trench, ended at x={:.2}",
        p.position.x
    );
    assert!(
        min_observed_y < -2.0,
        "Player must successfully descend into subterranean depths, min Y = {:.2}",
        min_observed_y
    );
}

// ----------------------------------------------------------------------------
// 3. SUBTERRANEAN JUMPING, GRAVITY & LANDING SHOCK
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_subterranean_jumping_and_landing_shock() {
    let floor_vy = -24; // Subterranean depth around -11.5m
    let grid = create_test_grid_with_floor_at(floor_vy);
    let center_x = 48;
    let center_z = 48;
    let ground_y = voxel_top_world_y(&grid, center_x, floor_vy, center_z);
    let wpos = grid.voxel_to_world(IVec3::new(center_x, floor_vy, center_z));

    let mut p = Player::new(Vec3::new(wpos.x, ground_y, wpos.z));
    p.velocity = Vec3::ZERO;
    p.is_grounded = true;

    // Trigger jump via jump_buffer
    p.jump_buffer = 0.15;
    p.update(0.016, &grid, None, false);

    assert!(p.just_jumped, "Buffered jump must set just_jumped");
    assert!(!p.is_grounded, "Jumping must unground player");
    assert!(p.velocity.y > 7.0, "Jump must apply upward impulse, got {:.2}", p.velocity.y);

    let mut reached_apex = false;
    let mut apex_y = ground_y;
    let mut landed = false;

    for _ in 0..120 {
        p.update(0.016, &grid, None, false);
        apex_y = apex_y.max(p.position.y);

        if p.velocity.y < 0.0 {
            reached_apex = true;
        }

        if p.is_grounded && reached_apex {
            landed = true;
            break;
        }
    }

    assert!(reached_apex, "Jump must reach apex");
    assert!(apex_y > ground_y + 1.2, "Jump apex ({:.2}) must be well above ground ({:.2})", apex_y, ground_y);
    assert!(landed, "Player must land back down on subterranean ground");
    assert!(
        (p.position.y - ground_y).abs() < 0.01,
        "Player must land precisely on subterranean floor {:.2}, got {:.2}",
        ground_y, p.position.y
    );
    assert!(p.just_landed.is_some(), "Landing shock must be registered");
    assert!(p.trauma > 0.05, "Landing shock must apply camera trauma, got {:.3}", p.trauma);
}

// ----------------------------------------------------------------------------
// 4. STEP-UP CLIMBING 1-VOXEL TERRACES (<= 0.55m) ACROSS ALL DIRECTIONS
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_step_up_1_voxel_terraces_all_directions() {
    let base_vy = -20; // -9.5m subterranean depth
    let directions = [
        ("positive X", Vec3::new(1.0, 0.0, 0.0), IVec3::new(1, 0, 0)),
        ("negative X", Vec3::new(-1.0, 0.0, 0.0), IVec3::new(-1, 0, 0)),
        ("positive Z", Vec3::new(0.0, 0.0, 1.0), IVec3::new(0, 0, 1)),
        ("negative Z", Vec3::new(0.0, 0.0, -1.0), IVec3::new(0, 0, -1)),
    ];

    for (dir_name, vel_dir, grid_step) in &directions {
        let mut grid = create_test_grid_with_floor_at(base_vy);
        let start_vx = 48;
        let start_vz = 48;

        // Add 1-voxel step in the target direction (height = 1 voxel = 0.50m <= 0.55m)
        let step_vx = start_vx + grid_step.x;
        let step_vz = start_vz + grid_step.z;
        grid.set(step_vx, base_vy + 1, step_vz, VoxelType::Basalt);

        let start_wpos = grid.voxel_to_world(IVec3::new(start_vx, base_vy, start_vz));
        let base_ground_y = start_wpos.y + VOXEL_SIZE * 0.5;
        let step_target_y = base_ground_y + VOXEL_SIZE; // +0.50m

        let mut p = Player::new(Vec3::new(start_wpos.x, base_ground_y, start_wpos.z));
        p.velocity = *vel_dir * 4.0;

        // Advance several frames
        for _ in 0..10 {
            p.velocity = *vel_dir * 4.0;
            p.update(0.016, &grid, None, false);
        }

        assert!(
            (p.position.y - step_target_y).abs() < 0.05,
            "Direction {}: Player must step up to {:.2}, got Y = {:.2}",
            dir_name, step_target_y, p.position.y
        );

        // Verify player moved into the step column
        let delta = p.position - start_wpos;
        let progress = delta.dot(*vel_dir);
        assert!(
            progress > 0.15,
            "Direction {}: Player must advance onto terrace, progress = {:.2}",
            dir_name, progress
        );
    }
}

// ----------------------------------------------------------------------------
// 5. STEP-UP BLOCKED BY HEADROOM OBSTRUCTION
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_step_up_blocked_by_low_ceiling() {
    let base_vy = -16;
    let mut grid = create_test_grid_with_floor_at(base_vy);
    let start_vx = 48;
    let start_vz = 48;

    // 1-voxel step at x=49
    grid.set(start_vx + 1, base_vy + 1, start_vz, VoxelType::Basalt);

    // Overhang obstacle right above the step (headroom < PLAYER_HEIGHT = 1.80m)
    // base_vy + 1 is step. base_vy + 3 is 1.0m above step (less than 1.80m headroom)
    grid.set(start_vx + 1, base_vy + 3, start_vz, VoxelType::HeavyArmor);

    let start_wpos = grid.voxel_to_world(IVec3::new(start_vx, base_vy, start_vz));
    let base_ground_y = start_wpos.y + VOXEL_SIZE * 0.5;

    let mut p = Player::new(Vec3::new(start_wpos.x, base_ground_y, start_wpos.z));
    p.velocity = Vec3::new(4.0, 0.0, 0.0);

    for _ in 0..10 {
        p.velocity = Vec3::new(4.0, 0.0, 0.0);
        p.update(0.016, &grid, None, false);
    }

    assert!(
        (p.position.y - base_ground_y).abs() < 0.02,
        "Player must NOT step up when headroom is obstructed! Y={:.2}, expected base={:.2}",
        p.position.y, base_ground_y
    );
    assert_eq!(p.velocity.x, 0.0, "Blocked horizontal velocity must be zeroed");
}

// ----------------------------------------------------------------------------
// 6. 2-VOXEL WALL (1.0m) SLIDING & PENETRATION REJECTION
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_wall_sliding_2_voxel_wall_no_penetration() {
    let base_vy = -14;
    let mut grid = create_test_grid_with_floor_at(base_vy);
    let start_vx = 48;
    let start_vz = 48;

    // 2-voxel high wall at x=49 (1.0m > MAX_STEP_UP 0.55m)
    for z in 45..=55 {
        grid.set(start_vx + 1, base_vy + 1, z, VoxelType::Concrete);
        grid.set(start_vx + 1, base_vy + 2, z, VoxelType::Concrete);
    }

    let start_wpos = grid.voxel_to_world(IVec3::new(start_vx, base_vy, start_vz));
    let wall_wpos = grid.voxel_to_world(IVec3::new(start_vx + 1, base_vy, start_vz));
    let wall_face_x = wall_wpos.x - VOXEL_SIZE * 0.5;
    let base_ground_y = start_wpos.y + VOXEL_SIZE * 0.5;

    // Start player facing wall at x=48, close to wall
    let initial_x = wall_face_x - PLAYER_RADIUS - 0.05;
    let mut p = Player::new(Vec3::new(initial_x, base_ground_y, start_wpos.z));

    // Diagonal velocity: pushing into wall (+X) while sliding along wall (+Z)
    p.velocity = Vec3::new(5.0, 0.0, 5.0);

    for frame in 0..30 {
        p.velocity.x = 5.0; // Continue trying to walk into wall
        p.velocity.z = 5.0; // Continue sliding along wall
        p.update(0.016, &grid, None, false);

        // Player must never penetrate into the wall
        assert!(
            p.position.x <= wall_face_x - PLAYER_RADIUS + 0.02,
            "Frame {}: Player penetrated wall! x={:.3}, wall_face_x={:.3}",
            frame, p.position.x, wall_face_x
        );

        // Normal velocity into wall must be killed
        assert_eq!(
            p.velocity.x, 0.0,
            "Frame {}: Velocity into wall must be zeroed for wall sliding",
            frame
        );

        // Vertical step must NOT occur
        assert!(
            (p.position.y - base_ground_y).abs() < 0.02,
            "Frame {}: Player must not step up 1.0m wall! Y={:.2}",
            frame, p.position.y
        );
    }

    // Player must have slid substantially along Z
    assert!(
        p.position.z > start_wpos.z + 1.5,
        "Player must slide along wall in Z direction, ended at z={:.2}",
        p.position.z
    );
}

// ----------------------------------------------------------------------------
// 7. CORNER TRAPPING INVARIANTS (BOTH X AND Z BLOCKED)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_corner_trapping_zero_penetration() {
    let base_vy = -10;
    let mut grid = create_test_grid_with_floor_at(base_vy);
    let start_vx = 48;
    let start_vz = 48;

    // Corner walls: +X wall and +Z wall (2 voxels high = 1.0m)
    for vy_offset in 1..=2 {
        for z in 45..=52 {
            grid.set(start_vx + 1, base_vy + vy_offset, z, VoxelType::Concrete);
        }
        for x in 45..=52 {
            grid.set(x, base_vy + vy_offset, start_vz + 1, VoxelType::Concrete);
        }
    }

    let start_wpos = grid.voxel_to_world(IVec3::new(start_vx, base_vy, start_vz));
    let base_ground_y = start_wpos.y + VOXEL_SIZE * 0.5;

    let mut p = Player::new(Vec3::new(start_wpos.x, base_ground_y, start_wpos.z));
    p.velocity = Vec3::new(6.0, 0.0, 6.0); // Pushing directly into internal corner

    for _ in 0..20 {
        p.velocity = Vec3::new(6.0, 0.0, 6.0);
        p.update(0.016, &grid, None, false);
    }

    assert_eq!(p.velocity.x, 0.0, "Corner must halt X velocity");
    assert_eq!(p.velocity.z, 0.0, "Corner must halt Z velocity");
    assert!(
        (p.position.y - base_ground_y).abs() < 0.02,
        "Corner walls must not allow climbing or popping out"
    );
}

// ----------------------------------------------------------------------------
// 8. SAFE BOTTOM VOID BOUNDARY RESET (Y <= -24.0m)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_safe_bottom_void_boundary_reset_exhaustive() {
    let spawn_pos = Vec3::new(12.5, 0.5, 34.0);
    let mut p = Player::new(spawn_pos);
    let grid = VoxelGrid::new(); // Empty grid (void everywhere)

    // Scenario A: Player falls just above boundary (-23.95m) -> should NOT reset yet
    p.position = Vec3::new(0.0, -23.95, 0.0);
    p.velocity = Vec3::new(0.0, -10.0, 0.0);
    p.update(0.001, &grid, None, false);
    // As long as Y > -24.0, no reset
    if p.position.y > VOID_BOUNDARY_Y {
        assert_ne!(p.position, spawn_pos);
    }

    // Scenario B: Player crosses VOID_BOUNDARY_Y (-24.0m)
    let test_fall_depths = [-24.0, -24.001, -25.0, -50.0, -100.0];
    for &depth in &test_fall_depths {
        p.position = Vec3::new(42.0, depth, -17.0);
        p.velocity = Vec3::new(15.0, -85.0, -25.0);
        p.trauma = 0.95;
        p.fov_kick = 15.0;
        p.is_grappling = true;
        p.grapple_hook = Some(Vec3::new(0.0, 10.0, 0.0));
        p.coyote_timer = 0.12;
        p.jump_buffer = 0.10;
        p.dash_cooldown = 0.50;
        p.just_respawned = false;

        p.update(0.016, &grid, None, false);

        assert_eq!(
            p.position, spawn_pos,
            "Falling to Y={:.2} must reset player to spawn_pos",
            depth
        );
        assert_eq!(
            p.velocity,
            Vec3::ZERO,
            "Velocity must be zeroed upon void reset"
        );
        assert_eq!(p.trauma, 0.0, "Trauma must be completely cleared upon void reset");
        assert_eq!(p.fov_kick, 0.0, "FOV kick must be cleared upon void reset");
        assert!(p.is_grounded, "Player must be set grounded at spawn");
        assert!(!p.is_grappling, "Grapple state must be cleared");
        assert!(p.grapple_hook.is_none(), "Grapple hook anchor must be cleared");
        assert_eq!(p.coyote_timer, 0.0, "Coyote timer must be cleared");
        assert_eq!(p.jump_buffer, 0.0, "Jump buffer must be cleared");
        assert_eq!(p.dash_cooldown, 0.0, "Dash cooldown must be cleared");
        assert!(p.just_respawned, "just_respawned flag must be set to true");
    }

    // Scenario C: Frame immediately following respawn stays clean
    p.update(0.016, &grid, None, false);
    assert_eq!(p.velocity.x, 0.0);
    assert_eq!(p.velocity.z, 0.0);
    assert_eq!(p.trauma, 0.0);
}

// ----------------------------------------------------------------------------
// 9. DYNAMIC CHUNKS VOID DESPAWN (Y <= -24.0m)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_dynamic_chunks_void_despawn_sweep() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    // Clear entire grid to air so chunks fall freely straight into the void
    for x in 0..GRID_X {
        for vy in MIN_VOXEL_Y..MAX_VOXEL_Y {
            for z in 0..GRID_Z {
                grid.set(x as i32, vy, z as i32, VoxelType::Air);
            }
        }
    }
    phys.rubble_height_grid.fill(VOID_BOUNDARY_Y);
    let audio = SoundSystem::empty();

    // Spawn 40 dynamic chunks at subterranean altitudes falling rapidly
    for i in 0..40 {
        let spawn_y = -18.0 - (i as f32 * 0.15); // between -18.0 and -24.0
        let vox = vec![(Vec3::ZERO, VoxelType::Basalt)];
        let chunk = DynamicChunk {
            voxels: vox.clone(),
            exposed_voxels: vox,
            voxel_health: Vec::new(),
            position: Vec3::new((i % 5) as f32, spawn_y, (i / 5) as f32),
            velocity: Vec3::new(0.0, -30.0, 0.0), // Fast downward plunge
            angular_velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: physics::Mat3::IDENTITY,
            mass: 15.0,
            settled: false,
            life: 0.0,
            local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
        };
        phys.chunks.push(chunk);
    }

    assert_eq!(phys.chunks.len(), 40);

    // Step physics forward until all chunks plunge past VOID_BOUNDARY_Y (-24.0m)
    for _ in 0..50 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));
    }

    if !phys.chunks.is_empty() {
        for (idx, ch) in phys.chunks.iter().enumerate() {
            eprintln!("REMAINING CHUNK {}: pos={:?}, vel={:?}, settled={}, life={}", idx, ch.position, ch.velocity, ch.settled, ch.life);
        }
    }

    assert_eq!(
        phys.chunks.len(), 0,
        "All 40 dynamic chunks falling below VOID_BOUNDARY_Y must be cleanly despawned without leaks"
    );

    // Test sub-stepping ultra-high-velocity chunk passing -24.0m in a single frame
    let supersonic_vox = vec![(Vec3::ZERO, VoxelType::Bedrock)];
    let supersonic_chunk = DynamicChunk {
        voxels: supersonic_vox.clone(),
        exposed_voxels: supersonic_vox,
        voxel_health: Vec::new(),
        position: Vec3::new(0.0, -22.0, 0.0),
        velocity: Vec3::new(0.0, -300.0, 0.0), // -4.8m in 0.016s -> leaps past -24.0 during substepping
        angular_velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        mass: 100.0,
        settled: false,
        life: 0.0,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    };
    phys.chunks.push(supersonic_chunk);

    phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));

    assert_eq!(
        phys.chunks.len(), 0,
        "High-velocity chunk leaping past void boundary during sub-steps must be cleanly despawned"
    );
}

// ----------------------------------------------------------------------------
// 10. DEBRIS, SPARKS, SMOKE VOID DESPAWN (Y <= -24.0m)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_debris_sparks_smoke_void_despawn_sweep() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    for x in 0..GRID_X {
        for vy in MIN_VOXEL_Y..MAX_VOXEL_Y {
            for z in 0..GRID_Z {
                grid.set(x as i32, vy, z as i32, VoxelType::Air);
            }
        }
    }
    phys.rubble_height_grid.fill(VOID_BOUNDARY_Y);
    let audio = SoundSystem::empty();

    // Populate 300 debris, 150 sparks, 100 smoke near void boundary
    for i in 0..300 {
        phys.debris.push(DebrisParticle {
            position: Vec3::new((i % 10) as f32, -23.0 - (i as f32 * 0.005), 0.0),
            velocity: Vec3::new(0.0, -15.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.15,
            color: WHITE,
            life: 10.0,
            max_life: 10.0,
            bounces: 0,
        });
    }

    for i in 0..150 {
        phys.sparks.push(SparkParticle {
            position: Vec3::new(0.0, -23.5, (i % 10) as f32),
            velocity: Vec3::new(0.0, -20.0, 0.0),
            color: YELLOW,
            life: 5.0,
            max_life: 5.0,
        });
    }

    for i in 0..100 {
        phys.smoke.push(SmokeParticle {
            position: Vec3::new(0.0, -23.8, (i % 10) as f32),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            size: 0.2,
            start_size: 0.2,
            max_size: 1.0,
            color: GRAY,
            life: 4.0,
            max_life: 4.0,
        });
    }

    assert_eq!(phys.debris.len(), 300);
    assert_eq!(phys.sparks.len(), 150);
    assert_eq!(phys.smoke.len(), 100);

    // Step physics forward
    for _ in 0..25 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));
    }

    assert_eq!(
        phys.debris.len(), 0,
        "All debris particles crossing VOID_BOUNDARY_Y must be cleanly despawned"
    );
    assert_eq!(
        phys.sparks.len(), 0,
        "All sparks crossing VOID_BOUNDARY_Y must be cleanly despawned"
    );
    assert_eq!(
        phys.smoke.len(), 0,
        "All smoke particles crossing VOID_BOUNDARY_Y must be cleanly despawned"
    );
}

// ----------------------------------------------------------------------------
// 11. SUBTERRANEAN CRATER WITH SETTLED RUBBLE PLAYER GROUNDING
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_subterranean_crater_with_settled_rubble() {
    let mut phys = PhysicsSystem::new();
    let crater_vy = -30; // ~ -14.5m subterranean depth
    let mut grid = create_test_grid_with_floor_at(crater_vy);
    let center = Vec3::new(0.0, 0.0, 0.0);

    // Carve crater cavity in grid
    let mut carve_result = CarveResult::default();
    grid.carve_sphere_advanced_into(center, 12.0, 10000.0, DamageType::Explosive, &mut carve_result);
    phys.update_crater_floor(&grid, center, 12.0);

    let rubble_h = phys.settled_height_at(center).expect("Crater floor must have rubble height");
    assert!(
        (VOID_BOUNDARY_Y..0.0).contains(&rubble_h),
        "Excavated crater rubble height must be subterranean, got {:.2}",
        rubble_h
    );

    // Now place player in crater above the rubble
    let mut p = Player::new(Vec3::new(0.0, rubble_h + 0.03, 0.0));
    p.velocity = Vec3::ZERO;
    p.update(0.016, &grid, Some(&phys), false);

    assert!(p.is_grounded, "Player must ground on subterranean crater rubble");
    assert!(
        (p.position.y - rubble_h).abs() < 0.01,
        "Player position Y must match crater rubble height {:.2}, got {:.2}",
        rubble_h, p.position.y
    );
}
