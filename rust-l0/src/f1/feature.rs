//! F1Segment feature intervals and exact observed-stroke normalization.

use crate::f1::stroke::{ConfirmedStroke, center_position};
use crate::f1::{F1State, FractalKind};
use crate::number::Int;
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub enum SegmentDirection {
    Up,
    Down,
}
#[derive(Debug, PartialEq, Eq)]
pub struct PriceInterval {
    pub low: Int,
    pub high: Int,
}
#[doc = " `source_stroke_indices` is the complete ordered fold, not only its last edge."]
#[derive(Debug, PartialEq, Eq)]
pub struct FeatureElement {
    pub source_stroke_indices: Vec<usize>,
    pub source_stroke_index: usize,
    pub interval: PriceInterval,
    pub extreme_source_stroke_index: usize,
}
pub fn copy_interval(value: &PriceInterval) -> PriceInterval {
    PriceInterval {
        low: value.low.clone(),
        high: value.high.clone(),
    }
}
pub fn copy_feature(value: &FeatureElement) -> FeatureElement {
    let mut sources = Vec::new();
    let mut index = 0usize;
    while index < value.source_stroke_indices.len() {
        sources.push(value.source_stroke_indices[index]);
        index += 1;
    }
    FeatureElement {
        source_stroke_indices: sources,
        source_stroke_index: value.source_stroke_index,
        interval: copy_interval(&value.interval),
        extreme_source_stroke_index: value.extreme_source_stroke_index,
    }
}
pub fn stroke_direction(f1: &F1State, stroke: &ConfirmedStroke) -> Option<SegmentDirection> {
    let begin = stroke.geometry.begin_index;
    let end = stroke.geometry.end_index;
    if begin >= f1.confirmed.len() || end >= f1.confirmed.len() {
        return None;
    }
    match (
        &f1.confirmed[begin].point.kind,
        &f1.confirmed[end].point.kind,
    ) {
        (FractalKind::Bottom, FractalKind::Top) => Some(SegmentDirection::Up),
        (FractalKind::Top, FractalKind::Bottom) => Some(SegmentDirection::Down),
        _ => None,
    }
}
#[doc = " Read the actual confirmed-stroke endpoint bars; formation geometry is not a proxy."]
pub fn stroke_interval(f1: &F1State, stroke: &ConfirmedStroke) -> Option<PriceInterval> {
    let begin = stroke.geometry.begin_index;
    let end = stroke.geometry.end_index;
    if begin >= f1.confirmed.len() || end >= f1.confirmed.len() {
        return None;
    }
    let Some(begin_center) = center_position(&f1.combined, &f1.confirmed[begin].point) else {
        return None;
    };
    let Some(end_center) = center_position(&f1.combined, &f1.confirmed[end].point) else {
        return None;
    };
    let interval = match stroke_direction(f1, stroke) {
        Some(SegmentDirection::Up) => PriceInterval {
            low: f1.combined[begin_center].low.clone(),
            high: f1.combined[end_center].high.clone(),
        },
        Some(SegmentDirection::Down) => PriceInterval {
            low: f1.combined[end_center].low.clone(),
            high: f1.combined[begin_center].high.clone(),
        },
        None => return None,
    };
    if interval.low.cmp_exact(&interval.high) == Ordering::Greater {
        None
    } else {
        Some(interval)
    }
}
pub fn feature_for_stroke(
    f1: &F1State,
    stroke: &ConfirmedStroke,
    index: usize,
) -> Option<FeatureElement> {
    let Some(interval) = stroke_interval(f1, stroke) else {
        return None;
    };
    Some(FeatureElement {
        source_stroke_indices: vec![index],
        source_stroke_index: index,
        interval,
        extreme_source_stroke_index: index,
    })
}
pub fn opposite(direction: &SegmentDirection) -> SegmentDirection {
    match direction {
        SegmentDirection::Up => SegmentDirection::Down,
        SegmentDirection::Down => SegmentDirection::Up,
    }
}
pub fn stroke_opposes(direction: &SegmentDirection, stroke_direction: &SegmentDirection) -> bool {
    match (direction, stroke_direction) {
        (SegmentDirection::Up, SegmentDirection::Down)
        | (SegmentDirection::Down, SegmentDirection::Up) => true,
        _ => false,
    }
}
fn lower(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Less => true,
        _ => false,
    }
}
fn le(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Greater => false,
        _ => true,
    }
}
fn higher(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Greater => true,
        _ => false,
    }
}
fn ge(a: &Int, b: &Int) -> bool {
    match a.cmp_exact(b) {
        Ordering::Less => false,
        _ => true,
    }
}
pub fn intervals_intersect(a: &PriceInterval, b: &PriceInterval) -> bool {
    le(&a.low, &b.high) && le(&b.low, &a.high)
}
pub fn interval_inclusion(a: &PriceInterval, b: &PriceInterval) -> bool {
    (ge(&a.high, &b.high) && le(&a.low, &b.low)) || (ge(&b.high, &a.high) && le(&b.low, &a.low))
}
fn maximum(a: &Int, b: &Int) -> Int {
    match a.cmp_exact(b) {
        Ordering::Less => b.clone(),
        _ => a.clone(),
    }
}
fn minimum(a: &Int, b: &Int) -> Int {
    match a.cmp_exact(b) {
        Ordering::Greater => b.clone(),
        _ => a.clone(),
    }
}
pub fn merge_interval(
    a: &PriceInterval,
    b: &PriceInterval,
    direction: &SegmentDirection,
) -> PriceInterval {
    match direction {
        SegmentDirection::Up => PriceInterval {
            low: maximum(&a.low, &b.low),
            high: maximum(&a.high, &b.high),
        },
        SegmentDirection::Down => PriceInterval {
            low: minimum(&a.low, &b.low),
            high: minimum(&a.high, &b.high),
        },
    }
}
pub fn normalized_push(
    mut normalized: Vec<FeatureElement>,
    next: FeatureElement,
    direction: &SegmentDirection,
    primary_split: usize,
) -> Vec<FeatureElement> {
    let merge = match normalized.last() {
        Some(last) => {
            interval_inclusion(&last.interval, &next.interval)
                && (last.source_stroke_index < primary_split)
                    == (next.source_stroke_index < primary_split)
        }
        None => false,
    };
    if merge {
        let last = normalized.pop().unwrap();
        let mut sources = last.source_stroke_indices;
        let mut index = 0usize;
        while index < next.source_stroke_indices.len() {
            sources.push(next.source_stroke_indices[index]);
            index += 1;
        }
        let new_extreme = match direction {
            SegmentDirection::Up => higher(&next.interval.high, &last.interval.high),
            SegmentDirection::Down => lower(&next.interval.low, &last.interval.low),
        };
        let interval = merge_interval(&last.interval, &next.interval, direction);
        normalized.push(FeatureElement {
            source_stroke_indices: sources,
            source_stroke_index: next.source_stroke_index,
            interval,
            extreme_source_stroke_index: if new_extreme {
                next.extreme_source_stroke_index
            } else {
                last.extreme_source_stroke_index
            },
        });
    } else {
        normalized.push(next);
    }
    normalized
}
pub fn primary_target(
    left: &FeatureElement,
    middle: &FeatureElement,
    right: &FeatureElement,
    direction: &SegmentDirection,
) -> bool {
    match direction {
        SegmentDirection::Up => {
            higher(&middle.interval.high, &left.interval.high)
                && ge(&middle.interval.high, &right.interval.high)
                && higher(&middle.interval.low, &right.interval.low)
        }
        SegmentDirection::Down => {
            lower(&middle.interval.low, &left.interval.low)
                && lower(&middle.interval.high, &right.interval.high)
                && le(&middle.interval.low, &right.interval.low)
        }
    }
}
pub fn standard_second_target(
    left: &FeatureElement,
    middle: &FeatureElement,
    right: &FeatureElement,
    direction: &SegmentDirection,
) -> bool {
    match direction {
        SegmentDirection::Up => {
            lower(&middle.interval.high, &left.interval.high)
                && lower(&middle.interval.high, &right.interval.high)
                && lower(&middle.interval.low, &left.interval.low)
                && lower(&middle.interval.low, &right.interval.low)
        }
        SegmentDirection::Down => {
            higher(&middle.interval.high, &left.interval.high)
                && higher(&middle.interval.high, &right.interval.high)
                && higher(&middle.interval.low, &left.interval.low)
                && higher(&middle.interval.low, &right.interval.low)
        }
    }
}
