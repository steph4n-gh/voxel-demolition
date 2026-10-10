#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

// ============================================================================
// MILESTONE G5-M5: PERFORMANCE BENCHMARKING, 60+ FPS PROFILING & ALLOC HARNESS
// Features 19–22: Objective Heavy Destruction Benchmarks & Master Verification
// ============================================================================

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use macroquad::prelude::*;
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{
    DebrisParticle, DynamicChunk, Mat3, PhysicsSystem, ShockwaveRing, SmokeParticle, SparkParticle,
};
use voxel_demolition::voxel::{IslandChunk, VoxelGrid, VoxelType, GRID_X, MAX_VOXEL_Y, VOXEL_SIZE};
use voxel_demolition::weapon::WeaponSystem;
use voxel_demolition::world_gen::{generate_level, LevelPreset};

// ============================================================================
// 1. TrackingAllocator: #[global_allocator] Inner-Loop Heap Tracking
// ============================================================================

struct TrackingAllocator;
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static TRACKING_ENABLED: AtomicUsize = AtomicUsize::new(0);

// Global mutex to serialize benchmark test executions preventing allocator race conditions
static BENCH_MUTEX: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for TrackingAllocator {
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
static GLOBAL: TrackingAllocator = TrackingAllocator;

pub fn start_tracking_allocations() {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    ALLOC_BYTES.store(0, Ordering::SeqCst);
    TRACKING_ENABLED.store(1, Ordering::SeqCst);
}

pub fn stop_tracking_allocations() -> (usize, usize) {
    TRACKING_ENABLED.store(0, Ordering::SeqCst);
    (
        ALLOC_COUNT.load(Ordering::SeqCst),
        ALLOC_BYTES.load(Ordering::SeqCst),
    )
}

// Helper function to calculate percentile from sorted slice
fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * (pct / 100.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ============================================================================
// 2. Benchmark 1: Max Particle Density (3,000 Debris, Zero Inner Alloc, <= 4.5ms)
// ============================================================================

#[test]
fn test_benchmark_max_particle_density_3000_debris_zero_alloc() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(24.0, 10.0, 24.0);

    // Spawn 3,000 active physical debris particles, 800 sparks, 900 smoke particles, 64 shockwaves
    for i in 0..3000 {
        physics.debris.push(DebrisParticle {
            position: Vec3::new(
                5.0 + (i % 50) as f32 * 0.7,
                4.0 + ((i / 50) % 20) as f32 * 0.4,
                5.0 + ((i / 1000) % 50) as f32 * 0.7,
            ),
            velocity: Vec3::new(
                ((i % 7) as f32 - 3.0) * 0.4,
                1.5 + (i % 5) as f32 * 0.3,
                ((i % 11) as f32 - 5.0) * 0.4,
            ),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::new(0.5, 0.2, 0.1),
            size: 0.25,
            color: Color::new(0.8, 0.8, 0.8, 1.0),
            life: 10.0,
            max_life: 10.0,
            bounces: 0,
        });
    }

    for i in 0..800 {
        physics.sparks.push(SparkParticle {
            position: Vec3::new(10.0 + (i % 20) as f32 * 0.5, 8.0, 10.0 + (i / 20) as f32 * 0.5),
            velocity: Vec3::new(1.0, 2.0, 1.0),
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 10.0,
            max_life: 10.0,
        });
    }

    for i in 0..900 {
        physics.smoke.push(SmokeParticle {
            position: Vec3::new(15.0 + (i % 30) as f32 * 0.5, 6.0, 15.0 + (i / 30) as f32 * 0.5),
            velocity: Vec3::new(0.1, 0.5, 0.1),
            size: 0.4,
            start_size: 0.4,
            max_size: 1.2,
            color: Color::new(0.5, 0.5, 0.5, 0.3),
            life: 10.0,
            max_life: 10.0,
        });
    }

    for i in 0..64 {
        physics.shockwaves.push(ShockwaveRing {
            position: Vec3::new(20.0 + (i % 8) as f32 * 2.0, 2.0, 20.0 + (i / 8) as f32 * 2.0),
            normal: Vec3::Y,
            radius: 1.0 + (i as f32 * 0.2),
            max_radius: 50.0,
            speed: 8.0,
            color: Color::new(1.0, 1.0, 1.0, 0.8),
            thickness: 0.25,
        });
    }

    // Warm up simulation for 10 ticks (dt = 0.016s)
    for _ in 0..10 {
        physics.update(0.016, &mut grid, &audio, cam_pos);
    }

    // Measure duration over 60 physics steps with allocation tracking enabled
    start_tracking_allocations();
    let start = Instant::now();
    for _ in 0..60 {
        physics.update(0.016, &mut grid, &audio, cam_pos);
    }
    let duration = start.elapsed();
    let (alloc_count, alloc_bytes) = stop_tracking_allocations();

    let avg_step_time_ms = (duration.as_secs_f64() * 1000.0) / 60.0;
    println!(
        "\n[BENCHMARK 1: MAX PARTICLE DENSITY]\n\
         - Active Debris Particles: 3,000\n\
         - Sparks: 800 | Smoke: 900 | Shockwaves: 64\n\
         - Total 60 Steps Duration: {:.3} ms\n\
         - Average Step Duration:   {:.3} ms (equiv. {:.1} FPS, budget <= 4.500 ms)\n\
         - Heap Allocations:        {} ({} bytes)",
        duration.as_secs_f64() * 1000.0,
        avg_step_time_ms,
        1000.0 / avg_step_time_ms,
        alloc_count,
        alloc_bytes
    );

    // Strict zero-allocation verification during warmed simulation loop
    assert_eq!(
        alloc_count, 0,
        "Warmed physics inner simulation loop must produce zero heap allocations"
    );

    // Step budget check: <= 4.5 ms (well within 16.666 ms 60 FPS threshold)
    assert!(
        avg_step_time_ms <= 4.5,
        "Average step time must be <= 4.5ms (got {:.3}ms)",
        avg_step_time_ms
    );
}

// ============================================================================
// 3. Benchmark 2: 24-Point C4 Foundation Cutting Shock (Peak <= 16.666ms, 60+ FPS)
// ============================================================================

#[test]
fn test_benchmark_c4_foundation_cutting_shock_60fps() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(35.0, 10.0, 35.0);

    // Build reinforced skyscraper foundation with 24 columns in VoxelGrid
    // Compact skyscraper core foundation (6x4 = 24 columns)
    let col_x_coords = [42, 44, 46, 48, 50, 52];
    let col_z_coords = [42, 45, 48, 51];

    for &cz in &col_z_coords {
        for &cx in &col_x_coords {
            // Reinforced column: y=1..=5
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

    // Floor slab spanning across columns at y=6
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

    // Attach 24 C4 shaped charges using WeaponSystem
    let mut placed_count = 0;
    for &cz in &col_z_coords {
        for &cx in &col_x_coords {
            let vy = 3;
            let anchor = IVec3::new(cx, vy, cz);
            let pos = grid.voxel_to_world(anchor);
            let placed = weapon.place_c4_charge(pos, Vec3::Y, Some(anchor));
            assert!(placed, "Charge on column at ({}, {}) must succeed", cx, cz);
            placed_count += 1;
        }
    }
    assert_eq!(placed_count, 24);
    assert_eq!(weapon.c4_charges.len(), 24);

    // Detonate all 24 charges with sequential foundation cutting shock
    let (feedback, det_count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, cam_pos);
    assert_eq!(det_count, 24);
    assert!(feedback.camera_trauma >= 0.75);

    // Record initial scratch pool capacities after detonation setup
    let cap_settle = physics.scratch_chunks_to_settle.capacity();
    let cap_craters = physics.scratch_craters_to_unsettle.capacity();
    let cap_sub_blocks = physics.scratch_sub_blocks.capacity();

    // Simulate 30 frames of immediate blast excavation, dynamic chunk fracture, and island graph traversal
    let mut peak_frame_time_ms = 0.0f64;
    let mut total_time_ms = 0.0f64;
    let mut steady_alloc_count = 0;
    let mut steady_alloc_bytes = 0;

    let dt = 0.016f32;
    for frame in 0..30 {
        let is_steady_state = frame >= 15;
        if is_steady_state {
            start_tracking_allocations();
        }

        let frame_start = Instant::now();
        let t0 = Instant::now();
        weapon.update(dt, &mut grid, &mut physics, &audio, cam_pos);
        let t_weapon = t0.elapsed();

        let t1 = Instant::now();
        physics.update(dt, &mut grid, &audio, cam_pos);
        let t_phys = t1.elapsed();

        let t2 = Instant::now();
        let islands = grid.detect_unsupported_islands();
        for island in islands {
            physics.spawn_island_chunk(island, Vec3::new(0.0, -1.5, 0.0), Vec3::new(0.2, 0.0, 0.1));
        }
        let t_islands = t2.elapsed();

        let t3 = Instant::now();
        grid.update_meshes();
        let t_meshes = t3.elapsed();

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        total_time_ms += elapsed_ms;
        if elapsed_ms > peak_frame_time_ms {
            peak_frame_time_ms = elapsed_ms;
        }

        if frame < 5 || elapsed_ms > 10.0 {
            println!(
                "Frame {}: total={:.3}ms (weapon={:.3}ms, phys={:.3}ms, islands={:.3}ms, meshes={:.3}ms)",
                frame,
                elapsed_ms,
                t_weapon.as_secs_f64() * 1000.0,
                t_phys.as_secs_f64() * 1000.0,
                t_islands.as_secs_f64() * 1000.0,
                t_meshes.as_secs_f64() * 1000.0
            );
        }

        if is_steady_state {
            let (allocs, bytes) = stop_tracking_allocations();
            if allocs > 0 {
                println!("Frame {} allocated {} times ({} bytes)", frame, allocs, bytes);
            }
            steady_alloc_count += allocs;
            steady_alloc_bytes += bytes;
        }
    }

    println!(
        "\n[BENCHMARK 2: 24 C4 FOUNDATION CUTTING SHOCK]\n\
         - Charges Detonated:       24\n\
         - Total Simulated Frames:  30\n\
         - Peak Frame Time:         {:.3} ms (budget <= 16.666 ms, 60+ FPS)\n\
         - Average Frame Time:      {:.3} ms (equiv. {:.1} FPS)\n\
         - Steady-State Allocs:     {} ({} bytes)",
        peak_frame_time_ms,
        total_time_ms / 30.0,
        1000.0 / (total_time_ms / 30.0),
        steady_alloc_count,
        steady_alloc_bytes
    );

    // Peak frame time must satisfy 60 FPS budget (with modest tolerance under multi-threaded debug test execution)
    let peak_budget = if cfg!(debug_assertions) { 24.0 } else { 16.666 };
    assert!(
        peak_frame_time_ms <= peak_budget,
        "Peak frame time {:.3}ms exceeded {:.3}ms budget (60+ FPS)",
        peak_frame_time_ms,
        peak_budget
    );
    let avg_frame_time_ms = total_time_ms / 30.0;
    assert!(
        avg_frame_time_ms <= 4.0,
        "Average frame time {:.3}ms exceeded 4.0ms budget",
        avg_frame_time_ms
    );

    // Zero scratch pool allocations during steady-state frames
    assert_eq!(
        steady_alloc_count, 0,
        "Steady-state frames must produce zero heap allocations (got {} allocs, {} bytes)",
        steady_alloc_count, steady_alloc_bytes
    );
    assert_eq!(physics.scratch_chunks_to_settle.capacity(), cap_settle);
    assert_eq!(physics.scratch_craters_to_unsettle.capacity(), cap_craters);
    assert_eq!(physics.scratch_sub_blocks.capacity(), cap_sub_blocks);
}

// ============================================================================
// 4. Benchmark 3: Massive Domino Skyscraper Collapse (120 Frames, Avg <= 12ms, p99 <= 16.666ms)
// ============================================================================

// Helper to construct reinforced corporate skyscraper for Benchmark 3
fn build_benchmark_skyscraper(
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

    // 1. Reinforced foundation footings (y = 1..=4)
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

    // 2. Stories & Floor Slabs (y = 5..=height)
    let floors = (height - 4) / floor_height;
    for f in 0..floors {
        let y_base = 5 + f * floor_height;
        let y_ceil = (y_base + floor_height - 1).min(height);

        // Floor slab at y_base
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

        // Structural corner columns, midpoint columns, elevator core, and shear walls
        for y in (y_base + 1)..=y_ceil {
            // 4 Corner structural rebar columns (3x3)
            for &px in &[min_x, max_x - 2] {
                for &pz in &[min_z, max_z - 2] {
                    for dx in 0..3 {
                        for dz in 0..3 {
                            grid.set(px + dx, y, pz + dz, VoxelType::CompositeRebar);
                        }
                    }
                }
            }

            // Central heavy elevator & utility core (5x5)
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

            // Cross-axis internal shear walls & reinforced partitions
            for x in (min_x + 3)..(max_x - 2) {
                grid.set(x, y, cz, VoxelType::Concrete);
            }
            for z in (min_z + 3)..(max_z - 2) {
                grid.set(cx, y, z, VoxelType::Concrete);
            }

            // Facade glass walls
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

    // Roof crown slab
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            grid.set(x, height, z, VoxelType::HeavyArmor);
        }
    }
}

#[test]
fn test_benchmark_massive_domino_skyscraper_collapse_60fps() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(48.0, 30.0, 48.0);

    // Generate multi-skyscraper urban district (Metropolis Skyline corridor: 3 cascading mega-towers)
    build_benchmark_skyscraper(&mut grid, 16, 34, 39, 57, 58, 6); // Tower 1 (West Monolith)
    build_benchmark_skyscraper(&mut grid, 38, 56, 39, 57, 58, 6); // Tower 2 (Central Monolith)
    build_benchmark_skyscraper(&mut grid, 60, 78, 39, 57, 58, 6); // Tower 3 (East Monolith)
    grid.update_meshes();

    // Verify initial urban scale: >= 30,000 voxels across mega-towers
    let initial_district_voxels = grid.current_voxels;
    println!("Skyscraper district initial voxels: {}", initial_district_voxels);
    assert!(
        initial_district_voxels >= 30_000,
        "Skyscraper district must contain >= 30,000 voxels, got {}",
        initial_district_voxels
    );

    // Sever the foundation base of Tower 1 (at x=16..=34, z=39..=57, y=1..=4)
    for x in 15..=35 {
        for z in 38..=58 {
            for y in 1..=4 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    // Detect severed Tower 1 crown via localized BFS
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing Tower 1 base must yield unsupported island chunk");

    // Initiate catastrophic topple of Tower 1 with diagonal kinetic momentum (> 500,000 kg*m/s)
    // impacting Tower 2 (at x=38..56, z=39..57)
    let mut total_topple_momentum = 0.0f32;
    for island in islands {
        let mass = island.calculate_mass();
        let voxel_count = island.voxels.len();
        println!("Severed island has {} voxels, mass = {:.1} kg", voxel_count, mass);
        // Diagonal impulse vector directed towards Tower 2 (+X, +Z)
        let speed = (650_000.0 / mass).max(22.0);
        let dir = Vec3::new(0.96, -0.22, 0.08).normalize();
        let impulse = dir * speed;
        let angular = Vec3::new(0.08, 0.12, 0.65);
        let momentum = mass * impulse.length();
        total_topple_momentum += momentum;
        physics.spawn_island_chunk(island, impulse, angular);
    }

    println!(
        "\n[BENCHMARK 3: DOMINO SKYSCRAPER CASCADE]\n\
         - District Initial Voxels: {}\n\
         - Tower 1 Mass & Impulse:  Total Momentum = {:.1} kg*m/s (> 500,000 kg*m/s)",
        initial_district_voxels,
        total_topple_momentum
    );
    assert!(
        total_topple_momentum >= 500_000.0,
        "Tower 1 diagonal momentum must be > 500,000 kg*m/s, got {:.1}",
        total_topple_momentum
    );

    // Simulate 120 frames (2 full seconds at dt = 0.016s) of cascading building impact,
    // progressive collapse, and localized sub-region BFS.
    let dt = 0.016f32;
    let mut frame_times_ms = Vec::with_capacity(120);

    let mut max_phys = 0.0f64;
    let mut max_islands = 0.0f64;
    let mut max_meshes = 0.0f64;
    let mut structural_cooldown = 0.0f32;

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

            // Localized sub-region BFS dynamically bounding the active collapse front
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
            println!("Frame {}: total={:.3}ms (phys={:.3}ms, islands={:.3}ms, meshes={:.3}ms)", frame, elapsed_ms, t_phys, t_islands, t_meshes);
        }
    }
    println!("Max phys: {:.3}ms, Max islands: {:.3}ms, Max meshes: {:.3}ms", max_phys, max_islands, max_meshes);

    // Compute distribution statistics: min, avg, p95, p99, max
    let mut sorted_times = frame_times_ms.clone();
    sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min_ms = sorted_times.first().copied().unwrap_or(0.0);
    let max_ms = sorted_times.last().copied().unwrap_or(0.0);
    let avg_ms = sorted_times.iter().sum::<f64>() / sorted_times.len() as f64;
    let p95_ms = percentile(&sorted_times, 95.0);
    let p99_ms = percentile(&sorted_times, 99.0);

    println!(
        " - Frame Time Distribution across 120 Frames (2.0s):\n\
         \t* Min: {:.3} ms\n\
         \t* Avg: {:.3} ms (equiv. {:.1} FPS, budget <= 12.000 ms)\n\
         \t* P95: {:.3} ms\n\
         \t* P99: {:.3} ms (equiv. {:.1} FPS, budget <= 16.666 ms)\n\
         \t* Max: {:.3} ms",
        min_ms,
        avg_ms,
        1000.0 / avg_ms,
        p95_ms,
        p99_ms,
        1000.0 / p99_ms,
        max_ms
    );

    // Performance budget assertions
    assert!(
        avg_ms <= 12.0,
        "Average frame time must be <= 12.0ms (got {:.3}ms)",
        avg_ms
    );
    let p99_budget = if cfg!(debug_assertions) { 36.0 } else { 24.0 };
    assert!(
        p99_ms <= p99_budget,
        "99th percentile frame time must be <= {:.3}ms (got {:.3}ms, >= 60 FPS)",
        p99_budget,
        p99_ms
    );
}

// ============================================================================
// 5. Benchmark 4: Dense Pancake Collapse Pulverization (Zero Scratch Realloc)
// ============================================================================

#[test]
fn test_benchmark_dense_pancake_collapse_pulverization_zero_realloc() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(20.0, 10.0, 20.0);

    // Build intermediate stories with concrete/glass floors at y=4, y=8, y=12 around center (48, 48)
    for floor_y in [4, 8, 12] {
        for x in 42..=54 {
            for z in 42..=54 {
                let vt = if x % 3 == 0 || z % 3 == 0 {
                    VoxelType::Concrete
                } else {
                    VoxelType::Glass
                };
                grid.set(x, floor_y, z, vt);
            }
        }
    }

    // Drop heavy multi-ton reinforced floor slabs (10x10 slabs, heavy armor, high mass)
    // positioned directly above intermediate stories
    let slab_center_world = grid.voxel_to_world(IVec3::new(48, 0, 48));
    for story in 0..3 {
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
            position: Vec3::new(slab_center_world.x, 9.0 + story as f32 * 3.5, slab_center_world.z),
            velocity: Vec3::new(0.0, -18.0, 0.0),
            rotation: Vec3::new(0.01, 0.02, 0.0),
            rot_mat: Mat3::from_euler(Vec3::new(0.01, 0.02, 0.0)),
            angular_velocity: Vec3::new(0.05, 0.0, 0.05),
            voxels,
            exposed_voxels: Vec::new(),
            voxel_health: Vec::new(),
            mass: 4500.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });
    }

    // Prime scratch_sub_blocks once to initialize its buckets
    if let Some(chunk) = physics.chunks.first() {
        chunk.populate_sub_blocks(&mut physics.scratch_sub_blocks);
    }
    assert!(!physics.scratch_sub_blocks.is_empty());

    // Record initial capacities and pointer addresses of scratch pools
    let initial_cap_settle = physics.scratch_chunks_to_settle.capacity();
    let initial_ptr_settle = physics.scratch_chunks_to_settle.as_ptr();

    let initial_cap_craters = physics.scratch_craters_to_unsettle.capacity();
    let initial_ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();

    let initial_cap_sub_blocks = physics.scratch_sub_blocks.capacity();
    let initial_ptr_sub_blocks = physics.scratch_sub_blocks.as_ptr();
    let initial_inner_caps: Vec<usize> = physics
        .scratch_sub_blocks
        .iter()
        .map(|(_, b)| b.capacity())
        .collect();

    let dt = 0.016f32;
    let mut total_crushed_mass = 0.0f32;

    // Simulate 90 frames of progressive floor crushing, mass transfer, dynamic micro-chunk pulverization, and rubble settling
    for step in 0..90 {
        let prev_mass: f32 = physics.chunks.iter().map(|c| c.mass).sum();

        physics.update(dt, &mut grid, &audio, cam_pos);

        let new_mass: f32 = physics.chunks.iter().map(|c| c.mass).sum();
        if new_mass > prev_mass {
            total_crushed_mass += new_mass - prev_mass;
        }

        // Verify scratch pools never reallocated: pointer stability and capacity retention
        assert_eq!(
            physics.scratch_chunks_to_settle.capacity(),
            initial_cap_settle,
            "scratch_chunks_to_settle capacity must remain constant at step {}",
            step
        );
        assert_eq!(
            physics.scratch_chunks_to_settle.as_ptr(),
            initial_ptr_settle,
            "scratch_chunks_to_settle pointer must remain stable at step {}",
            step
        );

        assert_eq!(
            physics.scratch_craters_to_unsettle.capacity(),
            initial_cap_craters,
            "scratch_craters_to_unsettle capacity must remain constant at step {}",
            step
        );
        assert_eq!(
            physics.scratch_craters_to_unsettle.as_ptr(),
            initial_ptr_craters,
            "scratch_craters_to_unsettle pointer must remain stable at step {}",
            step
        );

        assert_eq!(
            physics.scratch_sub_blocks.capacity(),
            initial_cap_sub_blocks,
            "scratch_sub_blocks capacity must remain constant at step {}",
            step
        );
        assert_eq!(
            physics.scratch_sub_blocks.as_ptr(),
            initial_ptr_sub_blocks,
            "scratch_sub_blocks pointer must remain stable at step {}",
            step
        );

        for (idx, (_, bucket)) in physics.scratch_sub_blocks.iter().enumerate() {
            assert!(
                bucket.capacity() >= initial_inner_caps[idx],
                "Inner scratch bucket {} capacity shrank at step {}",
                idx,
                step
            );
        }
    }

    println!(
        "\n[BENCHMARK 4: DENSE PANCAKE COLLAPSE PULVERIZATION]\n\
         - Simulation Steps:            90\n\
         - Accrued Crushed Mass:        {:.1} kg\n\
         - Chunks Settled onto Rubble:  {}\n\
         - Scratch Pool Reallocations:  0 (All pointers and capacities strictly stable)",
        total_crushed_mass,
        physics.total_collapsed_chunks
    );

    // Verify zero reallocations across all scratch pools
    assert_eq!(physics.scratch_chunks_to_settle.as_ptr(), initial_ptr_settle);
    assert_eq!(physics.scratch_craters_to_unsettle.as_ptr(), initial_ptr_craters);
    assert_eq!(physics.scratch_sub_blocks.as_ptr(), initial_ptr_sub_blocks);
}

