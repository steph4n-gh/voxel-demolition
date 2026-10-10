use macroquad::prelude::*;
use crate::voxel::VoxelGrid;
use crate::physics::PhysicsSystem;
use crate::world_gen::LevelPreset;

/// Demolition Contractor Performance Grade
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractGrade {
    S, // Master Demolitionist: Flawless execution, zero excess collateral
    A, // Superior Demolition: Clean drop, well under budget
    B, // Standard Demolition: Objectives met within tolerances
    C, // Substandard Demolition: High collateral or excessive budget
    F, // Failed: Objectives incomplete or bounds violated
}

impl ContractGrade {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContractGrade::S => "S (MASTER DEMOLITIONIST)",
            ContractGrade::A => "A (SUPERIOR CONTRACTOR)",
            ContractGrade::B => "B (STANDARD COMPLETION)",
            ContractGrade::C => "C (MARGINAL PASS)",
            ContractGrade::F => "F (CONTRACT TERMINATED)",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            ContractGrade::S => Color::new(1.0, 0.84, 0.0, 1.0),   // Gold
            ContractGrade::A => Color::new(0.3, 0.9, 0.3, 1.0),    // Emerald
            ContractGrade::B => Color::new(0.2, 0.7, 1.0, 1.0),    // Cyan
            ContractGrade::C => Color::new(1.0, 0.65, 0.1, 1.0),   // Amber
            ContractGrade::F => Color::new(1.0, 0.2, 0.2, 1.0),    // Crimson
        }
    }
}

/// Precision Drop Zone Target bounds
#[derive(Debug, Clone, Copy)]
pub struct DropZone {
    pub min: Vec3,
    pub max: Vec3,
    pub target_centroid: Vec3,
    pub radius: f32,
}

impl DropZone {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        let target_centroid = (min + max) * 0.5;
        let radius = (max - min).length() * 0.5;
        Self {
            min,
            max,
            target_centroid,
            radius,
        }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }
}

