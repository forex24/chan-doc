//! Exact incremental F1 primary split probes and segment ledger.

use crate::f1::feature::{
    FeatureElement, PriceInterval, SegmentDirection, copy_feature, copy_interval,
    feature_for_stroke, intervals_intersect, normalized_push, opposite, primary_target,
    standard_second_target, stroke_direction, stroke_interval, stroke_opposes,
};
use crate::f1::stroke::{ConfirmedStroke, StrokeState, center_position};
use crate::f1::{F1State, FractalKind};
use crate::number::{Int, Nat};
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub enum RecognitionReason {
    ImmediateNoGap,
    RepairedBySecondSequence,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SegmentTimes {
    pub occurred_at: Nat,
    pub candidate_known_at: Nat,
    pub confirmed_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmedSegment {
    pub ordinal: usize,
    pub begin_stroke_index: usize,
    pub end_stroke_index: usize,
    pub direction: SegmentDirection,
    pub elements: Vec<FeatureElement>,
    pub price_interval: PriceInterval,
    pub start_price: Int,
    pub end_price: Int,
    pub times: SegmentTimes,
    pub reason: RecognitionReason,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ActualBreakWitness {
    pub center_source_stroke_index: usize,
    pub observed_stroke_index: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeStage {
    SeekingPrimary,
    SeekingActualBreak {
        candidate_known_at: Nat,
    },
    SeekingSecond {
        candidate_known_at: Nat,
        actual_break: Option<ActualBreakWitness>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub struct PrimarySplitProbe {
    pub split_stroke_index: usize,
    pub primary: Vec<FeatureElement>,
    pub second: Vec<FeatureElement>,
    pub second_target: Option<usize>,
    pub stage: ProbeStage,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SegmentPending {
    pub begin_stroke_index: usize,
    pub direction: SegmentDirection,
    pub primary: Vec<FeatureElement>,
    pub known_at: Nat,
    pub probes: Vec<PrimarySplitProbe>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SegmentState {
    pub completed: Vec<ConfirmedSegment>,
    pub pending: Option<SegmentPending>,
    pub next_stroke_index: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SegmentError {
    InvalidStrokeSource,
    InvalidSegmentState,
}
pub struct Recognition {
    pub split_stroke_index: usize,
    pub window: Vec<FeatureElement>,
    pub candidate_known_at: Nat,
    pub confirmed_known_at: Nat,
    pub reason: RecognitionReason,
    pub actual_break: ActualBreakWitness,
}
pub enum ProbeStep {
    Discarded,
    Waiting(PrimarySplitProbe),
    Recognized(Recognition),
}
pub fn new_segment_state() -> SegmentState {
    SegmentState {
        completed: Vec::new(),
        pending: None,
        next_stroke_index: 0,
    }
}
fn less(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Less => true,
        _ => false,
    }
}
fn greater(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Greater => true,
        _ => false,
    }
}
fn le(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Greater => false,
        _ => true,
    }
}
fn max_int(a: &Int, b: &Int) -> Int {
    if less(a, b) { b.clone() } else { a.clone() }
}
fn min_int(a: &Int, b: &Int) -> Int {
    if greater(a, b) { b.clone() } else { a.clone() }
}
fn max_nat(a: &Nat, b: &Nat) -> Nat {
    match a.cmp_exact(b) {
        Ordering::Less => b.clone(),
        _ => a.clone(),
    }
}
fn stroke_begin_price(f1: &F1State, stroke: &ConfirmedStroke) -> Option<Int> {
    let index = stroke.geometry.begin_index;
    if index >= f1.confirmed.len() {
        return None;
    }
    let point = &f1.confirmed[index].point;
    let Some(center) = center_position(&f1.combined, point) else {
        return None;
    };
    Some(match point.kind {
        FractalKind::Top => f1.combined[center].high.clone(),
        FractalKind::Bottom => f1.combined[center].low.clone(),
    })
}
#[doc = " Uses actual stroke intervals and the four conjuncts of PrimaryHasActualBreakAtLedger."]
pub fn actual_break(
    f1: &F1State,
    strokes: &Vec<ConfirmedStroke>,
    split: usize,
    center_source: usize,
    index: usize,
    direction: &SegmentDirection,
) -> bool {
    if split < 2
        || split > center_source
        || center_source >= index
        || split > index
        || index - split < 2
        || index >= strokes.len()
    {
        return false;
    }
    let Some(first) = stroke_interval(f1, &strokes[split]) else {
        return false;
    };
    let Some(second) = stroke_interval(f1, &strokes[split + 1]) else {
        return false;
    };
    let Some(third) = stroke_interval(f1, &strokes[split + 2]) else {
        return false;
    };
    let Some(center) = stroke_interval(f1, &strokes[center_source]) else {
        return false;
    };
    let Some(challenge) = stroke_interval(f1, &strokes[index]) else {
        return false;
    };
    let Some(previous) = stroke_interval(f1, &strokes[split - 2]) else {
        return false;
    };
    let overlap_low = max_int(&max_int(&first.low, &second.low), &third.low);
    let overlap_high = min_int(&min_int(&first.high, &second.high), &third.high);
    if !le(&overlap_low, &overlap_high) {
        return false;
    }
    let center_break = match direction {
        SegmentDirection::Up => less(&challenge.low, &center.low),
        SegmentDirection::Down => greater(&challenge.high, &center.high),
    };
    center_break
        && first_stroke_directional_resolution(&previous, &first, &challenge, direction)
        && initial_three_direction_resolved(&first, &third, &challenge, direction)
}
pub fn first_stroke_directional_resolution(
    previous: &PriceInterval,
    first: &PriceInterval,
    challenge: &PriceInterval,
    direction: &SegmentDirection,
) -> bool {
    let prior_break = match direction {
        SegmentDirection::Up => less(&first.low, &previous.low),
        SegmentDirection::Down => greater(&first.high, &previous.high),
    };
    let first_break = match direction {
        SegmentDirection::Up => less(&challenge.low, &first.low),
        SegmentDirection::Down => greater(&challenge.high, &first.high),
    };
    !prior_break || first_break
}
pub fn initial_three_direction_resolved(
    first: &PriceInterval,
    third: &PriceInterval,
    challenge: &PriceInterval,
    direction: &SegmentDirection,
) -> bool {
    let first_break = match direction {
        SegmentDirection::Up => less(&challenge.low, &first.low),
        SegmentDirection::Down => greater(&challenge.high, &first.high),
    };
    let third_break = match direction {
        SegmentDirection::Up => less(&third.low, &first.low),
        SegmentDirection::Down => greater(&third.high, &first.high),
    };
    third_break || first_break
}
fn primary_tail_target(primary: &Vec<FeatureElement>, direction: &SegmentDirection) -> bool {
    if primary.len() < 3 {
        return false;
    }
    let len = primary.len();
    primary_target(
        &primary[len - 3],
        &primary[len - 2],
        &primary[len - 1],
        direction,
    )
}
#[doc = " The right equal extreme is checked before an inclusive interval can absorb it."]
fn advance_split_primary(
    primary: Vec<FeatureElement>,
    feature: &FeatureElement,
    direction: &SegmentDirection,
    split: usize,
) -> Vec<FeatureElement> {
    if primary.len() == 2 {
        let mut appended = Vec::new();
        appended.push(copy_feature(&primary[0]));
        appended.push(copy_feature(&primary[1]));
        appended.push(copy_feature(feature));
        if primary_tail_target(&appended, direction) {
            return appended;
        }
    }
    normalized_push(primary, copy_feature(feature), direction, split)
}
fn second_target_after(
    previous_count: usize,
    second: &Vec<FeatureElement>,
    old: Option<usize>,
    direction: &SegmentDirection,
) -> Option<usize> {
    match old {
        Some(middle)
            if second.len() == previous_count
                && second.len() >= 3
                && middle == second.len() - 2 =>
        {
            if standard_second_target(
                &second[middle - 1],
                &second[middle],
                &second[middle + 1],
                direction,
            ) {
                Some(middle)
            } else {
                None
            }
        }
        Some(middle) => Some(middle),
        None => {
            if second.len() >= 3 {
                let middle = second.len() - 2;
                if standard_second_target(
                    &second[middle - 1],
                    &second[middle],
                    &second[middle + 1],
                    direction,
                ) {
                    return Some(middle);
                }
            }
            None
        }
    }
}
fn recognition(
    split: usize,
    primary: &Vec<FeatureElement>,
    candidate: &Nat,
    known: &Nat,
    reason: RecognitionReason,
    witness: ActualBreakWitness,
) -> Recognition {
    let mut window = Vec::new();
    let mut index = 0usize;
    while index < primary.len() {
        window.push(copy_feature(&primary[index]));
        index += 1;
    }
    Recognition {
        split_stroke_index: split,
        window,
        candidate_known_at: candidate.clone(),
        confirmed_known_at: known.clone(),
        reason,
        actual_break: witness,
    }
}
pub fn advance_probe(
    mut probe: PrimarySplitProbe,
    feature: &FeatureElement,
    opposes: bool,
    direction: &SegmentDirection,
    known_at: &Nat,
    has_actual_break: bool,
) -> ProbeStep {
    if probe.primary.len() < 2 {
        return ProbeStep::Discarded;
    }
    let previous_second_count = probe.second.len();
    if !opposes {
        probe.second = normalized_push(
            probe.second,
            copy_feature(feature),
            &opposite(direction),
            probe.split_stroke_index,
        );
        probe.second_target = second_target_after(
            previous_second_count,
            &probe.second,
            probe.second_target,
            direction,
        );
    }
    let center = &probe.primary[1];
    let invalidated = match direction {
        SegmentDirection::Up => greater(&feature.interval.high, &center.interval.high),
        SegmentDirection::Down => less(&feature.interval.low, &center.interval.low),
    };
    let evidence = match &probe.stage {
        ProbeStage::SeekingSecond {
            actual_break: Some(value),
            ..
        } => Some(ActualBreakWitness {
            center_source_stroke_index: value.center_source_stroke_index,
            observed_stroke_index: value.observed_stroke_index,
        }),
        _ if has_actual_break => Some(ActualBreakWitness {
            center_source_stroke_index: center.source_stroke_index,
            observed_stroke_index: feature.source_stroke_index,
        }),
        _ => None,
    };
    let stage = probe.stage;
    match stage {
        ProbeStage::SeekingSecond {
            candidate_known_at, ..
        } => {
            if probe.second_target.is_some() {
                if let Some(witness) = evidence {
                    return ProbeStep::Recognized(recognition(
                        probe.split_stroke_index,
                        &probe.primary,
                        &candidate_known_at,
                        known_at,
                        RecognitionReason::RepairedBySecondSequence,
                        witness,
                    ));
                }
            }
            if invalidated {
                return ProbeStep::Discarded;
            }
            probe.stage = ProbeStage::SeekingSecond {
                candidate_known_at,
                actual_break: evidence,
            };
            ProbeStep::Waiting(probe)
        }
        ProbeStage::SeekingActualBreak { candidate_known_at } => {
            if invalidated {
                return ProbeStep::Discarded;
            }
            if let Some(witness) = evidence {
                ProbeStep::Recognized(recognition(
                    probe.split_stroke_index,
                    &probe.primary,
                    &candidate_known_at,
                    known_at,
                    RecognitionReason::ImmediateNoGap,
                    witness,
                ))
            } else {
                probe.stage = ProbeStage::SeekingActualBreak { candidate_known_at };
                ProbeStep::Waiting(probe)
            }
        }
        ProbeStage::SeekingPrimary => {
            if invalidated {
                return ProbeStep::Discarded;
            }
            if !opposes {
                probe.stage = ProbeStage::SeekingPrimary;
                return ProbeStep::Waiting(probe);
            }
            probe.primary =
                advance_split_primary(probe.primary, feature, direction, probe.split_stroke_index);
            if probe.primary.len() > 3 {
                return ProbeStep::Discarded;
            }
            if probe.primary.len() == 3 && primary_tail_target(&probe.primary, direction) {
                let no_gap =
                    intervals_intersect(&probe.primary[0].interval, &probe.primary[1].interval);
                if no_gap {
                    if let Some(witness) = evidence {
                        return ProbeStep::Recognized(recognition(
                            probe.split_stroke_index,
                            &probe.primary,
                            known_at,
                            known_at,
                            RecognitionReason::ImmediateNoGap,
                            witness,
                        ));
                    }
                    probe.stage = ProbeStage::SeekingActualBreak {
                        candidate_known_at: known_at.clone(),
                    };
                } else {
                    if probe.second_target.is_some() {
                        if let Some(witness) = evidence {
                            return ProbeStep::Recognized(recognition(
                                probe.split_stroke_index,
                                &probe.primary,
                                known_at,
                                known_at,
                                RecognitionReason::RepairedBySecondSequence,
                                witness,
                            ));
                        }
                    }
                    probe.stage = ProbeStage::SeekingSecond {
                        candidate_known_at: known_at.clone(),
                        actual_break: evidence,
                    };
                }
            } else {
                probe.stage = ProbeStage::SeekingPrimary;
            }
            ProbeStep::Waiting(probe)
        }
    }
}
fn span_range(
    f1: &F1State,
    strokes: &Vec<ConfirmedStroke>,
    first: usize,
    end: usize,
) -> Option<PriceInterval> {
    if first > end || end >= strokes.len() {
        return None;
    }
    if first == end {
        let price = stroke_begin_price(f1, &strokes[end])?;
        return Some(PriceInterval {
            low: price.clone(),
            high: price,
        });
    }
    let mut range = stroke_interval(f1, &strokes[first])?;
    let mut index = first + 1;
    while index < end {
        let next = stroke_interval(f1, &strokes[index])?;
        range = PriceInterval {
            low: min_int(&range.low, &next.low),
            high: max_int(&range.high, &next.high),
        };
        index += 1;
    }
    let tail_price = stroke_begin_price(f1, &strokes[end])?;
    Some(PriceInterval {
        low: min_int(&range.low, &tail_price),
        high: max_int(&range.high, &tail_price),
    })
}
fn build_segment(
    f1: &F1State,
    strokes: &Vec<ConfirmedStroke>,
    completed: &Vec<ConfirmedSegment>,
    pending: &SegmentPending,
    recognized: Recognition,
) -> Option<ConfirmedSegment> {
    if pending.begin_stroke_index >= strokes.len()
        || recognized.split_stroke_index >= strokes.len()
        || pending.begin_stroke_index >= recognized.split_stroke_index
    {
        return None;
    }
    let start_price = stroke_begin_price(f1, &strokes[pending.begin_stroke_index])?;
    let end_price = stroke_begin_price(f1, &strokes[recognized.split_stroke_index])?;
    let advancing = match pending.direction {
        SegmentDirection::Up => less(&start_price, &end_price),
        SegmentDirection::Down => greater(&start_price, &end_price),
    };
    if !advancing {
        return None;
    }
    let span = span_range(
        f1,
        strokes,
        pending.begin_stroke_index,
        recognized.split_stroke_index,
    )?;
    let price_interval = PriceInterval {
        low: min_int(&min_int(&span.low, &start_price), &end_price),
        high: max_int(&max_int(&span.high, &start_price), &end_price),
    };
    let previous_known = match completed.last() {
        Some(value) => value.times.confirmed_at.clone(),
        None => Nat::zero(),
    };
    let confirmed_at = max_nat(
        &max_nat(
            &recognized.candidate_known_at,
            &recognized.confirmed_known_at,
        ),
        &previous_known,
    );
    Some(ConfirmedSegment {
        ordinal: completed.len(),
        begin_stroke_index: pending.begin_stroke_index,
        end_stroke_index: recognized.split_stroke_index,
        direction: match pending.direction {
            SegmentDirection::Up => SegmentDirection::Up,
            SegmentDirection::Down => SegmentDirection::Down,
        },
        elements: recognized.window,
        price_interval,
        start_price,
        end_price,
        times: SegmentTimes {
            occurred_at: recognized.candidate_known_at.clone(),
            candidate_known_at: recognized.candidate_known_at,
            confirmed_at,
        },
        reason: recognized.reason,
    })
}
fn copy_probe(probe: &PrimarySplitProbe) -> PrimarySplitProbe {
    let mut primary = Vec::new();
    let mut index = 0usize;
    while index < probe.primary.len() {
        primary.push(copy_feature(&probe.primary[index]));
        index += 1;
    }
    let mut second = Vec::new();
    let mut index = 0usize;
    while index < probe.second.len() {
        second.push(copy_feature(&probe.second[index]));
        index += 1;
    }
    let stage = match &probe.stage {
        ProbeStage::SeekingPrimary => ProbeStage::SeekingPrimary,
        ProbeStage::SeekingActualBreak { candidate_known_at } => ProbeStage::SeekingActualBreak {
            candidate_known_at: candidate_known_at.clone(),
        },
        ProbeStage::SeekingSecond {
            candidate_known_at,
            actual_break,
        } => ProbeStage::SeekingSecond {
            candidate_known_at: candidate_known_at.clone(),
            actual_break: match actual_break {
                Some(value) => Some(ActualBreakWitness {
                    center_source_stroke_index: value.center_source_stroke_index,
                    observed_stroke_index: value.observed_stroke_index,
                }),
                None => None,
            },
        },
    };
    PrimarySplitProbe {
        split_stroke_index: probe.split_stroke_index,
        primary,
        second,
        second_target: probe.second_target,
        stage,
    }
}
pub fn copy_segment(value: &ConfirmedSegment) -> ConfirmedSegment {
    let mut elements = Vec::new();
    let mut index = 0usize;
    while index < value.elements.len() {
        elements.push(copy_feature(&value.elements[index]));
        index += 1;
    }
    ConfirmedSegment {
        ordinal: value.ordinal,
        begin_stroke_index: value.begin_stroke_index,
        end_stroke_index: value.end_stroke_index,
        direction: match value.direction {
            SegmentDirection::Up => SegmentDirection::Up,
            SegmentDirection::Down => SegmentDirection::Down,
        },
        elements,
        price_interval: copy_interval(&value.price_interval),
        start_price: value.start_price.clone(),
        end_price: value.end_price.clone(),
        times: SegmentTimes {
            occurred_at: value.times.occurred_at.clone(),
            candidate_known_at: value.times.candidate_known_at.clone(),
            confirmed_at: value.times.confirmed_at.clone(),
        },
        reason: match value.reason {
            RecognitionReason::ImmediateNoGap => RecognitionReason::ImmediateNoGap,
            RecognitionReason::RepairedBySecondSequence => {
                RecognitionReason::RepairedBySecondSequence
            }
        },
    }
}
#[doc = " Preserve the pre-advance state for fail-closed pipeline errors."]
pub fn copy_segment_state(value: &SegmentState) -> SegmentState {
    let mut completed: Vec<ConfirmedSegment> = Vec::new();
    let mut index = 0usize;
    while index < value.completed.len() {
        completed.push(copy_segment(&value.completed[index]));
        index += 1;
    }
    let pending = match &value.pending {
        None => None,
        Some(old) => {
            let mut primary = Vec::new();
            let mut index = 0usize;
            while index < old.primary.len() {
                primary.push(copy_feature(&old.primary[index]));
                index += 1;
            }
            let mut probes = Vec::new();
            let mut index = 0usize;
            while index < old.probes.len() {
                probes.push(copy_probe(&old.probes[index]));
                index += 1;
            }
            Some(SegmentPending {
                begin_stroke_index: old.begin_stroke_index,
                direction: match old.direction {
                    SegmentDirection::Up => SegmentDirection::Up,
                    SegmentDirection::Down => SegmentDirection::Down,
                },
                primary,
                known_at: old.known_at.clone(),
                probes,
            })
        }
    };
    SegmentState {
        completed,
        pending,
        next_stroke_index: value.next_stroke_index,
    }
}
#[doc = " One confirmed stroke advances every independent split probe before the active primary."]
fn step_segment(
    mut state: SegmentState,
    f1: &F1State,
    strokes: &Vec<ConfirmedStroke>,
) -> Result<(SegmentState, Option<ConfirmedSegment>), (SegmentState, SegmentError)> {
    let index = state.next_stroke_index;
    if index >= strokes.len() {
        return Err((state, SegmentError::InvalidSegmentState));
    }
    if state.pending.is_none() {
        let Some(direction) = stroke_direction(f1, &strokes[index]) else {
            return Err((state, SegmentError::InvalidStrokeSource));
        };
        state.pending = Some(SegmentPending {
            begin_stroke_index: index,
            direction,
            primary: Vec::new(),
            known_at: strokes[index].confirmed_known_at.clone(),
            probes: Vec::new(),
        });
        state.next_stroke_index += 1;
        return Ok((state, None));
    }
    let mut pending = state.pending.take().unwrap();
    if pending.begin_stroke_index >= strokes.len() {
        state.pending = Some(pending);
        return Err((state, SegmentError::InvalidSegmentState));
    }
    let Some(direction) = stroke_direction(f1, &strokes[index]) else {
        state.pending = Some(pending);
        return Err((state, SegmentError::InvalidStrokeSource));
    };
    let Some(feature) = feature_for_stroke(f1, &strokes[index], index) else {
        state.pending = Some(pending);
        return Err((state, SegmentError::InvalidStrokeSource));
    };
    let opposes = stroke_opposes(&pending.direction, &direction);
    let known_at = max_nat(&pending.known_at, &strokes[index].confirmed_known_at);
    let mut next_probes = Vec::new();
    let mut selected = None;
    let mut probe_index = 0usize;
    while probe_index < pending.probes.len() {
        let probe = copy_probe(&pending.probes[probe_index]);
        if probe.primary.len() < 2 {
            state.pending = Some(pending);
            return Err((state, SegmentError::InvalidSegmentState));
        }
        let center_source = probe.primary[1].source_stroke_index;
        let actual = actual_break(
            f1,
            strokes,
            probe.split_stroke_index,
            center_source,
            index,
            &pending.direction,
        );
        match advance_probe(
            probe,
            &feature,
            opposes,
            &pending.direction,
            &known_at,
            actual,
        ) {
            ProbeStep::Discarded => {}
            ProbeStep::Waiting(waiting) => next_probes.push(waiting),
            ProbeStep::Recognized(found) => {
                if found.window.len() != 3 {
                    state.pending = Some(pending);
                    return Err((state, SegmentError::InvalidSegmentState));
                }
                if selected.is_none() {
                    let start_price = stroke_begin_price(f1, &strokes[pending.begin_stroke_index]);
                    if let Some(start_price) = start_price {
                        let endpoint = match pending.direction {
                            SegmentDirection::Up => &found.window[1].interval.high,
                            SegmentDirection::Down => &found.window[1].interval.low,
                        };
                        let advances = match pending.direction {
                            SegmentDirection::Up => less(&start_price, endpoint),
                            SegmentDirection::Down => greater(&start_price, endpoint),
                        };
                        if advances {
                            selected = Some(found);
                        }
                    }
                }
            }
        }
        probe_index += 1;
    }
    if let Some(recognized) = selected {
        let Some(segment) = build_segment(f1, strokes, &state.completed, &pending, recognized)
        else {
            state.pending = Some(pending);
            return Err((state, SegmentError::InvalidStrokeSource));
        };
        let end_index = segment.end_stroke_index;
        if end_index == 0 || end_index > index {
            state.pending = Some(pending);
            return Err((state, SegmentError::InvalidSegmentState));
        }
        if let Some(last) = state.completed.last() {
            if end_index <= last.end_stroke_index {
                state.pending = Some(pending);
                return Err((state, SegmentError::InvalidSegmentState));
            }
        }
        state.completed.push(copy_segment(&segment));
        state.pending = None;
        state.next_stroke_index = end_index;
        return Ok((state, Some(segment)));
    }
    if opposes && !pending.primary.is_empty() {
        let mut primary = Vec::new();
        primary.push(copy_feature(pending.primary.last().unwrap()));
        primary.push(copy_feature(&feature));
        next_probes.push(PrimarySplitProbe {
            split_stroke_index: index,
            primary,
            second: Vec::new(),
            second_target: None,
            stage: ProbeStage::SeekingPrimary,
        });
    }
    if opposes {
        pending.primary = normalized_push(
            pending.primary,
            feature,
            &pending.direction,
            pending.begin_stroke_index,
        );
    }
    pending.known_at = known_at;
    pending.probes = next_probes;
    state.pending = Some(pending);
    state.next_stroke_index += 1;
    Ok((state, None))
}
#[doc = " A recognized endpoint may precede the current stroke, so replay begins there."]
pub fn advance_segments(
    mut state: SegmentState,
    f1: &F1State,
    strokes: &StrokeState,
) -> Result<(SegmentState, Vec<ConfirmedSegment>), (SegmentState, SegmentError)> {
    let mut emitted = Vec::new();
    while state.next_stroke_index < strokes.confirmed.len() {
        match step_segment(state, f1, &strokes.confirmed) {
            Ok((next, Some(segment))) => {
                state = next;
                emitted.push(segment);
            }
            Ok((next, None)) => state = next,
            Err((preserved, reason)) => return Err((preserved, reason)),
        }
    }
    Ok((state, emitted))
}
