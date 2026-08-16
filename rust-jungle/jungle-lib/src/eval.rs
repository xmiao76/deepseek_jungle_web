//! Evaluation: the linear features × weights form, ported from the C#
//! EvalFeatureExtractor + EvalParameters (24 weights). Score must be
//! byte-identical to the C# engine.

use crate::board::*;
use crate::movegen::{can_capture, has_clear_jump_to_enemy_half, neighbors};

pub const ENDGAME_PIECE_COUNT: i32 = 8;
pub const DEVELOPMENT_MATERIAL_GATE: i32 = 20;

// Weights (EvalParameters.Default).
pub const MATERIAL_W: i32 = 100;
pub const FORWARD_W: i32 = 1;
pub const DEN_OFFENSE_W: i32 = 12;
pub const DEN_GUARD_W: i32 = 8;
pub const TRAP_W: i32 = 80;
pub const DOOMED_PER_RANK_W: i32 = 5;
pub const DEN_ESCORT_W: i32 = 30;
pub const RIVER_BANK_W: i32 = 15;
pub const JUMP_PATH_W: i32 = 10;
pub const RAT_NEAR_WATER_W: i32 = 8;
pub const RAT_IN_WATER_W: i32 = 12;
pub const ELEPHANT_FEAR_W: i32 = 15;
pub const THREAT_STRONGER_W: i32 = 15;
pub const THREAT_EQUAL_W: i32 = 8;
pub const RAT_ELEPHANT_W: i32 = 25;
pub const MOBILITY_W: i32 = 3;
pub const DEN_THREAT_W: i32 = 40;
pub const ENDGAME_DEN_THREAT_W: i32 = 200;
pub const ENDGAME_ADV1_W: i32 = 50;
pub const ENDGAME_ADV2_W: i32 = 25;
pub const BACK_RANK_W: i32 = 5;
pub const RAT_DEN_INFLUENCE_W: i32 = 15;
pub const FRONT_TRAP_GUARD_W: i32 = 20;
pub const CLEAR_JUMP_LANE_W: i32 = 12;

#[derive(Default, Clone, Copy)]
struct Features {
    material_my: i32, material_opp: i32,
    forward_my: i32, forward_opp: i32,
    den_offense_my: i32, den_offense_opp: i32,
    den_guard_my: i32, den_guard_opp: i32,
    trap_my: i32, trap_opp: i32,
    doomed_my: i32, doomed_opp: i32,
    escort_my: i32, escort_opp: i32,
    river_bank_my: i32, river_bank_opp: i32,
    jump_path_my: i32, jump_path_opp: i32,
    rat_near_water_my: i32, rat_near_water_opp: i32,
    rat_in_water_my: i32, rat_in_water_opp: i32,
    elephant_fear_my: i32, elephant_fear_opp: i32,
    stronger_my: i32, stronger_opp: i32,
    equal_my: i32, equal_opp: i32,
    rat_elephant_my: i32, rat_elephant_opp: i32,
    den_threat_excess: i32,
    endgame_den_threat: i32,
    endgame_adv1_my: i32, endgame_adv1_opp: i32,
    endgame_adv2_my: i32, endgame_adv2_opp: i32,
    back_rank_my: i32, back_rank_opp: i32,
    rat_den_influence_my: i32, rat_den_influence_opp: i32,
    front_trap_guard_my: i32, front_trap_guard_opp: i32,
    clear_jump_lane_my: i32, clear_jump_lane_opp: i32,
}

fn manhattan(a: i32, b: i32) -> i32 {
    ((a % 7) - (b % 7)).abs() + ((a / 7) - (b / 7)).abs()
}

fn forward_bonus(rank: i32, row: i32, owner: i32) -> i32 {
    let progress = if owner == 0 { row } else { 8 - row };
    let aggression = match rank {
        7 => 6, 6 => 4, 5 => 4, 4 => 3, 3 => 3, 2 => 2, 1 => 2, 8 => 1, _ => 2,
    };
    progress * aggression
}