/// Contract Status Lifecycle
#[derive(Debug, Clone, PartialEq)]
pub enum ContractStatus {
    Ready,
    Active,
    Passed(ContractGrade),
    Failed(&'static str),
}

/// Demolition Contract Specification
#[derive(Debug, Clone)]
pub struct DemolitionContract {
    pub id: usize,
    pub title: &'static str,
    pub briefing: &'static str,
    pub target_level: LevelPreset,
    pub target_destruction_ratio: f32, // Required destruction of target mass (e.g. 0.85 = 85%)
    pub max_collateral_budget: f32,    // Maximum dollar penalty for collateral damage (e.g. $100,000)
    pub max_ordnance_budget: f32,      // Maximum allowed dollar spend on ordnance (e.g. $150,000)
    pub max_c4_charges: usize,         // Optional strict charge limit (e.g. 4 for Contract 2)
    pub time_limit_seconds: f32,       // Optional time cap (0.0 = unlimited)
    pub drop_zone: Option<DropZone>,   // Target rubble landing zone
}

/// Demolition Contractor Campaign State Manager
pub struct CampaignManager {
    pub contracts: Vec<DemolitionContract>,
    pub active_contract_idx: usize,
    pub status: ContractStatus,
    pub elapsed_time: f32,
    pub collateral_damage_dollars: f32,
    pub ordnance_dollars_spent: f32,
    pub c4_charges_used: usize,
    pub rubble_in_zone_ratio: f32,
    pub initial_target_voxels: usize,
    pub current_destruction_ratio: f32,
}

impl CampaignManager {
    pub fn new() -> Self {
        let contracts = vec![
            DemolitionContract {
                id: 1,
                title: "CONTRACT 01: CORPORATE DROP",
                briefing: "Demolish at least 70% of the corporate mega-block within 120 seconds. Keep settled rubble in the marked plaza and spill penalties below $100k.",
                target_level: LevelPreset::ApexSkyscraper,
                target_destruction_ratio: 0.70,
                max_collateral_budget: 100_000.0,
                max_ordnance_budget: 300_000.0,
                max_c4_charges: 12,
                time_limit_seconds: 120.0,
                drop_zone: Some(DropZone::new(Vec3::new(-20.0, 0.0, -20.0), Vec3::new(20.0, 40.0, 20.0))),
            },
            DemolitionContract {
                id: 2,
                title: "CONTRACT 02: BRIDGE SEVERANCE",
                briefing: "Demolish at least 75% of the industrial fusion district within 90 seconds and a $50k ordnance budget. At most 4 C4 charges may be planted.",
                target_level: LevelPreset::SuspensionSkybridge,
                target_destruction_ratio: 0.75,
                max_collateral_budget: 50_000.0,
                max_ordnance_budget: 50_000.0,
                max_c4_charges: 4,
                time_limit_seconds: 90.0,
                drop_zone: None,
            },
            DemolitionContract {
                id: 3,
                title: "CONTRACT 03: FUEL CHAIN REACTION",
                briefing: "Demolish at least 85% of the chemical refinery, including its foundations, within 20 seconds. Fuel silos and conduits can trigger chain reactions.",
                target_level: LevelPreset::ChemicalRefinery,
                target_destruction_ratio: 0.85,
                max_collateral_budget: 150_000.0,
                max_ordnance_budget: 100_000.0,
                max_c4_charges: 8,
                time_limit_seconds: 20.0,
                drop_zone: None,
            },
            DemolitionContract {
                id: 4,
                title: "CONTRACT 04: FORTRESS SIEGE",
                briefing: "Demolish at least 80% of the fortress citadel within 180 seconds and a $150k ordnance budget. Breach the armored bastions and central spire foundations.",
                target_level: LevelPreset::DominoMonoliths,
                target_destruction_ratio: 0.80,
                max_collateral_budget: 120_000.0,
                max_ordnance_budget: 150_000.0,
                max_c4_charges: 10,
                time_limit_seconds: 180.0,
                drop_zone: None,
            },
            DemolitionContract {
                id: 5,
                title: "CONTRACT 05: TOKAMAK CONTAINMENT BREACH",
                briefing: "Demolish at least 82% of the tokamak facility, including its foundations, within 150 seconds. Cooling rings, capacitors, and the plasma core are volatile targets.",
                target_level: LevelPreset::QuantumFusionReactor,
                target_destruction_ratio: 0.82,
                max_collateral_budget: 200_000.0,
                max_ordnance_budget: 250_000.0,
                max_c4_charges: 12,
                time_limit_seconds: 150.0,
                drop_zone: None,
            },
            DemolitionContract {
                id: 6,
                title: "CONTRACT 06: DOMINO PERFECTION",
                briefing: "Demolish at least 78% of the eight-spire neon metropolis within 60 seconds and a $35k ordnance budget. Exploit weak foundation hinges to initiate collapses.",
                target_level: LevelPreset::GothicCitadel,
                target_destruction_ratio: 0.78,
                max_collateral_budget: 80_000.0,
                max_ordnance_budget: 35_000.0,
                max_c4_charges: 2,
                time_limit_seconds: 60.0,
                drop_zone: None,
            },
            DemolitionContract {
                id: 7,
                title: "CONTRACT 07: AEROPOLIS FOUNDATION BREACH",
                briefing: "Demolish at least 65% of the aeropolis within 240 seconds. Foundation strikes and subterranean silo breaches can bring down the communications towers.",
                target_level: LevelPreset::MilitaryFortress,
                target_destruction_ratio: 0.65,
                max_collateral_budget: 180_000.0,
                max_ordnance_budget: 400_000.0,
                max_c4_charges: 16,
                time_limit_seconds: 240.0,
                drop_zone: None,
            },
            DemolitionContract {
                id: 8,
                title: "CONTRACT 08: NEO-TOKYO PRECISION DROP",
                briefing: "Demolish at least 75% of the twin cyber-spire district within 300 seconds. Keep settled rubble in the marked drop zone and spill penalties below $250k.",
                target_level: LevelPreset::NeoTokyoSpire,
                target_destruction_ratio: 0.75,
                max_collateral_budget: 250_000.0,
                max_ordnance_budget: 500_000.0,
                max_c4_charges: 24,
                time_limit_seconds: 300.0,
                drop_zone: Some(DropZone::new(Vec3::new(-35.0, 0.0, -35.0), Vec3::new(35.0, 60.0, 35.0))),
            },
        ];

        Self {
            contracts,
            active_contract_idx: 0,
            status: ContractStatus::Ready,
            elapsed_time: 0.0,
            collateral_damage_dollars: 0.0,
            ordnance_dollars_spent: 0.0,
            c4_charges_used: 0,
            rubble_in_zone_ratio: 1.0,
            initial_target_voxels: 1,
            current_destruction_ratio: 0.0,
        }
    }

    pub fn active_contract(&self) -> &DemolitionContract {
        &self.contracts[self.active_contract_idx]
    }

    pub fn select_contract(&mut self, idx: usize) {
        if idx < self.contracts.len() {
            self.active_contract_idx = idx;
            self.reset_contract_state();
        }
    }

    pub fn next_contract(&mut self) -> usize {
        self.active_contract_idx = (self.active_contract_idx + 1) % self.contracts.len();
        self.reset_contract_state();
        self.active_contract_idx
    }

    pub fn prev_contract(&mut self) -> usize {
        self.active_contract_idx = if self.active_contract_idx == 0 {
            self.contracts.len() - 1
        } else {
            self.active_contract_idx - 1
        };
        self.reset_contract_state();
        self.active_contract_idx
    }

    pub fn reset_contract_state(&mut self) {
        self.status = ContractStatus::Ready;
        self.elapsed_time = 0.0;
        self.collateral_damage_dollars = 0.0;
        self.ordnance_dollars_spent = 0.0;
        self.c4_charges_used = 0;
        self.rubble_in_zone_ratio = 1.0;
        self.current_destruction_ratio = 0.0;
    }

    pub fn start_contract(&mut self, initial_voxels: usize) {
        self.reset_contract_state();
        self.initial_target_voxels = initial_voxels.max(1);
        self.status = ContractStatus::Active;
    }

    pub fn record_ordnance_spend(&mut self, dollars: f32) {
        if self.status == ContractStatus::Active {
            self.ordnance_dollars_spent += dollars;
        }
    }

