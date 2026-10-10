// ============================================================================
// EMPIRICAL ADVERSARIAL STRESS TEST SUITE: MILESTONE G5-M5
// Performance Benchmarks, 60+ FPS Profiling & Master Verification
// Target: tests/g5_m5_challenger_1_stress.rs
// Authored by: g5_m5_challenger_1 (Empirical Challenger)
// ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use macroquad::prelude::*;
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{
    DebrisParticle, DynamicChunk, Mat3, PhysicsSystem, ShockwaveRing, SmokeParticle, SparkParticle,
};
use voxel_demolition::voxel::{
    IslandChunk, MaterialProperties, VoxelGrid, VoxelType, GRID_X, MAX_VOXEL_Y, VOXEL_SIZE,
};
use voxel_demolition::weapon::WeaponSystem;

// ============================================================================
// 1. TrackingAllocator for Challenger Stress Suite
// ============================================================================

struct ChallengerTrackingAllocator;
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static TRACKING_ENABLED: AtomicUsize = AtomicUsize::new(0);

static STRESS_MUTEX: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for ChallengerTrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING_ENABLED.load(Ordering::Relaxed) != 0 {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: ChallengerTrackingAllocator = ChallengerTrackingAllocator;

fn start_tracking() {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    ALLOC_BYTES.store(0, Ordering::SeqCst);
    TRACKING_ENABLED.store(1, Ordering::SeqCst);
}

fn stop_tracking() -> (usize, usize) {
    TRACKING_ENABLED.store(0, Ordering::SeqCst);
    (
        ALLOC_COUNT.load(Ordering::SeqCst),
        ALLOC_BYTES.load(Ordering::SeqCst),
    )
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * (pct / 100.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// Skyscraper generator matching benchmark dimensions
fn build_stress_skyscraper(
    grid: &mut VoxelGrid,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    height: i32,
    floor_height: i32,
) {
    let cx = (min_x + max_x) / 2;
    let cz = (min_z + max_z) / 2;

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            for y in 1..=4 {
                let vt = if y == 1 || x == min_x || x == max_x || z == min_z || z == max_z {
                    VoxelType::HeavyArmor
                } else if (x + z) % 2 == 0 {
                    VoxelType::CompositeRebar
                } else {
                    VoxelType::Concrete
                };
                grid.set(x, y, z, vt);
            }
        }
    }

    let floors = (height - 4) / floor_height;
    for f in 0..floors {
        let y_base = 5 + f * floor_height;
        let y_ceil = (y_base + floor_height - 1).min(height);

        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let vt = if (x + z) % 3 == 0 {
                    VoxelType::SteelRebar
                } else {
                    VoxelType::Concrete
                };
                grid.set(x, y_base, z, vt);
            }
        }

        for y in (y_base + 1)..=y_ceil {
            for &px in &[min_x, max_x - 2] {
                for &pz in &[min_z, max_z - 2] {
                    for dx in 0..3 {
                        for dz in 0..3 {
                            grid.set(px + dx, y, pz + dz, VoxelType::CompositeRebar);
                        }
                    }
                }
            }

            for dx in -2i32..=2i32 {
                for dz in -2i32..=2i32 {
                    let vt = if dx.abs() == 2 || dz.abs() == 2 {
                        VoxelType::CompositeRebar
                    } else {
                        VoxelType::SteelRebar
                    };
                    grid.set(cx + dx, y, cz + dz, vt);
                }
            }

            for x in (min_x + 3)..(max_x - 2) {
                grid.set(x, y, cz, VoxelType::Concrete);
            }
            for z in (min_z + 3)..(max_z - 2) {
                grid.set(cx, y, z, VoxelType::Concrete);
            }

            for x in (min_x + 3)..(max_x - 2) {
                grid.set(x, y, min_z, VoxelType::Glass);
                grid.set(x, y, max_z, VoxelType::Glass);
            }
            for z in (min_z + 3)..(max_z - 2) {
                grid.set(min_x, y, z, VoxelType::Glass);
                grid.set(max_x, y, z, VoxelType::Glass);
            }
        }
    }

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            grid.set(x, height, z, VoxelType::HeavyArmor);
        }
    }
}