fn is_river_bank(sq: i32) -> bool {
    if is_water(sq as u8) {
        return false;
    }
    for &adj in neighbors(sq as u8) {
        if is_water(adj) {
            return true;
        }
    }
    false
}

fn is_adjacent_to_water(sq: i32) -> bool {
    for &adj in neighbors(sq as u8) {
        if is_water(adj) {
            return true;
        }
    }
    false
}

fn has_river_between(from_sq: i32, to_sq: i32) -> bool {
    let fc = from_sq % 7;
    let fr = from_sq / 7;
    let tc = to_sq % 7;
    let tr = to_sq / 7;
    let step_col = (tc - fc).signum();
    let step_row = (tr - fr).signum();
    let mut col = fc + step_col;
    let mut row = fr + step_row;
    let mut found = false;
    while col != tc || row != tr {
        if (0..=6).contains(&col) && (0..=8).contains(&row) && is_water((row * 7 + col) as u8) {
            found = true;
        }
        col += step_col;
        row += step_row;
        if (col - fc).abs() > (tc - fc).abs() && (row - fr).abs() > (tr - fr).abs() {
            break;
        }
    }
    found
}

/// Iterates all pieces of `side`, invoking `f(id, sq)` for each (any order).
fn for_each_piece<F: FnMut(u8, u8)>(board: &Board, side: u8, mut f: F) {
    for sq in 0..SQUARES as u8 {
        let id = board.squares[sq as usize];
        if id != 0 && owner_of(id) == side {
            f(id, sq);
        }
    }
}

fn total_material(board: &Board) -> i32 {
    let mut total = 0;
    for sq in 0..SQUARES {
        let id = board.squares[sq];
        if id != 0 {
            total += rank_of(id) as i32;
        }
    }
    total
}

fn extract_static(board: &Board, side: u8, legacy_eval: bool, legacy_p5: bool) -> Features {
    let mut f = Features::default();

    let total_pieces = (board.count[0] + board.count[1]) as i32;
    let is_endgame = total_pieces <= ENDGAME_PIECE_COUNT;
    let total_material = total_material(board);

    // Den escort triggers.
    let mut blue_den_threatened = false;
    let mut red_den_threatened = false;
    if !legacy_eval {
        for_each_piece(board, 1, |_id, sq| {
            if manhattan(sq as i32, 3) <= 3 {
                blue_den_threatened = true;
            }
        });
        for_each_piece(board, 0, |_id, sq| {
            if manhattan(sq as i32, 59) <= 3 {
                red_den_threatened = true;
            }
        });
    }

    for s in 0..2u8 {
        let own_den_threatened = if s == 0 { blue_den_threatened } else { red_den_threatened };
        let for_me = s == side;
        for_each_piece(board, s, |id, sq| {
            accumulate_piece(&mut f, board, id, sq, is_endgame, legacy_eval, legacy_p5, own_den_threatened, for_me, total_material);
        });
    }

    // Den threat detection.
    let own_den_col = 3i32;
    let own_den_row = if side == 0 { 0 } else { 8 };
    let mut defenders = 0i32;
    let mut attackers = 0i32;
    for s in 0..2u8 {
        for_each_piece(board, s, |_id, sq| {
            let col = (sq % 7) as i32;
            let row = (sq / 7) as i32;
            if (col - own_den_col).abs() + (row - own_den_row).abs() <= 2 {
                if s == side {
                    defenders += 1;
                } else {
                    attackers += 1;
                }
            }
        });
    }

    f.den_threat_excess = (attackers - defenders).max(0);
    if is_endgame && attackers > 0 && defenders == 0 {
        f.endgame_den_threat = 1;
    }

    f
}