    pub fn record_c4_placement(&mut self) {
        if self.status == ContractStatus::Active {
            self.c4_charges_used += 1;
            self.ordnance_dollars_spent += 2_500.0;
        }
    }

    /// Evaluates current contract progress, calculating precision drop zone settlement and collateral damage
    pub fn update(&mut self, dt: f32, grid: &VoxelGrid, physics: &PhysicsSystem) {
        if self.status != ContractStatus::Active {
            return;
        }

        self.elapsed_time += dt.max(0.0);
        let contract = &self.contracts[self.active_contract_idx];

        // 1. Time Limit Failure Check
        if contract.time_limit_seconds > 0.0 && self.elapsed_time > contract.time_limit_seconds {
            self.status = ContractStatus::Failed("TIME LIMIT EXCEEDED");
            return;
        }

        // 2. C4 Cap Failure Check
        if contract.max_c4_charges > 0 && self.c4_charges_used > contract.max_c4_charges {
            self.status = ContractStatus::Failed("C4 CHARGE ALLOCATION EXCEEDED");
            return;
        }

        // 3. Ordnance Budget Failure Check
        if self.ordnance_dollars_spent > contract.max_ordnance_budget {
            self.status = ContractStatus::Failed("ORDNANCE BUDGET EXHAUSTED");
            return;
        }

        // 4. Calculate Target Destruction Ratio
        let current_solids = grid.solid_count();
        let destroyed = self.initial_target_voxels.saturating_sub(current_solids);
        self.current_destruction_ratio = (destroyed as f32 / self.initial_target_voxels as f32).clamp(0.0, 1.0);

        // 5. Evaluate settled rubble. Airborne chunks can still land inside the zone.
        let mut rubble_still_falling = false;
        if let Some(drop_zone) = &contract.drop_zone {
            let mut total_chunk_voxels = 0;
            let mut in_zone_voxels = 0;
            let mut out_zone_voxels = 0;

            for chunk in &physics.chunks {
                if !chunk.settled && !chunk.voxels.is_empty() {
                    rubble_still_falling = true;
                    continue;
                }
                let chunk_size = chunk.voxels.len();
                total_chunk_voxels += chunk_size;
                let (min, max) = chunk.world_aabb();
                if drop_zone.contains(min) && drop_zone.contains(max) {
                    in_zone_voxels += chunk_size;
                } else if max.x < drop_zone.min.x || min.x > drop_zone.max.x
                    || max.y < drop_zone.min.y || min.y > drop_zone.max.y
                    || max.z < drop_zone.min.z || min.z > drop_zone.max.z {
                    out_zone_voxels += chunk_size;
                } else {
                    // Only boundary chunks need a per-voxel check; their centroid can hide spills.
                    for (local_pos, _) in &chunk.voxels {
                        if drop_zone.contains(chunk.local_to_world(*local_pos)) {
                            in_zone_voxels += 1;
                        } else {
                            out_zone_voxels += 1;
                        }
                    }
                }
            }

            if total_chunk_voxels > 0 {
                self.rubble_in_zone_ratio = in_zone_voxels as f32 / total_chunk_voxels as f32;
                // Penalize $100 per rubble voxel spilled outside drop zone
                self.collateral_damage_dollars = self.collateral_damage_dollars.max(out_zone_voxels as f32 * 100.0);
            } else {
                self.rubble_in_zone_ratio = 1.0;
            }

            if self.collateral_damage_dollars > contract.max_collateral_budget {
                self.status = ContractStatus::Failed("COLLATERAL DAMAGE EXCEEDED PERMISSIBLE CEILING");
                return;
            }
        }

        // 6. Check Completion
        if self.current_destruction_ratio >= contract.target_destruction_ratio && !rubble_still_falling {
            let grade = self.calculate_final_grade(contract);
            self.status = ContractStatus::Passed(grade);
        }
    }

    /// Evaluates final performance grade S, A, B, C, F
    pub fn calculate_final_grade(&self, contract: &DemolitionContract) -> ContractGrade {
        // Disqualification checks
        if self.current_destruction_ratio < contract.target_destruction_ratio {
            return ContractGrade::F;
        }
        if self.collateral_damage_dollars > contract.max_collateral_budget {
            return ContractGrade::F;
        }
        if self.ordnance_dollars_spent > contract.max_ordnance_budget {
            return ContractGrade::F;
        }
        if contract.max_c4_charges > 0 && self.c4_charges_used > contract.max_c4_charges {
            return ContractGrade::F;
        }
        if contract.time_limit_seconds > 0.0 && self.elapsed_time > contract.time_limit_seconds {
            return ContractGrade::F;
        }

        let collateral_ratio = self.collateral_damage_dollars / contract.max_collateral_budget.max(1.0);
        let budget_ratio = self.ordnance_dollars_spent / contract.max_ordnance_budget.max(1.0);

        if collateral_ratio <= 0.15 && budget_ratio <= 0.50 && self.rubble_in_zone_ratio >= 0.90 {
            ContractGrade::S
        } else if collateral_ratio <= 0.40 && budget_ratio <= 0.75 {
            ContractGrade::A
        } else if collateral_ratio <= 0.70 && budget_ratio <= 0.90 {
            ContractGrade::B
        } else {
            ContractGrade::C
        }
    }