// ============================================================================
// 2. Adversarial Stress Test: Domino Cascade Under Varied Impact Angles
// ============================================================================

#[test]
fn test_stress_domino_skyscraper_varied_impact_angles() {
    let _guard = STRESS_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let test_angles = [
        ("Pure Horizontal (+X)", Vec3::new(1.0, 0.0, 0.0), 650_000.0),
        ("Steep Downward Pitch", Vec3::new(0.707, -0.707, 0.0).normalize(), 650_000.0),
        ("Skewed Lateral Yaw (+X, +Z)", Vec3::new(0.707, -0.2, 0.68).normalize(), 650_000.0),
        ("Diagonal Reverse Glance", Vec3::new(0.6, -0.3, -0.7).normalize(), 600_000.0),
        ("Extreme Hypervelocity Shock", Vec3::new(0.96, -0.22, 0.08).normalize(), 1_500_000.0),
    ];

    for (desc, dir, target_momentum) in test_angles {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(48.0, 30.0, 48.0);

        build_stress_skyscraper(&mut grid, 16, 34, 39, 57, 58, 6);
        build_stress_skyscraper(&mut grid, 38, 56, 39, 57, 58, 6);
        build_stress_skyscraper(&mut grid, 60, 78, 39, 57, 58, 6);
        grid.update_meshes();

        // Sever foundation base of Tower 1
        for x in 15..=35 {
            for z in 38..=58 {
                for y in 1..=4 {
                    grid.set(x, y, z, VoxelType::Air);
                }
            }
        }

        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "Severing base must yield unsupported island");

        for island in islands {
            let mass = island.calculate_mass();
            let speed = (target_momentum / mass).max(20.0);
            let impulse = dir * speed;
            let angular = Vec3::new(0.1, 0.15, 0.6);
            physics.spawn_island_chunk(island, impulse, angular);
        }

        let dt = 0.016f32;
        let mut frame_times_ms = Vec::with_capacity(120);
        let mut structural_cooldown = 0.0f32;

        for _frame in 0..120 {
            let frame_start = Instant::now();
            structural_cooldown -= dt;

            physics.update(dt, &mut grid, &audio, cam_pos);

            if grid.structural_dirty && structural_cooldown <= 0.0 {
                structural_cooldown = 0.05;
                let max_y = (grid.max_solid_y + 1).min(MAX_VOXEL_Y - 1);
                let mut min_x = 999;
                let mut max_x = -999;
                for c in &physics.chunks {
                    let vx = grid.world_to_voxel(c.position).x;
                    min_x = min_x.min(vx - 8);
                    max_x = max_x.max(vx + 10);
                }
                let bound_min_x = min_x.clamp(14, 52);
                let bound_max_x = max_x.clamp(bound_min_x + 14, 62);
                grid.mark_dirty_region(IVec3::new(bound_min_x, 1, 37), IVec3::new(bound_max_x, max_y, 59));

                let new_islands = grid.detect_unsupported_islands();
                for island in new_islands {
                    physics.spawn_island_chunk(island, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.1, 0.0, 0.1));
                }
            }

            grid.update_meshes();

            let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            frame_times_ms.push(elapsed_ms);
        }

        // Verify numeric stability across all active chunks
        for c in &physics.chunks {
            assert!(c.position.is_finite(), "Position must be finite");
            assert!(c.velocity.is_finite(), "Velocity must be finite");
            assert!(c.angular_velocity.is_finite(), "Angular velocity must be finite");
        }

        let mut sorted_times = frame_times_ms.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg_ms = sorted_times.iter().sum::<f64>() / sorted_times.len() as f64;
        let p99_ms = percentile(&sorted_times, 99.0);

        let p99_budget = if cfg!(debug_assertions) { 75.0 } else { 16.666 };
        println!(
            "[STRESS ANGLE: {}] Avg: {:.3} ms (<= 12.0ms), P99: {:.3} ms (<= {:.3}ms), Chunks: {}",
            desc,
            avg_ms,
            p99_ms,
            p99_budget,
            physics.chunks.len()
        );

        assert!(
            avg_ms <= 12.0,
            "Angle '{}': average frame time {:.3}ms exceeded 12.0ms budget",
            desc,
            avg_ms
        );
        assert!(
            p99_ms <= p99_budget,
            "Angle '{}': p99 frame time {:.3}ms exceeded {:.3}ms budget",
            desc,
            p99_ms,
            p99_budget
        );
    }
}

