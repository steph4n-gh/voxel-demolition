use voxel_demolition::voxel::VoxelGrid;
use voxel_demolition::world_gen::{generate_level, LevelPreset};

#[test]
fn test_global_check_no_collapse() {
    let presets = [
        LevelPreset::ApexSkyscraper,
        LevelPreset::SuspensionSkybridge,
        LevelPreset::MilitaryFortress,
        LevelPreset::DominoMonoliths,
        LevelPreset::GothicCitadel,
        LevelPreset::ChemicalRefinery,
    ];
    for preset in presets {
        let mut grid = VoxelGrid::new();
        generate_level(&mut grid, preset);
        
        grid.structural_dirty = true;
        grid.dirty_min = None;
        grid.dirty_max = None;
        
        let islands = grid.detect_unsupported_islands();
        if !islands.is_empty() {
            for (i, isl) in islands.iter().enumerate() {
                let v = isl.voxels[0];
                println!("Island {} size {}: sample at ({}, {}, {})", i, isl.voxels.len(), v.0.x, v.0.y, v.0.z);
            }
        }
        assert_eq!(islands.len(), 0, "Global check should find 0 islands for {:?}, found {}", preset, islands.len());
    }
}