    /// Screen rectangle `(x, y, w, h)` of the contract card.
    ///
    /// Uses the same scale as the HUD and sits on the right edge below the top visor *and* the
    /// stress-scanner strip (which spans the visor width), so it never overlaps either.
    pub fn contract_card_rect(sw: f32, sh: f32) -> (f32, f32, f32, f32) {
        let s = crate::ui::compute_ui_scale(sw, sh);
        let card_w = (sw * 0.28).clamp(280.0 * s, 400.0 * s);
        let card_h = 140.0 * s;
        let card_x = sw - card_w - 16.0 * s;
        // Visor: 10s + 58s. Scanner strip: +4s gap, 24s tall. Then an 8s margin.
        let card_y = (10.0 + 58.0 + 4.0 + 24.0 + 8.0) * s;
        (card_x, card_y, card_w, card_h)
    }

    /// Renders Demolition Contractor Objective Card HUD overlay
    pub fn draw_contract_card(&self) {
        use crate::ui::fit_text_to_width;

        let sw = screen_width();
        let sh = screen_height();
        let s = crate::ui::compute_ui_scale(sw, sh);
        let (card_x, card_y, card_w, card_h) = Self::contract_card_rect(sw, sh);
        let text_w = card_w - 20.0 * s;

        let contract = self.active_contract();
        let border_c = match &self.status {
            ContractStatus::Passed(grade) => grade.color(),
            ContractStatus::Failed(_) => Color::new(1.0, 0.2, 0.2, 1.0),
            _ => Color::new(0.2, 0.8, 1.0, 1.0),
        };

        draw_rectangle(card_x, card_y, card_w, card_h, Color::new(0.06, 0.08, 0.12, 0.92));
        draw_rectangle_lines(card_x, card_y, card_w, card_h, 1.5 * s, border_c);
        draw_rectangle(card_x, card_y, card_w, 3.0 * s, border_c);

        let status_lbl = match &self.status {
            ContractStatus::Active => "[ACTIVE]",
            ContractStatus::Passed(g) => g.as_str(),
            ContractStatus::Failed(reason) => reason,
            ContractStatus::Ready => "[READY]",
        };
        let title_text = format!("[{}/8] {} {}", self.active_contract_idx + 1, contract.title, status_lbl);
        let title_fit = fit_text_to_width(&title_text, text_w, 11.5 * s);
        draw_text(&title_fit, card_x + 10.0 * s, card_y + 16.0 * s, 11.5 * s, border_c);

        let pct = (self.current_destruction_ratio * 100.0).clamp(0.0, 100.0);
        let target_pct = contract.target_destruction_ratio * 100.0;
        let p_text = format!("DESTRUCTION: {:4.1}% / {:4.1}%", pct, target_pct);
        draw_text(&p_text, card_x + 10.0 * s, card_y + 36.0 * s, 11.0 * s, WHITE);

        let pbar_x = card_x + 10.0 * s;
        let pbar_y = card_y + 42.0 * s;
        let pbar_w = card_w - 20.0 * s;
        let pbar_h = 6.0 * s;
        draw_rectangle(pbar_x, pbar_y, pbar_w, pbar_h, Color::new(0.12, 0.14, 0.18, 1.0));
        let fill = (pct / target_pct.max(1.0)).clamp(0.0, 1.0);
        draw_rectangle(pbar_x, pbar_y, pbar_w * fill, pbar_h, border_c);

        let coll_text = format!("COLLATERAL: ${:.0} / ${:.0}", self.collateral_damage_dollars, contract.max_collateral_budget);
        let coll_col = if self.collateral_damage_dollars > contract.max_collateral_budget {
            Color::new(1.0, 0.25, 0.2, 1.0)
        } else {
            Color::new(0.85, 0.85, 0.85, 1.0)
        };
        draw_text(&coll_text, card_x + 10.0 * s, card_y + 64.0 * s, 10.5 * s, coll_col);

        let ord_text = format!("ORDNANCE: ${:.0} / ${:.0}", self.ordnance_dollars_spent, contract.max_ordnance_budget);
        draw_text(&ord_text, card_x + 10.0 * s, card_y + 78.0 * s, 10.5 * s, Color::new(0.7, 0.8, 0.9, 1.0));

        let zone_text = format!("DROP ZONE ACCURACY: {:3.0}%", self.rubble_in_zone_ratio * 100.0);
        draw_text(&zone_text, card_x + 10.0 * s, card_y + 92.0 * s, 10.5 * s, Color::new(0.4, 0.9, 0.5, 1.0));

        let remaining = (contract.time_limit_seconds - self.elapsed_time).max(0.0);
        let time_text = if contract.time_limit_seconds > 0.0 {
            format!("TIME LEFT: {:.1}s | C4: {}/{}", remaining, self.c4_charges_used, contract.max_c4_charges)
        } else {
            format!("TIME: UNLIMITED | C4: {}/{}", self.c4_charges_used, contract.max_c4_charges)
        };
        let time_fit = fit_text_to_width(&time_text, text_w, 10.5 * s);
        let time_color = if contract.time_limit_seconds > 0.0 && remaining <= 10.0 { RED } else { WHITE };
        draw_text(&time_fit, card_x + 10.0 * s, card_y + 108.0 * s, 10.5 * s, time_color);

        // Hotkey guidance row, built from the live binding table.
        let hint_text = Self::hint_text();
        let hint_fit = fit_text_to_width(&hint_text, text_w, 10.0 * s);
        draw_text(&hint_fit, card_x + 10.0 * s, card_y + 130.0 * s, 10.0 * s, Color::new(0.35, 0.85, 1.0, 0.95));
    }