// ============================================================================
// 3. Adversarial Stress Test: Domino Cascade Under Scaled Island Masses
// ============================================================================

#[test]
fn test_stress_domino_skyscraper_varied_island_masses() {
    let _guard = STRESS_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let mass_multipliers = [
        ("Lightweight Hollow Shell (0.3x)", 0.3f32),
        ("Standard Massive Reinforced (1.0x)", 1.0f32),
        ("Hyper-Dense Armored Core (2.5x)", 2.5f32),
    ];

    for (desc, mass_mult) in mass_multipliers {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(48.0, 30.0, 48.0);

        build_stress_skyscraper(&mut grid, 16, 34, 39, 57, 58, 6);
        build_stress_skyscraper(&mut grid, 38, 56, 39, 57, 58, 6);
        build_stress_skyscraper(&mut grid, 60, 78, 39, 57, 58, 6);
        grid.update_meshes();

        for x in 15..=35 {
            for z in 38..=58 {
                for y in 1..=4 {
                    grid.set(x, y, z, VoxelType::Air);
                }
            }
        }

        let islands = grid.detect_unsupported_islands();
        for island in islands {
            let base_mass = island.calculate_mass();
            let scaled_mass = base_mass * mass_mult;
            let dir = Vec3::new(0.96, -0.22, 0.08).normalize();
            let speed = (650_000.0 / scaled_mass).max(18.0);
            let impulse = dir * speed;
            let angular = Vec3::new(0.08, 0.12, 0.65);

            physics.spawn_island_chunk(island, impulse, angular);
            if let Some(chunk) = physics.chunks.last_mut() {
                chunk.mass = scaled_mass;
            }
        }

        let dt = 0.016f32;
        let mut frame_times_ms = Vec::with_capacity(120);
        let mut structural_cooldown = 0.0f32;
        let mut max_phys = 0.0f64;
        let mut max_islands = 0.0f64;
        let mut max_meshes = 0.0f64;

        for frame in 0..120 {
            let frame_start = Instant::now();
            structural_cooldown -= dt;

            let t0 = Instant::now();
            physics.update(dt, &mut grid, &audio, cam_pos);
            let t_phys = t0.elapsed().as_secs_f64() * 1000.0;
            if t_phys > max_phys { max_phys = t_phys; }

            let t1 = Instant::now();
            if grid.structural_dirty && structural_cooldown <= 0.0 {
                structural_cooldown = 0.05;
                let max_y = (grid.max_solid_y + 1).min(MAX_VOXEL_Y - 1);
                let mut min_x = 999;
                let mut max_x = -999;
                for c in &physics.chunks {
                    let vx = grid.world_to_voxel(c.position).x;
                    min_x = min_x.min(vx - 8);
                    max_x = max_x.max(vx + 10);
                }
                let bound_min_x = min_x.clamp(14, 52);
                let bound_max_x = max_x.clamp(bound_min_x + 14, 62);
                grid.mark_dirty_region(IVec3::new(bound_min_x, 1, 37), IVec3::new(bound_max_x, max_y, 59));

                let new_islands = grid.detect_unsupported_islands();
                for island in new_islands {
                    physics.spawn_island_chunk(island, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.1, 0.0, 0.1));
                }
            }
            let t_islands = t1.elapsed().as_secs_f64() * 1000.0;
            if t_islands > max_islands { max_islands = t_islands; }

            let t2 = Instant::now();
            grid.update_meshes();
            let t_meshes = t2.elapsed().as_secs_f64() * 1000.0;
            if t_meshes > max_meshes { max_meshes = t_meshes; }

            let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            frame_times_ms.push(elapsed_ms);
            if elapsed_ms > 15.0 {
                println!("  [Slow Frame {}] total={:.3}ms (phys={:.3}ms, islands={:.3}ms, meshes={:.3}ms)", frame, elapsed_ms, t_phys, t_islands, t_meshes);
            }
        }

        let mut sorted_times = frame_times_ms.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg_ms = sorted_times.iter().sum::<f64>() / sorted_times.len() as f64;
        let p99_ms = percentile(&sorted_times, 99.0);

        let p99_budget = if cfg!(debug_assertions) { 75.0 } else { 16.666 };
        println!(
            "[STRESS MASS: {}] Avg: {:.3} ms (<= 12.0ms), P99: {:.3} ms (<= {:.3}ms), Chunks: {} | max_phys: {:.3}ms, max_islands: {:.3}ms, max_meshes: {:.3}ms",
            desc,
            avg_ms,
            p99_ms,
            p99_budget,
            physics.chunks.len(),
            max_phys,
            max_islands,
            max_meshes
        );

        assert!(
            avg_ms <= 12.0,
            "Mass '{}': average frame time {:.3}ms exceeded 12.0ms budget",
            desc,
            avg_ms
        );
        assert!(
            p99_ms <= p99_budget,
            "Mass '{}': p99 frame time {:.3}ms exceeded {:.3}ms budget",
            desc,
            p99_ms,
            p99_budget
        );
    }
}

