//! Puzzle solving and status tracking.

use soroban_sdk::{BytesN, Env};

use super::{
    get_puzzle,
    puzzle::{is_time_reached, verify_solution},
    save_puzzle, PuzzleStatus, TimeLockPuzzle,
};

/// Result of a solve attempt.
pub enum SolveResult {
    /// Puzzle solved successfully; tip can now be released.
    Success,
    /// Unlock time has not been reached yet.
    TooEarly,
    /// The supplied secret/nonce did not match the commitment.
    WrongSolution,
    /// Puzzle is not in `Active` status.
    NotActive,
}

/// Attempt to solve a puzzle.
///
/// Validates:
/// 1. Puzzle exists and is `Active`.
/// 2. `unlock_time` has been reached.
/// 3. The provided `secret` + `nonce` reproduce the stored commitment.
///
/// On success the puzzle status is updated to `Solved`.
pub fn solve_puzzle(
    env: &Env,
    puzzle_id: u64,
    secret: &BytesN<32>,
    nonce: &BytesN<32>,
) -> SolveResult {
    let mut puzzle: TimeLockPuzzle = match get_puzzle(env, puzzle_id) {
        Some(p) => p,
        None => return SolveResult::NotActive, // treat missing as not active
    };

    if puzzle.status != PuzzleStatus::Active {
        return SolveResult::NotActive;
    }

    if !is_time_reached(env, puzzle.unlock_time) {
        return SolveResult::TooEarly;
    }

    if !verify_solution(env, &puzzle.commitment, secret, nonce) {
        return SolveResult::WrongSolution;
    }

    puzzle.status = PuzzleStatus::Solved;
    save_puzzle(env, &puzzle);

    SolveResult::Success
}

/// Cancel an active puzzle (only the creator may do this).
///
/// Returns `true` if the puzzle was successfully cancelled, `false` if it was
/// already solved or cancelled.
pub fn cancel_puzzle(env: &Env, puzzle_id: u64) -> bool {
    let mut puzzle: TimeLockPuzzle = match get_puzzle(env, puzzle_id) {
        Some(p) => p,
        None => return false,
    };

    if puzzle.status != PuzzleStatus::Active {
        return false;
    }

    puzzle.status = PuzzleStatus::Cancelled;
    save_puzzle(env, &puzzle);
    true
}

/// Return the current status of a puzzle without modifying state.
pub fn get_puzzle_status(env: &Env, puzzle_id: u64) -> Option<PuzzleStatus> {
    get_puzzle(env, puzzle_id).map(|p| p.status)
}

/// Return whether the puzzle's unlock time has been reached.
pub fn is_puzzle_unlocked(env: &Env, puzzle_id: u64) -> bool {
    get_puzzle(env, puzzle_id)
        .map(|p| is_time_reached(env, p.unlock_time))
        .unwrap_or(false)
}