    /// Contract card key hint, e.g. `[ENTER] START/RETRY | [N] NEXT [P] PREV | [C] SANDBOX`.
    pub fn hint_text() -> String {
        use crate::controls;
        format!(
            "{} START/RETRY  |  {} NEXT  {} PREV  |  {} SANDBOX",
            controls::CONTRACT_START.token,
            controls::CONTRACT_NEXT.token,
            controls::CONTRACT_PREV.token,
            controls::CAMPAIGN.token
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detach_unsupported(grid: &mut VoxelGrid, physics: &mut PhysicsSystem) {
        if grid.structural_dirty {
            let islands = grid.detect_unsupported_islands();
            if !islands.is_empty() { grid.structural_dirty = true; }
            for island in islands {
                let radius = ((island.voxels.len() as f32).cbrt() * crate::voxel::VOXEL_SIZE).max(0.5);
                let scale = (0.5 / radius).min(1.0);
                let impulse = Vec3::new(rand::gen_range(-1.5, 1.5) * scale, rand::gen_range(-2.5, -1.0), rand::gen_range(-1.5, 1.5) * scale);
                let spin = Vec3::new(rand::gen_range(-1.2, 1.2), rand::gen_range(-0.6, 0.6), rand::gen_range(-1.2, 1.2)) * scale;
                physics.spawn_island_chunk(island, impulse, spin);
            }
        }
    }

    // Actual cooldown-limited shots from the generated spawn, structural detachments,
    // physics, and campaign accounting. The seed makes qualification runs reproducible.
    fn play_generated_contract(idx: usize) -> CampaignManager {
        use crate::audio::SoundSystem;
        use crate::weapon::{WeaponSystem, WeaponMode};
        let audio = SoundSystem::empty();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let mut mgr = CampaignManager::new();
        macroquad::rand::srand(0xB04A + idx as u64);
        mgr.select_contract(idx);
        let preset = mgr.active_contract().target_level;
        let layout = crate::world_gen::generate_level(&mut grid, preset);
        physics.clear();
        let camera = layout.spawn_point(&grid) + Vec3::Y * 1.75;
        let mut weapon = WeaponSystem::new();
        let precision = mgr.active_contract().drop_zone.is_some();
        weapon.config.mode = if precision { WeaponMode::ApexRailgun } else { WeaponMode::EventHorizon };
        // Legal maximum radius reaches the refinery and fortress objectives.
        if idx == 2 || idx == 3 { weapon.config.blast_radius_mult = 3.0; }
        mgr.start_contract(grid.solid_count());
        let initial = grid.solid_count();
        let mut shots = 0;
        for z in [24, 48, 72, 12, 84] {
            for x in [48, 24, 72, 12, 84] {
                if mgr.status != ContractStatus::Active { break; }
                let target = layout.design_to_world(&grid, x, 4, z);
                let direction = (target - camera).normalize();
                if !precision {
                    let cooldown = weapon.fire_cooldown;
                    weapon.fire_primary(camera + direction * 0.5, direction, camera, &mut grid, &mut physics, &audio);
                    assert!(weapon.fire_cooldown > cooldown, "A shot following its cooldown must fire");
                    mgr.record_ordnance_spend(500.0);
                } else {
                    let (_, notification) = weapon.fire_alt(camera + direction * 0.5, direction, camera, &mut grid, &mut physics, &audio);
                    assert!(!notification.is_empty(), "A shot following its cooldown must fire");
                    mgr.record_ordnance_spend(75.0);
                }
                shots += 1;
                for _ in 0..24 {
                    physics.update(0.05, &mut grid, &audio, camera);
                    weapon.update(0.05, &mut grid, &mut physics, &audio, camera);
                    detach_unsupported(&mut grid, &mut physics);
                    mgr.update(0.05, &grid, &physics);
                }
            }
        }
        let remaining_frames = ((mgr.active_contract().time_limit_seconds - mgr.elapsed_time).max(0.0) / 0.05).ceil() as usize + 1;
        for _ in 0..remaining_frames {
            if mgr.status != ContractStatus::Active { break; }
            physics.update(0.05, &mut grid, &audio, camera);
            weapon.update(0.05, &mut grid, &mut physics, &audio, camera);
            detach_unsupported(&mut grid, &mut physics);
            mgr.update(0.05, &grid, &physics);
        }
        let falling = physics.chunks.iter().filter(|c| !c.settled).count();
        println!("contract={} level={:?} initial={} shots={} progress={:.1}% elapsed={:.1}s spend=${:.0} collateral=${:.0} chunks={} falling={} status={:?}",
            idx + 1, preset, initial, shots, mgr.current_destruction_ratio * 100.0,
            mgr.elapsed_time, mgr.ordnance_dollars_spent, mgr.collateral_damage_dollars,
            physics.chunks.len(), falling, mgr.status);
        if idx == 7 {
            for chunk in physics.chunks.iter().filter(|c| !c.settled) {
                println!("remaining airborne voxels={} position={:?} velocity={:?} spin={:?} lowest_y={:.3}", chunk.voxels.len(), chunk.position, chunk.velocity, chunk.angular_velocity, chunk.lowest_world_y());
            }
        }
        mgr
    }

    #[test]
    #[ignore = "bounded campaign gameplay qualification; run explicitly"]
    fn campaign_reachability_probe() {
        for idx in 0..CampaignManager::new().contracts.len() {
            play_generated_contract(idx);
        }
    }

    #[test]
    #[ignore = "complete generated refinery gameplay qualification; run explicitly"]
    fn refinery_contract_win_and_retry() {
        let mut mgr = play_generated_contract(2);
        assert!(matches!(mgr.status, ContractStatus::Passed(_)), "The refinery is reachable using live, cooldown-limited weapons");
        assert!(mgr.current_destruction_ratio >= mgr.active_contract().target_destruction_ratio);
        assert!(mgr.elapsed_time <= mgr.active_contract().time_limit_seconds);
        assert!(mgr.ordnance_dollars_spent <= mgr.active_contract().max_ordnance_budget);
        assert_eq!(mgr.next_contract(), 3);
        assert_eq!(mgr.status, ContractStatus::Ready);
        assert_eq!(mgr.ordnance_dollars_spent, 0.0);
        let mut grid = VoxelGrid::new();
        crate::world_gen::generate_level(&mut grid, mgr.active_contract().target_level);
        mgr.start_contract(grid.solid_count());
        mgr.update(mgr.active_contract().time_limit_seconds + 0.1, &grid, &PhysicsSystem::new());
        assert_eq!(mgr.status, ContractStatus::Failed("TIME LIMIT EXCEEDED"));
        mgr.start_contract(grid.solid_count());
        assert_eq!(mgr.status, ContractStatus::Active);
        assert_eq!(mgr.elapsed_time, 0.0);
    }

    #[test]
    #[ignore = "complete generated contract gameplay qualification; run explicitly"]
    fn neo_tokyo_contract_win_and_campaign_progression() {
        let mut mgr = play_generated_contract(7);
        assert!(matches!(mgr.status, ContractStatus::Passed(_)), "A complete precision-drop run must finish after real rubble settlement");
        assert!(mgr.current_destruction_ratio >= mgr.active_contract().target_destruction_ratio);
        assert!(mgr.elapsed_time <= mgr.active_contract().time_limit_seconds);
        assert!(mgr.ordnance_dollars_spent <= mgr.active_contract().max_ordnance_budget);
        assert!(mgr.collateral_damage_dollars <= mgr.active_contract().max_collateral_budget);
        let spend = mgr.ordnance_dollars_spent;
        mgr.record_ordnance_spend(75.0);
        assert_eq!(mgr.ordnance_dollars_spent, spend, "Completed results remain frozen");
        assert_eq!(mgr.next_contract(), 0);
        assert_eq!(mgr.status, ContractStatus::Ready);
        assert_eq!(mgr.ordnance_dollars_spent, 0.0);
        assert_eq!(mgr.c4_charges_used, 0);
        let mut grid = VoxelGrid::new();
        crate::world_gen::generate_level(&mut grid, mgr.active_contract().target_level);
        mgr.start_contract(grid.solid_count());
        assert_eq!(mgr.status, ContractStatus::Active);
        mgr.update(mgr.active_contract().time_limit_seconds + 0.1, &grid, &PhysicsSystem::new());
        assert_eq!(mgr.status, ContractStatus::Failed("TIME LIMIT EXCEEDED"));
        mgr.start_contract(grid.solid_count());
        assert_eq!(mgr.status, ContractStatus::Active);
        assert_eq!(mgr.elapsed_time, 0.0);
    }

    #[test]
    fn test_campaign_manager_contains_eight_contracts() {
        let mgr = CampaignManager::new();
        assert_eq!(mgr.contracts.len(), 8);
        assert_eq!(mgr.contracts[0].title, "CONTRACT 01: CORPORATE DROP");
        assert_eq!(mgr.contracts[1].title, "CONTRACT 02: BRIDGE SEVERANCE");
        assert_eq!(mgr.contracts[2].title, "CONTRACT 03: FUEL CHAIN REACTION");
        assert_eq!(mgr.contracts[3].title, "CONTRACT 04: FORTRESS SIEGE");
        assert_eq!(mgr.contracts[4].title, "CONTRACT 05: TOKAMAK CONTAINMENT BREACH");
        assert_eq!(mgr.contracts[5].title, "CONTRACT 06: DOMINO PERFECTION");
        assert_eq!(mgr.contracts[6].title, "CONTRACT 07: AEROPOLIS FOUNDATION BREACH");
        assert_eq!(mgr.contracts[7].title, "CONTRACT 08: NEO-TOKYO PRECISION DROP");
    }

    #[test]
    fn contract_targets_match_districts_without_duplicate_load_selection() {
        let mgr = CampaignManager::new();
        let expected = [
            LevelPreset::ApexSkyscraper, LevelPreset::SuspensionSkybridge,
            LevelPreset::ChemicalRefinery, LevelPreset::DominoMonoliths,
            LevelPreset::QuantumFusionReactor, LevelPreset::GothicCitadel,
            LevelPreset::MilitaryFortress, LevelPreset::NeoTokyoSpire,
        ];
        for (idx, &preset) in expected.iter().enumerate() {
            assert_eq!(mgr.contracts[idx].target_level, preset);
            assert_eq!(mgr.contracts.iter().position(|c| c.target_level == preset), Some(idx), "Level loading must keep the selected contract");
        }
    }

    #[test]
    fn generated_district_c4_limit_survives_detonation_and_resets_on_retry() {
        use crate::audio::SoundSystem;
        use crate::weapon::WeaponSystem;
        let mut mgr = CampaignManager::new();
        mgr.select_contract(1);
        let mut grid = VoxelGrid::new();
        let layout = crate::world_gen::generate_level(&mut grid, mgr.active_contract().target_level);
        let camera = layout.spawn_point(&grid) + Vec3::Y * 1.75;
        let hit = grid.raycast(camera, Vec3::NEG_Y, 8.0).expect("Spawn has solid support for C4");
        let mut weapon = WeaponSystem::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        mgr.start_contract(grid.solid_count());
        for _ in 0..4 {
            assert!(weapon.place_c4_charge(hit.world_pos, hit.normal, Some(hit.voxel_pos)));
            mgr.record_c4_placement();
        }
        mgr.update(0.05, &grid, &physics);
        assert_eq!(mgr.status, ContractStatus::Active);
        let (_, detonated) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, camera);
        assert_eq!(detonated, 4);
        assert!(weapon.c4_charges.is_empty());
        assert_eq!(mgr.c4_charges_used, 4, "Detonation does not refund the contract allocation");
        assert_eq!(mgr.ordnance_dollars_spent, 10_000.0);
        let new_hit = grid.raycast(camera, Vec3::NEG_Y, 20.0).expect("Excavation still has a crater floor");
        assert!(weapon.place_c4_charge(new_hit.world_pos, new_hit.normal, Some(new_hit.voxel_pos)));
        mgr.record_c4_placement();
        mgr.update(0.05, &grid, &physics);
        assert_eq!(mgr.status, ContractStatus::Failed("C4 CHARGE ALLOCATION EXCEEDED"));
        crate::world_gen::generate_level(&mut grid, mgr.active_contract().target_level);
        mgr.start_contract(grid.solid_count());
        assert_eq!(mgr.status, ContractStatus::Active);
        assert_eq!(mgr.c4_charges_used, 0);
        assert_eq!(mgr.ordnance_dollars_spent, 0.0);
    }

    #[test]
    fn test_drop_zone_bounds_evaluation() {
        let zone = DropZone::new(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 20.0, 10.0));
        assert!(zone.contains(Vec3::new(0.0, 5.0, 0.0)));
        assert!(zone.contains(Vec3::new(-10.0, 0.0, 10.0)));
        assert!(!zone.contains(Vec3::new(15.0, 5.0, 0.0)));
        assert!(!zone.contains(Vec3::new(0.0, -1.0, 0.0)));
    }