// ============================================================================
// 4. Adversarial Stress Test: Repeated C4 Foundation Cutting Runs
// ============================================================================

#[test]
fn test_stress_c4_foundation_cutting_repeated_runs_zero_alloc() {
    let _guard = STRESS_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let col_x_coords = [42, 44, 46, 48, 50, 52];
    let col_z_coords = [42, 45, 48, 51];

    for iteration in 1..=5 {
        let mut grid = VoxelGrid::new();
        let mut weapon = WeaponSystem::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(35.0, 10.0, 35.0);

        for &cz in &col_z_coords {
            for &cx in &col_x_coords {
                for dy in 1..=5 {
                    let vt = if dy % 2 == 1 {
                        VoxelType::SteelRebar
                    } else {
                        VoxelType::Concrete
                    };
                    grid.set(cx, dy, cz, vt);
                }
            }
        }

        for x in 41..=53 {
            for z in 41..=52 {
                let vt = if x % 2 == 0 || z % 3 == 0 {
                    VoxelType::SteelRebar
                } else {
                    VoxelType::Concrete
                };
                grid.set(x, 6, z, vt);
            }
        }

        grid.update_meshes();

        for &cz in &col_z_coords {
            for &cx in &col_x_coords {
                let vy = 3;
                let anchor = IVec3::new(cx, vy, cz);
                let pos = grid.voxel_to_world(anchor);
                let placed = weapon.place_c4_charge(pos, Vec3::Y, Some(anchor));
                assert!(placed);
            }
        }
        assert_eq!(weapon.c4_charges.len(), 24);

        let (feedback, det_count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, cam_pos);
        assert_eq!(det_count, 24);
        assert!(feedback.camera_trauma >= 0.75);

        let cap_settle = physics.scratch_chunks_to_settle.capacity();
        let cap_craters = physics.scratch_craters_to_unsettle.capacity();
        let cap_sub_blocks = physics.scratch_sub_blocks.capacity();

        let mut peak_frame_time_ms = 0.0f64;
        let mut total_time_ms = 0.0f64;
        let mut steady_alloc_count = 0;
        let mut steady_alloc_bytes = 0;

        let dt = 0.016f32;
        for frame in 0..30 {
            let is_steady_state = frame >= 15;
            if is_steady_state {
                start_tracking();
            }

            let frame_start = Instant::now();
            weapon.update(dt, &mut grid, &mut physics, &audio, cam_pos);
            physics.update(dt, &mut grid, &audio, cam_pos);
            let islands = grid.detect_unsupported_islands();
            for island in islands {
                physics.spawn_island_chunk(island, Vec3::new(0.0, -1.5, 0.0), Vec3::new(0.2, 0.0, 0.1));
            }
            grid.update_meshes();

            let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            total_time_ms += elapsed_ms;
            if elapsed_ms > peak_frame_time_ms {
                peak_frame_time_ms = elapsed_ms;
            }

            if is_steady_state {
                let (allocs, bytes) = stop_tracking();
                steady_alloc_count += allocs;
                steady_alloc_bytes += bytes;
            }
        }

        let avg_time_ms = total_time_ms / 30.0;
        println!(
            "[STRESS C4 RUN {}/5] Peak: {:.3} ms (<= 16.666ms), Avg: {:.3} ms, Steady Allocs: {} ({} bytes)",
            iteration,
            peak_frame_time_ms,
            avg_time_ms,
            steady_alloc_count,
            steady_alloc_bytes
        );

        let peak_budget = if cfg!(debug_assertions) { 60.0 } else { 16.666 };
        assert!(
            peak_frame_time_ms <= peak_budget,
            "Run {}: peak frame time {:.3}ms exceeded {:.3}ms budget",
            iteration,
            peak_frame_time_ms,
            peak_budget
        );
        assert_eq!(
            steady_alloc_count, 0,
            "Run {}: steady state produced {} allocations ({} bytes)",
            iteration, steady_alloc_count, steady_alloc_bytes
        );
        assert!(physics.scratch_chunks_to_settle.capacity() >= cap_settle);
        assert!(physics.scratch_craters_to_unsettle.capacity() >= cap_craters);
        assert!(physics.scratch_sub_blocks.capacity() >= cap_sub_blocks);
    }
}