#[allow(clippy::too_many_arguments)]
fn accumulate_piece(
    f: &mut Features, board: &Board, id: u8, sq: u8, is_endgame: bool,
    legacy_eval: bool, legacy_p5: bool, own_den_threatened: bool, for_me: bool, total_material: i32,
) {
    let rank = rank_of(id) as i32;
    let owner = owner_of(id) as i32;
    let sq_i = sq as i32;
    let row = sq_i / 7;
    let col = sq_i % 7;

    if for_me { f.material_my += rank; } else { f.material_opp += rank; }

    let forward = forward_bonus(rank, row, owner);
    if for_me { f.forward_my += forward; } else { f.forward_opp += forward; }

    let own_den_row = if owner == 0 { 0 } else { 8 };
    let opp_den_row = if owner == 0 { 8 } else { 0 };
    let dist_opp = (col - 3).abs() + (row - opp_den_row).abs();
    let dist_own = (col - 3).abs() + (row - own_den_row).abs();

    if dist_opp <= 3 {
        let offense = 4 - dist_opp;
        if for_me { f.den_offense_my += offense; } else { f.den_offense_opp += offense; }
    }
    if dist_own <= 2 {
        let guard = 3 - dist_own;
        if for_me { f.den_guard_my += guard; } else { f.den_guard_opp += guard; }
    }

    // P5 front-trap guard.
    if !legacy_p5 && sq_i == (if owner == 0 { 10 } else { 52 }) {
        if for_me { f.front_trap_guard_my += 1; } else { f.front_trap_guard_opp += 1; }
    }

    // Trap penalty + doomed-piece bonus.
    if eff_rank(id, sq) == 0 {
        if for_me { f.trap_my += 1; } else { f.trap_opp += 1; }
        if !legacy_eval {
            if for_me { f.doomed_my += rank; } else { f.doomed_opp += rank; }
        }
    }

    // Den escort.
    if !legacy_eval && rank >= 7 && own_den_threatened && dist_own <= 2 {
        if for_me { f.escort_my += 1; } else { f.escort_opp += 1; }
    }

    // Lion/Tiger river bank + jump path + clear jump lane.
    if rank == 7 || rank == 6 {
        if is_river_bank(sq_i) {
            if for_me { f.river_bank_my += 1; } else { f.river_bank_opp += 1; }
        }
        let opp_den_sq = if owner == 0 { 59 } else { 3 };
        if has_river_between(sq_i, opp_den_sq) {
            if for_me { f.jump_path_my += 1; } else { f.jump_path_opp += 1; }
        }
        if !legacy_p5 && has_clear_jump_to_enemy_half(board, id, sq) {
            if for_me { f.clear_jump_lane_my += 1; } else { f.clear_jump_lane_opp += 1; }
        }
    }

    // Rat near water.
    if rank == 1 {
        if is_adjacent_to_water(sq_i) {
            if for_me { f.rat_near_water_my += 1; } else { f.rat_near_water_opp += 1; }
        }
        if is_water(sq) {
            if for_me { f.rat_in_water_my += 1; } else { f.rat_in_water_opp += 1; }
        }
        if !legacy_p5 && dist_opp <= 2 {
            let influence = 3 - dist_opp;
            if for_me { f.rat_den_influence_my += influence; } else { f.rat_den_influence_opp += influence; }
        }
    }

    // Elephant fear.
    if rank == 8 {
        let opp = owner ^ 1;
        let mut fear = 0;
        for_each_piece(board, opp as u8, |enemy, enemy_sq| {
            if rank_of(enemy) == 1 && !is_water(enemy_sq) {
                let dist = manhattan(sq_i, enemy_sq as i32);
                if dist <= 3 {
                    fear += 4 - dist;
                }
            }
        });
        if for_me { f.elephant_fear_my += fear; } else { f.elephant_fear_opp += fear; }
    }

    // Threat analysis.
    for &neighbor in neighbors(sq) {
        let enemy = board.squares[neighbor as usize];
        if enemy == 0 || owner_of(enemy) as i32 == owner {
            continue;
        }
        let our_eff = eff_rank(id, sq) as i32;
        let enemy_eff = eff_rank(enemy, neighbor) as i32;
        let enemy_rank = rank_of(enemy) as i32;

        if rank == 8 && enemy_rank == 1 {
            if !is_water(neighbor) {
                if for_me { f.rat_elephant_my += 1; } else { f.rat_elephant_opp += 1; }
            }
        } else if enemy_eff >= our_eff {
            if can_capture(enemy, neighbor, id, sq) {
                if enemy_eff > our_eff {
                    if for_me { f.stronger_my += rank; } else { f.stronger_opp += rank; }
                } else {
                    if for_me { f.equal_my += rank; } else { f.equal_opp += rank; }
                }
            }
        }
    }

    // Endgame advance (PSQT-lite).
    if is_endgame && rank >= 5 {
        if dist_opp == 1 {
            if for_me { f.endgame_adv1_my += 1; } else { f.endgame_adv1_opp += 1; }
        } else if dist_opp == 2 {
            if for_me { f.endgame_adv2_my += 1; } else { f.endgame_adv2_opp += 1; }
        }
    }

    // Back-rank penalty.
    let home_row = if owner == 0 { 0 } else { 8 };
    if row == home_row && total_material > DEVELOPMENT_MATERIAL_GATE {
        if for_me { f.back_rank_my += 1; } else { f.back_rank_opp += 1; }
    }
}