    #[test]
    fn test_contract_c4_and_ordnance_accounting() {
        let mut mgr = CampaignManager::new();
        mgr.select_contract(1); // Bridge Severance (max 4 C4)
        mgr.start_contract(10_000);

        assert_eq!(mgr.c4_charges_used, 0);
        assert_eq!(mgr.ordnance_dollars_spent, 0.0);

        mgr.record_c4_placement();
        mgr.record_c4_placement();
        assert_eq!(mgr.c4_charges_used, 2);
        assert_eq!(mgr.ordnance_dollars_spent, 5_000.0);

        mgr.record_ordnance_spend(10_000.0);
        assert_eq!(mgr.ordnance_dollars_spent, 15_000.0);
    }

    #[test]
    fn test_grading_logic_evaluates_s_rank() {
        let mgr = CampaignManager::new();
        let contract = &mgr.contracts[0];

        let mut test_mgr = CampaignManager::new();
        test_mgr.current_destruction_ratio = 0.95;
        test_mgr.collateral_damage_dollars = 5_000.0;
        test_mgr.ordnance_dollars_spent = 50_000.0;
        test_mgr.rubble_in_zone_ratio = 0.98;

        let grade = test_mgr.calculate_final_grade(contract);
        assert_eq!(grade, ContractGrade::S);
    }