// ============================================================================
// 5. Adversarial Stress Test: Scaled 48-Point Foundation Cutting Shock
// ============================================================================

#[test]
fn test_stress_c4_foundation_cutting_48_charges_expanded_grid() {
    let _guard = STRESS_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(35.0, 10.0, 35.0);

    // 8 x 6 = 48 column layout spanning wider building footprint
    let col_x_coords = [38, 40, 42, 44, 46, 48, 50, 52];
    let col_z_coords = [38, 41, 44, 47, 50, 53];

    for &cz in &col_z_coords {
        for &cx in &col_x_coords {
            for dy in 1..=5 {
                let vt = if dy % 2 == 1 {
                    VoxelType::SteelRebar
                } else {
                    VoxelType::CompositeRebar
                };
                grid.set(cx, dy, cz, vt);
            }
        }
    }

    // Heavy reinforced floor slab across all 48 columns
    for x in 37..=53 {
        for z in 37..=54 {
            let vt = if (x + z) % 2 == 0 {
                VoxelType::SteelRebar
            } else {
                VoxelType::Concrete
            };
            grid.set(x, 6, z, vt);
        }
    }
    grid.update_meshes();

    // Wave 1: First 24 columns
    let mut placed_wave1 = 0;
    for &cz in &col_z_coords[0..3] {
        for &cx in &col_x_coords {
            let vy = 3;
            let anchor = IVec3::new(cx, vy, cz);
            let pos = grid.voxel_to_world(anchor);
            if weapon.place_c4_charge(pos, Vec3::Y, Some(anchor)) {
                placed_wave1 += 1;
            }
        }
    }
    assert_eq!(placed_wave1, 24);
    assert_eq!(weapon.c4_charges.len(), 24);

    // Verify architectural limit: attempting 25th charge fails gracefully
    let anchor_overflow = IVec3::new(col_x_coords[0], 3, col_z_coords[3]);
    let pos_overflow = grid.voxel_to_world(anchor_overflow);
    assert!(
        !weapon.place_c4_charge(pos_overflow, Vec3::Y, Some(anchor_overflow)),
        "25th charge must be rejected due to MAX_C4_CHARGES = 24 limit"
    );

    // Detonate Wave 1
    let (feedback1, det_count1) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, cam_pos);
    assert_eq!(det_count1, 24);
    assert!(feedback1.camera_trauma >= 0.75);

    // Wave 2: Second 24 columns on the damaged structure
    let mut placed_wave2 = 0;
    for &cz in &col_z_coords[3..6] {
        for &cx in &col_x_coords {
            let vy = 3;
            let anchor = IVec3::new(cx, vy, cz);
            let pos = grid.voxel_to_world(anchor);
            if weapon.place_c4_charge(pos, Vec3::Y, Some(anchor)) {
                placed_wave2 += 1;
            }
        }
    }
    assert_eq!(placed_wave2, 24);

    // Detonate Wave 2
    let (feedback2, det_count2) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, cam_pos);
    assert_eq!(det_count2, 24);
    assert!(feedback2.camera_trauma >= 0.75);
    assert_eq!(det_count1 + det_count2, 48);

    let mut peak_frame_time_ms = 0.0f64;
    let mut total_time_ms = 0.0f64;
    let mut steady_alloc_count = 0;
    let mut steady_alloc_bytes = 0;

    let dt = 0.016f32;
    for frame in 0..30 {
        let is_steady_state = frame >= 15;
        if is_steady_state {
            start_tracking();
        }

        let frame_start = Instant::now();
        weapon.update(dt, &mut grid, &mut physics, &audio, cam_pos);
        physics.update(dt, &mut grid, &audio, cam_pos);
        let islands = grid.detect_unsupported_islands();
        for island in islands {
            physics.spawn_island_chunk(island, Vec3::new(0.0, -1.5, 0.0), Vec3::new(0.2, 0.0, 0.1));
        }
        grid.update_meshes();

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        total_time_ms += elapsed_ms;
        if elapsed_ms > peak_frame_time_ms {
            peak_frame_time_ms = elapsed_ms;
        }

        if is_steady_state {
            let (allocs, bytes) = stop_tracking();
            steady_alloc_count += allocs;
            steady_alloc_bytes += bytes;
        }
    }

    let avg_time_ms = total_time_ms / 30.0;
    println!(
        "[STRESS SCALED 48 C4] Peak: {:.3} ms (<= 16.666ms), Avg: {:.3} ms, Steady Allocs: {} ({} bytes)",
        peak_frame_time_ms,
        avg_time_ms,
        steady_alloc_count,
        steady_alloc_bytes
    );

    let peak_budget = if cfg!(debug_assertions) { 60.0 } else { 16.666 };
    assert!(
        peak_frame_time_ms <= peak_budget,
        "48 C4 charges: peak frame time {:.3}ms exceeded {:.3}ms budget",
        peak_frame_time_ms,
        peak_budget
    );
    assert_eq!(
        steady_alloc_count, 0,
        "48 C4 charges: steady state produced {} allocations ({} bytes)",
        steady_alloc_count, steady_alloc_bytes
    );
}

