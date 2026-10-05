//! Executable F1StrokeExecution event reduction over T03's confirmed point ledger.

use crate::f1::{CombinedBar, ConfirmedFractal, F1State, FractalKind, FractalPoint};
use crate::number::{Int, Nat};
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub struct StrokeGeometry {
    pub begin_index: usize,
    pub formation_end_index: usize,
    pub end_index: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub struct TentativeStroke {
    pub begin_point_index: usize,
    pub geometry: StrokeGeometry,
    pub formed_known_at: Nat,
    pub current_known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmedStroke {
    pub begin_point_index: usize,
    pub geometry: StrokeGeometry,
    pub formed_known_at: Nat,
    pub current_known_at: Nat,
    pub confirmed_by_point_index: usize,
    pub confirmed_known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct PendingReverse {
    pub predecessor: TentativeStroke,
    pub confirming_point_index: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum StrokePhase {
    AwaitingFirstPoint,
    SeekingFirstStroke {
        anchor_point_index: usize,
    },
    BuildingStroke {
        active: TentativeStroke,
        pending: Option<PendingReverse>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub struct StrokeState {
    pub confirmed: Vec<ConfirmedStroke>,
    pub phase: StrokePhase,
    pub detected_point_count: usize,
    pub confirmed_point_count: usize,
}
pub fn new_stroke_state() -> StrokeState {
    StrokeState {
        confirmed: Vec::new(),
        phase: StrokePhase::AwaitingFirstPoint,
        detected_point_count: 0,
        confirmed_point_count: 0,
    }
}
fn copy_geometry(value: &StrokeGeometry) -> StrokeGeometry {
    StrokeGeometry {
        begin_index: value.begin_index,
        formation_end_index: value.formation_end_index,
        end_index: value.end_index,
    }
}
pub fn copy_tentative(value: &TentativeStroke) -> TentativeStroke {
    TentativeStroke {
        begin_point_index: value.begin_point_index,
        geometry: copy_geometry(&value.geometry),
        formed_known_at: value.formed_known_at.clone(),
        current_known_at: value.current_known_at.clone(),
    }
}
pub fn copy_confirmed_stroke(value: &ConfirmedStroke) -> ConfirmedStroke {
    ConfirmedStroke {
        begin_point_index: value.begin_point_index,
        geometry: copy_geometry(&value.geometry),
        formed_known_at: value.formed_known_at.clone(),
        current_known_at: value.current_known_at.clone(),
        confirmed_by_point_index: value.confirmed_by_point_index,
        confirmed_known_at: value.confirmed_known_at.clone(),
    }
}
fn greater(a: &Int, b: &Int) -> bool {
    a.cmp_exact(b) == Ordering::Greater
}
fn less(a: &Int, b: &Int) -> bool {
    a.cmp_exact(b) == Ordering::Less
}
fn maximum(a: &Nat, b: &Nat) -> Nat {
    match a.cmp_exact(b) {
        Ordering::Less => b.clone(),
        _ => a.clone(),
    }
}
#[doc = " A point index is a semantic nat; only indexes actually in this ledger resolve."]
pub(crate) fn center_position(bars: &Vec<CombinedBar>, point: &FractalPoint) -> Option<usize> {
    let mut index = 0usize;
    while index < bars.len() {
        let target = Nat::from_u64(index as u64);
        match point.center_index.cmp_exact(&target) {
            Ordering::Equal => {
                {};
                return Some(index);
            }
            _ => {}
        }
        index += 1;
    }
    None
}
fn kind_matches(a: &FractalKind, b: &FractalKind) -> bool {
    match (a, b) {
        (FractalKind::Top, FractalKind::Top) | (FractalKind::Bottom, FractalKind::Bottom) => true,
        _ => false,
    }
}
fn strictly_more_extreme(f1: &F1State, candidate: &FractalPoint, current: &FractalPoint) -> bool {
    let Some(candidate_center) = center_position(&f1.combined, candidate) else {
        return false;
    };
    let Some(current_center) = center_position(&f1.combined, current) else {
        return false;
    };
    if f1.combined.len() < 3
        || candidate_center == 0
        || current_center == 0
        || candidate_center >= f1.combined.len() - 1
        || current_center >= f1.combined.len() - 1
    {
        return false;
    }
    match &candidate.kind {
        FractalKind::Top => greater(
            &f1.combined[candidate_center].high,
            &f1.combined[current_center].high,
        ),
        FractalKind::Bottom => less(
            &f1.combined[candidate_center].low,
            &f1.combined[current_center].low,
        ),
    }
}
fn can_form_stroke(f1: &F1State, begin_index: usize, end_index: usize) -> Option<(usize, usize)> {
    if begin_index >= end_index || end_index >= f1.confirmed.len() {
        return None;
    }
    let begin = &f1.confirmed[begin_index].point;
    let end = &f1.confirmed[end_index].point;
    if kind_matches(&begin.kind, &end.kind) {
        return None;
    }
    let Some(begin_center) = center_position(&f1.combined, begin) else {
        return None;
    };
    let Some(end_center) = center_position(&f1.combined, end) else {
        return None;
    };
    if f1.combined.len() < 3
        || begin_center == 0
        || end_center < begin_center
        || end_center - begin_center < 4
        || end_center >= f1.combined.len() - 1
    {
        return None;
    }
    let bars = &f1.combined;
    let strict_patterns = match &begin.kind {
        FractalKind::Bottom => {
            let begin_low = &bars[begin_center].low;
            let end_high = &bars[end_center].high;
            less(begin_low, &bars[end_center - 1].low)
                && less(begin_low, &bars[end_center].low)
                && less(begin_low, &bars[end_center + 1].low)
                && greater(end_high, &bars[begin_center - 1].high)
                && greater(end_high, &bars[begin_center].high)
                && greater(end_high, &bars[begin_center + 1].high)
        }
        FractalKind::Top => {
            let begin_high = &bars[begin_center].high;
            let end_low = &bars[end_center].low;
            greater(begin_high, &bars[end_center - 1].high)
                && greater(begin_high, &bars[end_center].high)
                && greater(begin_high, &bars[end_center + 1].high)
                && less(end_low, &bars[begin_center - 1].low)
                && less(end_low, &bars[begin_center].low)
                && less(end_low, &bars[begin_center + 1].low)
        }
    };
    if !strict_patterns {
        return None;
    }
    let mut index = begin_center + 1;
    while index < end_center {
        let clean = match &begin.kind {
            FractalKind::Bottom => {
                bars[index].high.cmp_exact(&bars[end_center].high) != Ordering::Greater
            }
            FractalKind::Top => bars[index].low.cmp_exact(&bars[end_center].low) != Ordering::Less,
        };
        if !clean {
            return None;
        }
        index += 1;
    }
    Some((begin_center, end_center))
}
fn formation_known_at(f1: &F1State, begin_center: usize, end_center: usize) -> Nat {
    let mut known_at = Nat::zero();
    let mut index = begin_center - 1;
    loop {
        if index >= f1.combined.len() {
            break;
        }
        known_at = maximum(&known_at, &f1.combined[index].known_at);
        if index > end_center {
            break;
        }
        index += 1;
    }
    known_at
}
fn new_tentative(
    f1: &F1State,
    begin_index: usize,
    end_index: usize,
    begin_center: usize,
    end_center: usize,
) -> TentativeStroke {
    let formed = formation_known_at(f1, begin_center, end_center);
    TentativeStroke {
        begin_point_index: begin_index,
        geometry: StrokeGeometry {
            begin_index,
            formation_end_index: end_index,
            end_index,
        },
        formed_known_at: formed.clone(),
        current_known_at: formed,
    }
}
fn replace_active(
    active: TentativeStroke,
    point_index: usize,
    detected_known_at: &Nat,
) -> TentativeStroke {
    TentativeStroke {
        begin_point_index: active.begin_point_index,
        geometry: StrokeGeometry {
            begin_index: active.geometry.begin_index,
            formation_end_index: active.geometry.formation_end_index,
            end_index: point_index,
        },
        formed_known_at: active.formed_known_at,
        current_known_at: maximum(&active.current_known_at, detected_known_at),
    }
}
fn freeze(pending: PendingReverse, confirmed_known_at: &Nat) -> ConfirmedStroke {
    let predecessor = pending.predecessor;
    ConfirmedStroke {
        begin_point_index: predecessor.begin_point_index,
        geometry: predecessor.geometry,
        formed_known_at: predecessor.formed_known_at,
        current_known_at: predecessor.current_known_at.clone(),
        confirmed_by_point_index: pending.confirming_point_index,
        confirmed_known_at: maximum(&predecessor.current_known_at, confirmed_known_at),
    }
}
#[doc = " T03 commits a formerly forming point only when its right neighbor freezes."]
#[doc = " Detection and confirmation are then the same closed-Bar transaction."]
pub fn accept_confirmed_point(
    mut state: StrokeState,
    f1: &F1State,
    point: &ConfirmedFractal,
) -> (StrokeState, Option<ConfirmedStroke>) {
    let index = state.detected_point_count;
    if index != state.confirmed_point_count
        || index >= f1.confirmed.len()
        || f1.confirmed[index] != *point
    {
        return (state, None);
    }
    if let StrokePhase::SeekingFirstStroke { anchor_point_index } = &state.phase {
        if *anchor_point_index >= index {
            return (state, None);
        }
    }
    let phase = state.phase;
    state.phase = match phase {
        StrokePhase::AwaitingFirstPoint => StrokePhase::SeekingFirstStroke {
            anchor_point_index: index,
        },
        StrokePhase::SeekingFirstStroke { anchor_point_index } => {
            let anchor = &f1.confirmed[anchor_point_index].point;
            if kind_matches(&anchor.kind, &point.point.kind) {
                if strictly_more_extreme(f1, &point.point, anchor) {
                    StrokePhase::SeekingFirstStroke {
                        anchor_point_index: index,
                    }
                } else {
                    StrokePhase::SeekingFirstStroke { anchor_point_index }
                }
            } else {
                match can_form_stroke(f1, anchor_point_index, index) {
                    Some((begin_center, end_center)) => StrokePhase::BuildingStroke {
                        active: new_tentative(
                            f1,
                            anchor_point_index,
                            index,
                            begin_center,
                            end_center,
                        ),
                        pending: None,
                    },
                    None => StrokePhase::SeekingFirstStroke { anchor_point_index },
                }
            }
        }
        StrokePhase::BuildingStroke { active, pending } => {
            if pending.is_some() || active.geometry.end_index >= index {
                StrokePhase::BuildingStroke { active, pending }
            } else {
                let current = &f1.confirmed[active.geometry.end_index].point;
                if kind_matches(&current.kind, &point.point.kind) {
                    if strictly_more_extreme(f1, &point.point, current) {
                        StrokePhase::BuildingStroke {
                            active: replace_active(active, index, &point.point.detected_known_at),
                            pending: None,
                        }
                    } else {
                        StrokePhase::BuildingStroke {
                            active,
                            pending: None,
                        }
                    }
                } else {
                    match can_form_stroke(f1, active.geometry.end_index, index) {
                        Some((begin_center, end_center)) => {
                            let next = new_tentative(
                                f1,
                                active.geometry.end_index,
                                index,
                                begin_center,
                                end_center,
                            );
                            StrokePhase::BuildingStroke {
                                active: next,
                                pending: Some(PendingReverse {
                                    predecessor: copy_tentative(&active),
                                    confirming_point_index: index,
                                }),
                            }
                        }
                        None => StrokePhase::BuildingStroke {
                            active,
                            pending: None,
                        },
                    }
                }
            }
        }
    };
    state.detected_point_count += 1;
    let mut published = None;
    let phase = state.phase;
    state.phase = match phase {
        StrokePhase::BuildingStroke {
            active,
            pending: Some(pending),
        } if pending.confirming_point_index == index => {
            let confirmed = freeze(pending, &point.confirmed_known_at);
            state.confirmed.push(copy_confirmed_stroke(&confirmed));
            published = Some(confirmed);
            StrokePhase::BuildingStroke {
                active,
                pending: None,
            }
        }
        other => other,
    };
    state.confirmed_point_count += 1;
    {}
    (state, published)
}