    #[test]
    fn test_grading_logic_fails_on_budget_overrun() {
        let mgr = CampaignManager::new();
        let contract = &mgr.contracts[0];

        let mut test_mgr = CampaignManager::new();
        test_mgr.current_destruction_ratio = 0.95;
        test_mgr.collateral_damage_dollars = 500_000.0; // Over $100k cap
        test_mgr.ordnance_dollars_spent = 50_000.0;

        let grade = test_mgr.calculate_final_grade(contract);
        assert_eq!(grade, ContractGrade::F);
    }

    #[test]
    fn test_campaign_contract_next_prev_cycling() {
        let mut mgr = CampaignManager::new();
        assert_eq!(mgr.active_contract_idx, 0);

        let next = mgr.next_contract();
        assert_eq!(next, 1);
        assert_eq!(mgr.active_contract_idx, 1);

        for _ in 1..8 {
            mgr.next_contract();
        }
        assert_eq!(mgr.active_contract_idx, 0, "Cycling 8 times returns to 0");

        let prev = mgr.prev_contract();
        assert_eq!(prev, 7, "Prev from 0 wraps around to 7");
        assert_eq!(mgr.active_contract_idx, 7);
    }

    #[test]
    fn falling_rubble_is_evaluated_after_landing() {
        let mut mgr = CampaignManager::new();
        let grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        mgr.start_contract(100);
        physics.chunks.push(crate::physics::DynamicChunk::new(
            Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
            vec![(Vec3::ZERO, crate::voxel::VoxelType::Concrete); 2_000],
            10.0, 0.0, false, (Vec3::ZERO, Vec3::ZERO),
        ));

        mgr.update(0.1, &grid, &physics);
        assert_eq!(mgr.status, ContractStatus::Active, "Wait for falling rubble to land before grading or charging collateral");
        assert_eq!(mgr.collateral_damage_dollars, 0.0);

        physics.chunks[0].position.y = 1.0;
        physics.chunks[0].settled = true;
        mgr.update(0.1, &grid, &physics);
        assert_eq!(mgr.status, ContractStatus::Passed(ContractGrade::S));
    }