// ============================================================================
// 6. Adversarial Stress Test: Dense Multi-Slab Pancake Collapse Pulverization
// ============================================================================

#[test]
fn test_stress_dense_pancake_collapse_six_slabs_scratch_invariance() {
    let _guard = STRESS_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(20.0, 10.0, 20.0);

    // Multi-tier floors at heights 4, 8, 12, 16, 20
    for floor_y in [4, 8, 12, 16, 20] {
        for x in 40..=56 {
            for z in 40..=56 {
                let vt = if x % 3 == 0 || z % 3 == 0 {
                    VoxelType::Concrete
                } else {
                    VoxelType::Glass
                };
                grid.set(x, floor_y, z, vt);
            }
        }
    }

    // Drop 6 consecutive multi-ton reinforced floor slabs
    let slab_center_world = grid.voxel_to_world(IVec3::new(48, 0, 48));
    for story in 0..6 {
        let mut voxels = Vec::new();
        for x in 0..8 {
            for z in 0..8 {
                voxels.push((
                    Vec3::new((x as f32 - 3.5) * VOXEL_SIZE, 0.0, (z as f32 - 3.5) * VOXEL_SIZE),
                    VoxelType::HeavyArmor,
                ));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&voxels);
        physics.chunks.push(DynamicChunk {
            position: Vec3::new(slab_center_world.x, 10.0 + story as f32 * 3.5, slab_center_world.z),
            velocity: Vec3::new(0.0, -16.0, 0.0),
            rotation: Vec3::new(0.01, 0.02, 0.0),
            rot_mat: Mat3::from_euler(Vec3::new(0.01, 0.02, 0.0)),
            angular_velocity: Vec3::new(0.05, 0.0, 0.05),
            voxels,
            exposed_voxels: Vec::new(),
            voxel_health: Vec::new(),
            mass: 5000.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });
    }

    // Prime scratch buckets
    if let Some(chunk) = physics.chunks.first() {
        chunk.populate_sub_blocks(&mut physics.scratch_sub_blocks);
    }

    let initial_cap_settle = physics.scratch_chunks_to_settle.capacity();
    let initial_ptr_settle = physics.scratch_chunks_to_settle.as_ptr();

    let initial_cap_craters = physics.scratch_craters_to_unsettle.capacity();
    let initial_ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();

    let initial_cap_sub_blocks = physics.scratch_sub_blocks.capacity();
    let initial_ptr_sub_blocks = physics.scratch_sub_blocks.as_ptr();

    let dt = 0.016f32;
    let mut total_crushed_mass = 0.0f32;

    for step in 0..90 {
        let prev_mass: f32 = physics.chunks.iter().map(|c| c.mass).sum();

        physics.update(dt, &mut grid, &audio, cam_pos);

        let new_mass: f32 = physics.chunks.iter().map(|c| c.mass).sum();
        if new_mass > prev_mass {
            total_crushed_mass += new_mass - prev_mass;
        }

        assert_eq!(
            physics.scratch_chunks_to_settle.capacity(),
            initial_cap_settle,
            "scratch_chunks_to_settle capacity changed at step {}",
            step
        );
        assert_eq!(
            physics.scratch_chunks_to_settle.as_ptr(),
            initial_ptr_settle,
            "scratch_chunks_to_settle pointer changed at step {}",
            step
        );

        assert_eq!(
            physics.scratch_craters_to_unsettle.capacity(),
            initial_cap_craters,
            "scratch_craters_to_unsettle capacity changed at step {}",
            step
        );
        assert_eq!(
            physics.scratch_craters_to_unsettle.as_ptr(),
            initial_ptr_craters,
            "scratch_craters_to_unsettle pointer changed at step {}",
            step
        );

        assert_eq!(
            physics.scratch_sub_blocks.capacity(),
            initial_cap_sub_blocks,
            "scratch_sub_blocks capacity changed at step {}",
            step
        );
        assert_eq!(
            physics.scratch_sub_blocks.as_ptr(),
            initial_ptr_sub_blocks,
            "scratch_sub_blocks pointer changed at step {}",
            step
        );
    }

    println!(
        "[STRESS 6-SLAB PANCAKE COLLAPSE] Crushed Mass: {:.1} kg, Collapsed Chunks: {}, Zero Reallocations Verified",
        total_crushed_mass,
        physics.total_collapsed_chunks
    );

    assert_eq!(physics.scratch_chunks_to_settle.as_ptr(), initial_ptr_settle);
    assert_eq!(physics.scratch_craters_to_unsettle.as_ptr(), initial_ptr_craters);
    assert_eq!(physics.scratch_sub_blocks.as_ptr(), initial_ptr_sub_blocks);
}