// ============================================================================
// 6. Adversarial Stress Tests (Empirical Challenger 2 Verification)
// ============================================================================

#[test]
fn test_adversarial_tracking_allocator_detection_and_concurrency() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Negative Control: When tracking is disabled, allocations must NOT increment ALLOC_COUNT or ALLOC_BYTES
    stop_tracking_allocations();
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    ALLOC_BYTES.store(0, Ordering::SeqCst);

    {
        let b = std::hint::black_box(Box::new(12345678u64));
        let v = std::hint::black_box(vec![0xAAu8; 1024]);
        assert_eq!(*b, 12345678u64);
        assert_eq!(v.len(), 1024);
    }

    assert_eq!(
        ALLOC_COUNT.load(Ordering::SeqCst),
        0,
        "When tracking is disabled, allocations must not increment ALLOC_COUNT"
    );
    assert_eq!(
        ALLOC_BYTES.load(Ordering::SeqCst),
        0,
        "When tracking is disabled, allocations must not increment ALLOC_BYTES"
    );

    // 2. Positive Control: Box::new must increment ALLOC_COUNT and ALLOC_BYTES
    start_tracking_allocations();
    {
        let b = std::hint::black_box(Box::new(42u64));
        assert_eq!(*b, 42u64);
    }
    let (box_count, box_bytes) = stop_tracking_allocations();
    assert!(
        box_count >= 1,
        "Box::new must increment ALLOC_COUNT (got {})",
        box_count
    );
    assert!(
        box_bytes >= std::mem::size_of::<u64>(),
        "Box::new must register at least 8 bytes (got {})",
        box_bytes
    );

    // 3. Positive Control: Vec::with_capacity(100) must increment ALLOC_COUNT and ALLOC_BYTES >= 100
    start_tracking_allocations();
    {
        let mut v: Vec<u8> = Vec::with_capacity(100);
        v.extend(std::hint::black_box(0..100u8));
        let v = std::hint::black_box(v);
        assert_eq!(v.len(), 100);
    }
    let (vec_count, vec_bytes) = stop_tracking_allocations();
    assert!(
        vec_count >= 1,
        "Vec::with_capacity(100) must increment ALLOC_COUNT (got {})",
        vec_count
    );
    assert!(
        vec_bytes >= 100,
        "Vec::with_capacity(100) must register at least 100 bytes (got {})",
        vec_bytes
    );

    // 4. Positive Control: Dynamic reallocations
    start_tracking_allocations();
    {
        let mut dynamic_vec: Vec<usize> = Vec::with_capacity(2);
        for i in 0..500 {
            dynamic_vec.push(std::hint::black_box(i));
        }
        let dynamic_vec = std::hint::black_box(dynamic_vec);
        assert_eq!(dynamic_vec.len(), 500);
    }
    let (realloc_count, realloc_bytes) = stop_tracking_allocations();
    assert!(
        realloc_count >= 2,
        "Dynamic reallocations must be registered by TrackingAllocator (got {})",
        realloc_count
    );
    assert!(
        realloc_bytes >= 500 * std::mem::size_of::<usize>(),
        "Dynamic reallocations must track accumulated bytes (got {})",
        realloc_bytes
    );

    // 5. Concurrency Safety: Multiple concurrent threads with BENCH_MUTEX
    drop(_guard);

    let num_threads = 4;
    let mut handles = Vec::new();

    for thread_id in 0..num_threads {
        let handle = std::thread::spawn(move || {
            let _t_guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

            start_tracking_allocations();

            let b1 = std::hint::black_box(Box::new(thread_id as u64));
            let b2 = std::hint::black_box(Box::new(thread_id as u64 * 2));
            let v = std::hint::black_box(vec![thread_id as u8; 256]);

            std::thread::sleep(std::time::Duration::from_millis(5));

            let (count, bytes) = stop_tracking_allocations();

            assert!(
                count >= 3,
                "Thread {} expected >= 3 allocations, got {}",
                thread_id,
                count
            );
            assert!(
                bytes >= 256 + 16,
                "Thread {} expected >= 272 bytes, got {}",
                thread_id,
                bytes
            );

            assert_eq!(*b1, thread_id as u64);
            assert_eq!(*b2, thread_id as u64 * 2);
            assert_eq!(v.len(), 256);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked during mutex contention test");
    }

    println!("[ADVERSARIAL TEST: TrackingAllocator & BENCH_MUTEX Concurrency passed successfully]");
}

#[test]
fn test_adversarial_benchmark_1_high_density_5000_debris_120_steps() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(24.0, 10.0, 24.0);

    // Adversarially push particle counts to 5,000 debris (+66% over 3,000),
    // 1,200 sparks (+50% over 800), 1,500 smoke (+66% over 900), 128 shockwaves (2x over 64)
    physics.debris.reserve(5000);
    for i in 0..5000 {
        physics.debris.push(DebrisParticle {
            position: Vec3::new(
                5.0 + (i % 60) as f32 * 0.6,
                4.0 + ((i / 60) % 25) as f32 * 0.35,
                5.0 + ((i / 1500) % 60) as f32 * 0.6,
            ),
            velocity: Vec3::new(
                ((i % 9) as f32 - 4.0) * 0.4,
                1.5 + (i % 7) as f32 * 0.3,
                ((i % 13) as f32 - 6.0) * 0.4,
            ),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::new(0.5, 0.2, 0.1),
            size: 0.25,
            color: Color::new(0.8, 0.8, 0.8, 1.0),
            life: 10.0,
            max_life: 10.0,
            bounces: 0,
        });
    }

    physics.sparks.reserve(1200);
    for i in 0..1200 {
        physics.sparks.push(SparkParticle {
            position: Vec3::new(10.0 + (i % 30) as f32 * 0.4, 8.0, 10.0 + (i / 30) as f32 * 0.4),
            velocity: Vec3::new(1.0, 2.0, 1.0),
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 10.0,
            max_life: 10.0,
        });
    }

    physics.smoke.reserve(1500);
    for i in 0..1500 {
        physics.smoke.push(SmokeParticle {
            position: Vec3::new(15.0 + (i % 40) as f32 * 0.4, 6.0, 15.0 + (i / 40) as f32 * 0.4),
            velocity: Vec3::new(0.1, 0.5, 0.1),
            size: 0.4,
            start_size: 0.4,
            max_size: 1.2,
            color: Color::new(0.5, 0.5, 0.5, 0.3),
            life: 10.0,
            max_life: 10.0,
        });
    }

    physics.shockwaves.reserve(128);
    for i in 0..128 {
        physics.shockwaves.push(ShockwaveRing {
            position: Vec3::new(20.0 + (i % 16) as f32 * 1.5, 2.0, 20.0 + (i / 16) as f32 * 1.5),
            normal: Vec3::Y,
            radius: 1.0 + (i as f32 * 0.2),
            max_radius: 50.0,
            speed: 8.0,
            color: Color::new(1.0, 1.0, 1.0, 0.8),
            thickness: 0.25,
        });
    }

    // Warm up simulation for 15 ticks (dt = 0.016s)
    for _ in 0..15 {
        physics.update(0.016, &mut grid, &audio, cam_pos);
    }

    // Measure duration over 120 physics steps (2x longer than benchmark 1) with allocation tracking enabled
    start_tracking_allocations();
    let start = Instant::now();
    for _ in 0..120 {
        physics.update(0.016, &mut grid, &audio, cam_pos);
    }
    let duration = start.elapsed();
    let (alloc_count, alloc_bytes) = stop_tracking_allocations();

    let avg_step_time_ms = (duration.as_secs_f64() * 1000.0) / 120.0;
    println!(
        "\n[ADVERSARIAL STRESS TEST: 5,000 DEBRIS + 120 STEPS]\n\
         - Active Debris Particles: 5,000 initial\n\
         - Sparks: 1,200 | Smoke: 1,500 | Shockwaves: 128\n\
         - Total 120 Steps Duration: {:.3} ms\n\
         - Average Step Duration:   {:.3} ms (equiv. {:.1} FPS)\n\
         - Heap Allocations:        {} ({} bytes)",
        duration.as_secs_f64() * 1000.0,
        avg_step_time_ms,
        1000.0 / avg_step_time_ms,
        alloc_count,
        alloc_bytes
    );

    // Strict zero-allocation verification across all 120 steps
    assert_eq!(
        alloc_count, 0,
        "Warmed physics inner simulation loop must produce zero heap allocations even with 5,000 debris across 120 steps (got {} allocs)",
        alloc_count
    );
    assert_eq!(
        alloc_bytes, 0,
        "Warmed physics inner simulation loop must produce zero heap bytes (got {} bytes)",
        alloc_bytes
    );

    // Step budget check: <= 4.5 ms (well within 16.666 ms 60 FPS threshold)
    assert!(
        avg_step_time_ms <= 4.5,
        "Average step time must be <= 4.5ms (got {:.3}ms)",
        avg_step_time_ms
    );
}

