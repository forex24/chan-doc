//! F2EndToEndPipelineMapping's level-zero movement projection and append ledger.

use crate::f1::feature::{PriceInterval, SegmentDirection, copy_interval};
use crate::f1::morphology::{
    MorphologyDelta, MorphologyState, advance_morphology, new_morphology_state,
};
use crate::f1::segment::{
    ConfirmedSegment, SegmentError, SegmentState, advance_segments, copy_segment_state,
    new_segment_state,
};
use crate::f1::stroke::{StrokeState, center_position};
use crate::f1::{F1InputError, F1State, FractalKind, copy_direction};
use crate::input::{BarStreamIdentity, MarketDirection, QualityBar};
use crate::number::{Int, Nat};
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub struct MarketEndpoint {
    pub market_order: Nat,
    pub open_time: Nat,
    pub price: Int,
}
#[derive(Debug, PartialEq, Eq)]
pub enum MovementComposition {
    FrozenLowestSegment,
    ConsolidationMovement,
    TrendMovement,
}
#[derive(Debug, PartialEq, Eq)]
pub struct MovementKey {
    pub level_ordinal: Nat,
    pub start_endpoint: MarketEndpoint,
    pub end_endpoint: MarketEndpoint,
    pub direction: MarketDirection,
    pub full_range: PriceInterval,
    pub composition: MovementComposition,
}
#[derive(Debug, PartialEq, Eq)]
pub struct KnownCompleteMovement {
    pub key: MovementKey,
    pub direct_materials: Vec<MovementKey>,
    pub known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct CompleteBoundary {
    pub root_point: Nat,
    pub initial_inclusion_direction: MarketDirection,
    pub known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct LowestState {
    pub completed: Vec<KnownCompleteMovement>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct F1PipelineState {
    pub morphology: MorphologyState,
    pub segment: SegmentState,
    pub lowest: LowestState,
    pub left_boundary: CompleteBoundary,
}
#[derive(Debug, PartialEq, Eq)]
pub struct F1PipelineDelta {
    pub morphology: MorphologyDelta,
    pub segments: Vec<ConfirmedSegment>,
    pub lowest_movements: Vec<KnownCompleteMovement>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PipelineError {
    F1(F1InputError),
    Segment(SegmentError),
    LowestMovementInvalid,
}
pub fn movement_key_is_valid(value: &MovementKey) -> bool {
    let direction_matches = match value
        .start_endpoint
        .price
        .cmp_exact(&value.end_endpoint.price)
    {
        Ordering::Less => matches!(value.direction, MarketDirection::Upward),
        Ordering::Greater => matches!(value.direction, MarketDirection::Downward),
        Ordering::Equal => matches!(value.direction, MarketDirection::NoNetDisplacement),
    };
    matches!(
        value
            .start_endpoint
            .market_order
            .cmp_exact(&value.end_endpoint.market_order),
        Ordering::Less
    ) && matches!(
        value
            .start_endpoint
            .open_time
            .cmp_exact(&value.end_endpoint.open_time),
        Ordering::Less
    ) && !matches!(
        value.full_range.low.cmp_exact(&value.full_range.high),
        Ordering::Greater
    ) && !matches!(
        value.full_range.low.cmp_exact(&value.start_endpoint.price),
        Ordering::Greater
    ) && !matches!(
        value.start_endpoint.price.cmp_exact(&value.full_range.high),
        Ordering::Greater
    ) && !matches!(
        value.full_range.low.cmp_exact(&value.end_endpoint.price),
        Ordering::Greater
    ) && !matches!(
        value.end_endpoint.price.cmp_exact(&value.full_range.high),
        Ordering::Greater
    ) && direction_matches
        && (!matches!(value.direction, MarketDirection::NoNetDisplacement)
            || matches!(
                value.composition,
                MovementComposition::ConsolidationMovement
            ))
        && (matches!(value.level_ordinal.cmp_exact(&Nat::zero()), Ordering::Equal)
            == matches!(value.composition, MovementComposition::FrozenLowestSegment))
}
fn copy_endpoint(value: &MarketEndpoint) -> MarketEndpoint {
    MarketEndpoint {
        market_order: value.market_order.clone(),
        open_time: value.open_time.clone(),
        price: value.price.clone(),
    }
}
fn copy_key(value: &MovementKey) -> MovementKey {
    MovementKey {
        level_ordinal: value.level_ordinal.clone(),
        start_endpoint: copy_endpoint(&value.start_endpoint),
        end_endpoint: copy_endpoint(&value.end_endpoint),
        direction: copy_direction(&value.direction),
        full_range: copy_interval(&value.full_range),
        composition: match value.composition {
            MovementComposition::FrozenLowestSegment => MovementComposition::FrozenLowestSegment,
            MovementComposition::ConsolidationMovement => {
                MovementComposition::ConsolidationMovement
            }
            MovementComposition::TrendMovement => MovementComposition::TrendMovement,
        },
    }
}
pub fn copy_movement(value: &KnownCompleteMovement) -> KnownCompleteMovement {
    let mut materials = Vec::new();
    let mut index = 0usize;
    while index < value.direct_materials.len() {
        materials.push(copy_key(&value.direct_materials[index]));
        index += 1;
    }
    KnownCompleteMovement {
        key: copy_key(&value.key),
        direct_materials: materials,
        known_at: value.known_at.clone(),
    }
}
fn less_nat(a: &Nat, b: &Nat) -> bool {
    match a.cmp_exact(b) {
        Ordering::Less => true,
        _ => false,
    }
}
fn le_nat(a: &Nat, b: &Nat) -> bool {
    match a.cmp_exact(b) {
        Ordering::Greater => false,
        _ => true,
    }
}
fn less_int(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Less => true,
        _ => false,
    }
}
fn le_int(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Greater => false,
        _ => true,
    }
}
fn point_endpoint(f1: &F1State, point_index: usize, price: &Int) -> Option<MarketEndpoint> {
    if point_index >= f1.confirmed.len() {
        return None;
    }
    let point = &f1.confirmed[point_index].point;
    let center = center_position(&f1.combined, point)?;
    let open_time = match point.kind {
        FractalKind::Top => f1.combined[center].high_slot.clone(),
        FractalKind::Bottom => f1.combined[center].low_slot.clone(),
    };
    Some(MarketEndpoint {
        market_order: point.center_index.clone(),
        open_time,
        price: price.clone(),
    })
}
pub fn movement_of_segment(
    f1: &F1State,
    strokes: &StrokeState,
    segment: &ConfirmedSegment,
) -> Option<KnownCompleteMovement> {
    if segment.begin_stroke_index >= strokes.confirmed.len()
        || segment.end_stroke_index >= strokes.confirmed.len()
    {
        return None;
    }
    let start_point = strokes.confirmed[segment.begin_stroke_index]
        .geometry
        .begin_index;
    let end_point = strokes.confirmed[segment.end_stroke_index]
        .geometry
        .begin_index;
    let start = point_endpoint(f1, start_point, &segment.start_price)?;
    let end = point_endpoint(f1, end_point, &segment.end_price)?;
    let direction = match segment.direction {
        SegmentDirection::Up => MarketDirection::Upward,
        SegmentDirection::Down => MarketDirection::Downward,
    };
    if !less_nat(&start.market_order, &end.market_order)
        || !less_nat(&start.open_time, &end.open_time)
        || !le_nat(&end.open_time, &segment.times.confirmed_at)
        || !le_int(&segment.price_interval.low, &segment.price_interval.high)
        || !le_int(&segment.price_interval.low, &start.price)
        || !le_int(&start.price, &segment.price_interval.high)
        || !le_int(&segment.price_interval.low, &end.price)
        || !le_int(&end.price, &segment.price_interval.high)
    {
        return None;
    }
    if match direction {
        MarketDirection::Upward => !less_int(&start.price, &end.price),
        MarketDirection::Downward => !less_int(&end.price, &start.price),
        MarketDirection::NoNetDisplacement => true,
    } {
        return None;
    }
    {};
    {};
    {};
    Some(KnownCompleteMovement {
        key: MovementKey {
            level_ordinal: Nat::zero(),
            start_endpoint: start,
            end_endpoint: end,
            direction,
            full_range: copy_interval(&segment.price_interval),
            composition: MovementComposition::FrozenLowestSegment,
        },
        direct_materials: Vec::new(),
        known_at: segment.times.confirmed_at.clone(),
    })
}
pub fn new_pipeline_state(
    stream: BarStreamIdentity,
    boundary: CompleteBoundary,
) -> F1PipelineState {
    let direction = copy_direction(&boundary.initial_inclusion_direction);
    F1PipelineState {
        morphology: new_morphology_state(stream, direction),
        segment: new_segment_state(),
        lowest: LowestState {
            completed: Vec::new(),
        },
        left_boundary: boundary,
    }
}
fn can_append_lowest(
    previous: Option<&KnownCompleteMovement>,
    next: &KnownCompleteMovement,
    boundary: &CompleteBoundary,
) -> bool {
    if let Some(last) = previous {
        last.key.end_endpoint == next.key.start_endpoint
            && last.key.direction != next.key.direction
            && le_nat(&last.known_at, &next.known_at)
    } else {
        boundary.root_point == next.key.start_endpoint.market_order
            && le_nat(&next.key.start_endpoint.open_time, &boundary.known_at)
            && le_nat(&boundary.known_at, &next.known_at)
    }
}
#[doc = " Only the segment delta is mapped and appended; old movement history is never rebuilt."]
pub fn advance_pipeline(
    state: F1PipelineState,
    bar: &QualityBar,
) -> Result<(F1PipelineState, F1PipelineDelta), (F1PipelineState, PipelineError)> {
    let morphology = state.morphology;
    let segment = state.segment;
    let lowest = state.lowest;
    let left_boundary = state.left_boundary;
    let (morphology, morphology_delta) = match advance_morphology(morphology, bar) {
        Ok(value) => value,
        Err((preserved, reason)) => {
            return Err((
                F1PipelineState {
                    morphology: preserved,
                    segment,
                    lowest,
                    left_boundary,
                },
                PipelineError::F1(reason),
            ));
        }
    };
    let preserved_segment = copy_segment_state(&segment);
    let (segment, segments) = match advance_segments(segment, &morphology.f1, &morphology.stroke) {
        Ok(value) => value,
        Err((_, reason)) => {
            return Err((
                F1PipelineState {
                    morphology,
                    segment: preserved_segment,
                    lowest,
                    left_boundary,
                },
                PipelineError::Segment(reason),
            ));
        }
    };
    let mut movement_delta = Vec::new();
    let mut index = 0usize;
    while index < segments.len() {
        let Some(next) = movement_of_segment(&morphology.f1, &morphology.stroke, &segments[index])
        else {
            return Err((
                F1PipelineState {
                    morphology,
                    segment: preserved_segment,
                    lowest,
                    left_boundary,
                },
                PipelineError::LowestMovementInvalid,
            ));
        };
        let preceding = match movement_delta.last() {
            Some(value) => Some(value),
            None => lowest.completed.last(),
        };
        if !can_append_lowest(preceding, &next, &left_boundary) {
            return Err((
                F1PipelineState {
                    morphology,
                    segment: preserved_segment,
                    lowest,
                    left_boundary,
                },
                PipelineError::LowestMovementInvalid,
            ));
        }
        movement_delta.push(next);
        index += 1;
    }
    let mut lowest = lowest;
    let mut index = 0usize;
    while index < movement_delta.len() {
        lowest.completed.push(copy_movement(&movement_delta[index]));
        index += 1;
    }
    Ok((
        F1PipelineState {
            morphology,
            segment,
            lowest,
            left_boundary,
        },
        F1PipelineDelta {
            morphology: morphology_delta,
            segments,
            lowest_movements: movement_delta,
        },
    ))
}