    #[test]
    fn newly_detached_rubble_prevents_premature_precision_grade() {
        use crate::voxel::VoxelType;
        let mut mgr = CampaignManager::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        for y in 1..=6 {
            grid.set(96, y, 96, VoxelType::Concrete);
        }
        grid.set(97, 6, 96, VoxelType::Concrete);
        mgr.start_contract(grid.solid_count());
        for y in 1..6 {
            grid.set(96, y, 96, VoxelType::Air);
        }
        assert!((1.0 - grid.solid_count() as f32 / mgr.initial_target_voxels as f32) >= mgr.active_contract().target_destruction_ratio);
        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "The destroyed support releases real structural rubble");
        for island in islands {
            physics.spawn_island_chunk(island, Vec3::NEG_Y, Vec3::ZERO);
        }
        mgr.update(0.05, &grid, &physics);
        assert_eq!(mgr.status, ContractStatus::Active, "Grade only after this damage frame's detached rubble lands");
        assert_eq!(mgr.collateral_damage_dollars, 0.0);
        for chunk in &mut physics.chunks {
            chunk.position.y = 1.0;
            chunk.settled = true;
        }
        mgr.update(0.05, &grid, &physics);
        assert_eq!(mgr.status, ContractStatus::Passed(ContractGrade::S));
    }

    #[test]
    fn settled_rubble_spill_uses_voxel_positions_and_preserves_damage() {
        let mut mgr = CampaignManager::new();
        let mut grid = VoxelGrid::new();
        grid.set(96, 10, 96, crate::voxel::VoxelType::Concrete);
        let mut physics = PhysicsSystem::new();
        mgr.start_contract(1);
        let voxels = vec![
            (Vec3::new(-0.5, 0.0, 0.0), crate::voxel::VoxelType::Concrete),
            (Vec3::new(0.5, 0.0, 0.0), crate::voxel::VoxelType::Concrete),
        ];
        let bounds = crate::physics::DynamicChunk::compute_aabb(&voxels);
        physics.chunks.push(crate::physics::DynamicChunk::new(
            Vec3::new(20.0, 1.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
            voxels, 10.0, 0.0, true, bounds,
        ));

        mgr.update(0.1, &grid, &physics);
        assert_eq!(mgr.collateral_damage_dollars, 100.0, "A chunk centered on the boundary still spills one voxel");
        assert_eq!(mgr.rubble_in_zone_ratio, 0.5);

        physics.chunks.clear();
        mgr.update(0.1, &grid, &physics);
        assert_eq!(mgr.collateral_damage_dollars, 100.0, "Removing damaged rubble must not erase its collateral penalty");
    }

    #[test]
    fn final_grade_rejects_time_limit_overrun() {
        let mut mgr = CampaignManager::new();
        mgr.current_destruction_ratio = 1.0;
        mgr.elapsed_time = mgr.active_contract().time_limit_seconds + 0.1;
        assert_eq!(mgr.calculate_final_grade(mgr.active_contract()), ContractGrade::F);
    }

    #[test]
    fn test_contract_card_bounds_and_no_overlap() {
        let test_resolutions = [
            (800.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
        ];

        for &(sw, sh) in &test_resolutions {
            let s = crate::ui::compute_ui_scale(sw, sh);
            let (ccx, ccy, ccw, cch) = CampaignManager::contract_card_rect(sw, sh);
            // Verify below visor/scanner
            assert!(ccy >= 96.0 * s, "Contract card top must clear visor/scanner on {}x{}", sw, sh);
            // Verify above weapon score card (sh - 188.0 * s)
            let score_y = sh - 188.0 * s;
            assert!(ccy + cch <= score_y, "Contract card bottom must not overlap score card on {}x{}", sw, sh);
            assert!(ccx + ccw <= sw, "Contract card must stay within screen bounds on {}x{}", sw, sh);
        }
    }
}
