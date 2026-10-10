//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G5-M4
//! ADVERSARIAL STRESS-TESTING: RELEASE FOOTPRINT, SYMBOL STRIPPING,
//! PANIC ABORT BEHAVIOR & CRATE ARCHITECTURE
//! Target: tests/g5_m4_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports)]

use std::fs;
use std::path::Path;
use std::process::Command;

use voxel_demolition::customizer::CustomizerUI;
use voxel_demolition::physics::PhysicsSystem;
use voxel_demolition::player::Player;
use voxel_demolition::siege::SiegeSystem;
use voxel_demolition::skyline;
use voxel_demolition::ui::{fit_text_to_width, fit_text_to_width_stack, HUD, StackStr};
use voxel_demolition::voxel::{CarveResult, DamageType, VoxelGrid, VoxelType, VOXEL_SIZE};
use voxel_demolition::weapon::{WeaponMode, WeaponSystem};
use voxel_demolition::world_gen::{generate_level, LevelPreset};

// ============================================================================
// SUITE 1: RELEASE BINARY FOOTPRINT & MARGIN TO CEILING ASSERTIONS
// ============================================================================

const STRICT_SIZE_CEILING_BYTES: u64 = 2_621_440; // 2.5 MB (2.5 * 1024 * 1024)
const EXPECTED_UPPER_BOUND_BYTES: u64 = 1_800_000; // Expected ~1.55 MB with safety headroom

#[test]
fn test_release_binary_footprint_strict_ceiling() {
    let candidate_paths = [
        "target/release/voxel_demolition",
        "target_cli/release/voxel_demolition",
    ];

    let mut found = false;
    for path_str in &candidate_paths {
        let path = Path::new(path_str);
        if path.exists() {
            found = true;
            let metadata = fs::metadata(path).unwrap_or_else(|e| {
                panic!("Failed to query metadata for {}: {:?}", path_str, e);
            });
            let size = metadata.len();

            println!("Inspecting release binary at '{}': {} bytes", path_str, size);

            // Assert strictly below 2.5 MB ceiling
            assert!(
                size < STRICT_SIZE_CEILING_BYTES,
                "Release binary {} ({} bytes) exceeds strict 2.5 MB ceiling ({} bytes)",
                path_str,
                size,
                STRICT_SIZE_CEILING_BYTES
            );

            // Assert reasonable range (< 1.8 MB)
            assert!(
                size <= EXPECTED_UPPER_BOUND_BYTES,
                "Release binary {} ({} bytes) exceeds expected upper bound of {} bytes",
                path_str,
                size,
                EXPECTED_UPPER_BOUND_BYTES
            );

            // Assert non-empty binary
            assert!(
                size > 500_000,
                "Release binary {} ({} bytes) is unexpectedly small (< 500 KB)",
                path_str,
                size
            );

            let margin = STRICT_SIZE_CEILING_BYTES - size;
            println!(
                "Release binary headroom: {} bytes ({:.2} MB under ceiling)",
                margin,
                margin as f64 / (1024.0 * 1024.0)
            );
            assert!(
                margin >= 800_000,
                "Release binary margin ({} bytes) is smaller than required safety margin",
                margin
            );
        }
    }

    assert!(found, "At least one release binary target must exist");
}

// ============================================================================
// SUITE 2: PROFILE CONFIGURATION VERIFICATION (Cargo.toml)
// ============================================================================

#[test]
fn test_release_profile_cargo_toml_specifications() {
    let cargo_toml_path = Path::new("Cargo.toml");
    assert!(cargo_toml_path.exists(), "Cargo.toml must exist at project root");

    let cargo_toml_content = fs::read_to_string(cargo_toml_path)
        .expect("Cargo.toml must be readable");

    let release_profile_start = cargo_toml_content.find("[profile.release]")
        .expect("Cargo.toml must contain [profile.release] section");

    let release_section = &cargo_toml_content[release_profile_start..];

    // Assert key optimization and footprint flags
    assert!(
        release_section.contains("opt-level = 3"),
        "[profile.release] must specify opt-level = 3"
    );
    assert!(
        release_section.contains("lto = true"),
        "[profile.release] must specify lto = true"
    );
    assert!(
        release_section.contains("codegen-units = 1"),
        "[profile.release] must specify codegen-units = 1"
    );
    assert!(
        release_section.contains("panic = \"abort\""),
        "[profile.release] must specify panic = \"abort\""
    );
    assert!(
        release_section.contains("strip = true"),
        "[profile.release] must specify strip = true"
    );
}

// ============================================================================
// SUITE 3: SYMBOL STRIPPING & UNWIND TABLE ADVERSARIAL INSPECTION
// ============================================================================