fn dot(f: &Features) -> i32 {
    (f.material_my - f.material_opp) * MATERIAL_W
        + (f.forward_my - f.forward_opp) * FORWARD_W
        + (f.den_offense_my - f.den_offense_opp) * DEN_OFFENSE_W
        + (f.den_guard_my - f.den_guard_opp) * DEN_GUARD_W
        + (-f.trap_my + f.trap_opp) * TRAP_W
        + (-f.doomed_my + f.doomed_opp) * DOOMED_PER_RANK_W
        + (f.escort_my - f.escort_opp) * DEN_ESCORT_W
        + (f.river_bank_my - f.river_bank_opp) * RIVER_BANK_W
        + (f.jump_path_my - f.jump_path_opp) * JUMP_PATH_W
        + (f.rat_near_water_my - f.rat_near_water_opp) * RAT_NEAR_WATER_W
        + (f.rat_in_water_my - f.rat_in_water_opp) * RAT_IN_WATER_W
        + (-f.elephant_fear_my + f.elephant_fear_opp) * ELEPHANT_FEAR_W
        + (-f.stronger_my + f.stronger_opp) * THREAT_STRONGER_W
        + (-f.equal_my + f.equal_opp) * THREAT_EQUAL_W
        + (-f.rat_elephant_my + f.rat_elephant_opp) * RAT_ELEPHANT_W
        - f.den_threat_excess * DEN_THREAT_W
        - f.endgame_den_threat * ENDGAME_DEN_THREAT_W
        + (f.endgame_adv1_my - f.endgame_adv1_opp) * ENDGAME_ADV1_W
        + (f.endgame_adv2_my - f.endgame_adv2_opp) * ENDGAME_ADV2_W
        + (-f.back_rank_my + f.back_rank_opp) * BACK_RANK_W
        + (f.rat_den_influence_my - f.rat_den_influence_opp) * RAT_DEN_INFLUENCE_W
        + (f.front_trap_guard_my - f.front_trap_guard_opp) * FRONT_TRAP_GUARD_W
        + (f.clear_jump_lane_my - f.clear_jump_lane_opp) * CLEAR_JUMP_LANE_W
}

/// Static eval (no mobility), from `side`'s perspective.
pub fn evaluate_static(board: &Board, side: u8, legacy_eval: bool, legacy_p5: bool) -> i32 {
    dot(&extract_static(board, side, legacy_eval, legacy_p5))
}

/// Full eval with the mobility delta (myMobility - oppMobility).
pub fn evaluate(board: &Board, side: u8, my_mobility: i32, opp_mobility: i32, legacy_eval: bool, legacy_p5: bool) -> i32 {
    evaluate_static(board, side, legacy_eval, legacy_p5) + (my_mobility - opp_mobility) * MOBILITY_W
}