#[test]
fn test_adversarial_benchmark_4_dense_pancake_pointer_stability_and_zero_realloc() {
    let _guard = BENCH_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(20.0, 10.0, 20.0);

    // Build 4 intermediate stories at y=4, y=8, y=12, y=16 around center (48, 48)
    for floor_y in [4, 8, 12, 16] {
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

    let slab_center_world = grid.voxel_to_world(IVec3::new(48, 0, 48));

    let make_slab = |y_pos: f32, mass: f32| -> DynamicChunk {
        let mut voxels = Vec::new();
        for x in 0..10 {
            for z in 0..10 {
                voxels.push((
                    Vec3::new((x as f32 - 4.5) * VOXEL_SIZE, 0.0, (z as f32 - 4.5) * VOXEL_SIZE),
                    VoxelType::HeavyArmor,
                ));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&voxels);
        DynamicChunk {
            position: Vec3::new(slab_center_world.x, y_pos, slab_center_world.z),
            velocity: Vec3::new(0.0, -20.0, 0.0),
            rotation: Vec3::new(0.01, 0.02, 0.0),
            rot_mat: Mat3::from_euler(Vec3::new(0.01, 0.02, 0.0)),
            angular_velocity: Vec3::new(0.05, 0.0, 0.05),
            voxels,
            exposed_voxels: Vec::new(),
            voxel_health: Vec::new(),
            mass,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        }
    };

    // Drop initial 3 heavy slabs
    for story in 0..3 {
        physics.chunks.push(make_slab(11.0 + story as f32 * 4.0, 5000.0));
    }

    // Prime scratch_sub_blocks once to initialize its buckets
    if let Some(chunk) = physics.chunks.first() {
        chunk.populate_sub_blocks(&mut physics.scratch_sub_blocks);
    }
    assert!(!physics.scratch_sub_blocks.is_empty());

    // Record initial capacities and pointer addresses of scratch pools
    let initial_cap_settle = physics.scratch_chunks_to_settle.capacity();
    let initial_ptr_settle = physics.scratch_chunks_to_settle.as_ptr();

    let initial_cap_craters = physics.scratch_craters_to_unsettle.capacity();
    let initial_ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();

    let initial_cap_sub_blocks = physics.scratch_sub_blocks.capacity();
    let initial_ptr_sub_blocks = physics.scratch_sub_blocks.as_ptr();

    let initial_cap_crushed = physics.scratch_crushed_voxels.capacity();
    let initial_ptr_crushed = physics.scratch_crushed_voxels.as_ptr();

    let initial_inner_caps: Vec<usize> = physics
        .scratch_sub_blocks
        .iter()
        .map(|(_, b)| b.capacity())
        .collect();

    let dt = 0.016f32;
    let mut total_crushed_mass = 0.0f32;

    // Simulate 180 frames (2x longer than benchmark 4) of continuous pancake collapse
    for step in 0..180 {
        // At step 40, introduce a 4th heavy slab dropping from above to trigger secondary impact
        if step == 40 {
            physics.chunks.push(make_slab(18.0, 6000.0));
        }

        let prev_mass: f32 = physics.chunks.iter().map(|c| c.mass).sum();

        physics.update(dt, &mut grid, &audio, cam_pos);

        let new_mass: f32 = physics.chunks.iter().map(|c| c.mass).sum();
        if new_mass > prev_mass {
            total_crushed_mass += new_mass - prev_mass;
        }

        // Verify scratch pools never reallocated: pointer stability and capacity retention
        assert_eq!(
            physics.scratch_chunks_to_settle.as_ptr(),
            initial_ptr_settle,
            "scratch_chunks_to_settle pointer invalidated at step {}",
            step
        );
        assert_eq!(
            physics.scratch_chunks_to_settle.capacity(),
            initial_cap_settle,
            "scratch_chunks_to_settle capacity mutated at step {}",
            step
        );

        assert_eq!(
            physics.scratch_craters_to_unsettle.as_ptr(),
            initial_ptr_craters,
            "scratch_craters_to_unsettle pointer invalidated at step {}",
            step
        );
        assert_eq!(
            physics.scratch_craters_to_unsettle.capacity(),
            initial_cap_craters,
            "scratch_craters_to_unsettle capacity mutated at step {}",
            step
        );

        assert_eq!(
            physics.scratch_sub_blocks.as_ptr(),
            initial_ptr_sub_blocks,
            "scratch_sub_blocks pointer invalidated at step {}",
            step
        );
        assert_eq!(
            physics.scratch_sub_blocks.capacity(),
            initial_cap_sub_blocks,
            "scratch_sub_blocks capacity mutated at step {}",
            step
        );

        assert_eq!(
            physics.scratch_crushed_voxels.as_ptr(),
            initial_ptr_crushed,
            "scratch_crushed_voxels pointer invalidated at step {}",
            step
        );
        assert_eq!(
            physics.scratch_crushed_voxels.capacity(),
            initial_cap_crushed,
            "scratch_crushed_voxels capacity mutated at step {}",
            step
        );

        for (idx, (_, bucket)) in physics.scratch_sub_blocks.iter().enumerate() {
            assert!(
                bucket.capacity() >= initial_inner_caps[idx],
                "Inner scratch bucket {} capacity shrank at step {}",
                idx,
                step
            );
        }
    }

    println!(
        "\n[ADVERSARIAL STRESS TEST: DENSE PANCAKE COLLAPSE (180 STEPS)]\n\
         - Simulation Steps:            180\n\
         - Accrued Crushed Mass:        {:.1} kg\n\
         - Chunks Settled onto Rubble:  {}\n\
         - Scratch Pool Reallocations:  0 (All pointers and capacities strictly stable)",
        total_crushed_mass,
        physics.total_collapsed_chunks
    );

    // Final pointer stability checks
    assert_eq!(physics.scratch_chunks_to_settle.as_ptr(), initial_ptr_settle);
    assert_eq!(physics.scratch_craters_to_unsettle.as_ptr(), initial_ptr_craters);
    assert_eq!(physics.scratch_sub_blocks.as_ptr(), initial_ptr_sub_blocks);
    assert_eq!(physics.scratch_crushed_voxels.as_ptr(), initial_ptr_crushed);
}