#[test]
fn test_symbol_stripping_via_nm() {
    let candidate_paths = [
        "target/release/voxel_demolition",
        "target_cli/release/voxel_demolition",
    ];

    for path_str in &candidate_paths {
        let path = Path::new(path_str);
        if !path.exists() {
            continue;
        }

        // Run `nm` to inspect defined global and local symbols
        let output = Command::new("nm")
            .arg(path_str)
            .output();

        if let Ok(cmd_res) = output {
            if cmd_res.status.success() {
                let stdout = String::from_utf8_lossy(&cmd_res.stdout);
                
                // Assert no unstripped Rust mangled symbols (e.g. _ZN or $LT$)
                assert!(
                    !stdout.contains("_ZN"),
                    "Found legacy unstripped mangled Rust symbols (_ZN) in {}",
                    path_str
                );
                assert!(
                    !stdout.contains("$LT$"),
                    "Found unstripped Rust generic symbols ($LT$) in {}",
                    path_str
                );

                // Assert that internal engine module names are not exposed in symbols
                let internal_modules = [
                    "voxel_demolition::voxel",
                    "voxel_demolition::physics",
                    "voxel_demolition::weapon",
                    "voxel_demolition::siege",
                    "voxel_demolition::world_gen",
                ];
                for mod_name in &internal_modules {
                    assert!(
                        !stdout.contains(mod_name),
                        "Found internal module symbol {} in stripped binary {}",
                        mod_name,
                        path_str
                    );
                }

                // Verify that the only defined text symbols are minimal Mach-O / CRT headers
                let defined_symbols: Vec<&str> = stdout
                    .lines()
                    .filter(|l| {
                        let trimmed = l.trim();
                        // Defined symbols typically have uppercase T or D or S
                        (trimmed.contains(" T ") || trimmed.contains(" t ")) &&
                        !trimmed.contains("__mh_execute_header")
                    })
                    .collect();

                println!(
                    "Defined text symbols count in {}: {}",
                    path_str,
                    defined_symbols.len()
                );
                assert!(
                    defined_symbols.is_empty(),
                    "Stripped release binary must have 0 defined non-header text symbols, found: {:?}",
                    defined_symbols
                );
            }
        }
    }
}

// ============================================================================
// SUITE 4: PANIC ABORT & UNWINDING ISOLATION SANITY CHECK
// ============================================================================

#[test]
fn test_panic_isolation_catch_unwind_in_tests() {
    // Verify that the test profile properly retains unwinding so test assertions,
    // catch_unwind, and error boundaries function without killing the test runner.
    let caught = std::panic::catch_unwind(|| {
        panic!("Intentional test panic for unwind verification");
    });

    assert!(caught.is_err(), "std::panic::catch_unwind must intercept panics in test harness");
}

#[test]
fn test_normal_engine_execution_zero_panics() {
    // Verify that regular operations across engine systems do not trigger any panic paths.
    let mut grid = VoxelGrid::new();
    generate_level(&mut grid, LevelPreset::ApexSkyscraper);

    let mut weapon = WeaponSystem::new();
    let mut siege = SiegeSystem::new();
    siege.setup_default_fleet();

    // 1. Verify level generation populated voxels without panic
    assert!(grid.current_voxels > 0);

    // 2. Carve a small sphere without panic
    let mut result = CarveResult::default();
    grid.carve_sphere_advanced_into(
        macroquad::prelude::Vec3::new(0.0, 10.0, 0.0),
        3.0,
        100.0,
        DamageType::Explosive,
        &mut result,
    );

    // 3. Update det cord cache without panic
    weapon.update_det_cord_cache();

    // 4. Test UI formatting without panic
    let mut stack_str = StackStr::<64>::new();
    stack_str.push_str("HUD: ALL SYSTEMS NOMINAL");
    assert_eq!(stack_str.as_str(), "HUD: ALL SYSTEMS NOMINAL");

    let mut hud = HUD::new();
    hud.update(0.016);
    assert_eq!(hud.collapse_notifications.len(), 0);
}

// ============================================================================
// SUITE 5: DUAL-TARGET CRATE ARCHITECTURE ACCESSIBILITY
// ============================================================================

#[test]
fn test_dual_target_library_crate_modules_accessible() {
    // Explicitly verify accessibility of all 10 modules from voxel_demolition library crate
    use voxel_demolition::audio;
    use voxel_demolition::customizer;
    use voxel_demolition::physics;
    use voxel_demolition::player;
    use voxel_demolition::siege;
    use voxel_demolition::skyline;
    use voxel_demolition::ui;
    use voxel_demolition::voxel;
    use voxel_demolition::weapon;
    use voxel_demolition::world_gen;

    let grid = voxel::VoxelGrid::new();
    assert_eq!(grid.dirty_chunks.len(), 0);

    let config = siege::SiegeWeaponConfig::default();
    let cloned_config = config; // Verifies Copy derivation
    assert_eq!(config.engine_kind.name(), cloned_config.engine_kind.name());

    let customizer = customizer::CustomizerUI::new();
    assert!(!customizer.is_open);

    let mut hud = ui::HUD::new();
    hud.add_notification("TEST NOTIFICATION");
    assert_eq!(hud.collapse_notifications.len(), 1);
}
