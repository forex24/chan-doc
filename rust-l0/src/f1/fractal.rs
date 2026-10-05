//! Strict two-dimensional fractals and right-neighbor freeze confirmation.

use crate::f1::inclusion::{absorb, combined_absorbs, fresh_combined, single_combined};
use crate::f1::{
    ConfirmedFractal, F1Delta, F1InputError, F1State, FractalKind, FractalPoint, copy_combined,
    copy_point, raw_from_closed,
};
use crate::input::QualityBar;
use crate::number::Nat;
use crate::quality::is_valid_quality_bar;
use std::cmp::Ordering;

pub fn shape_at(bars: &Vec<crate::f1::CombinedBar>, center: usize) -> Option<FractalKind> {
    if bars.len() < 3 || center == 0 || center >= bars.len() - 1 {
        return None;
    }
    let left = &bars[center - 1];
    let middle = &bars[center];
    let right = &bars[center + 1];
    if middle.high.cmp_exact(&left.high) == Ordering::Greater
        && middle.high.cmp_exact(&right.high) == Ordering::Greater
        && middle.low.cmp_exact(&left.low) == Ordering::Greater
        && middle.low.cmp_exact(&right.low) == Ordering::Greater
    {
        Some(FractalKind::Top)
    } else if middle.high.cmp_exact(&left.high) == Ordering::Less
        && middle.high.cmp_exact(&right.high) == Ordering::Less
        && middle.low.cmp_exact(&left.low) == Ordering::Less
        && middle.low.cmp_exact(&right.low) == Ordering::Less
    {
        Some(FractalKind::Bottom)
    } else {
        None
    }
}
pub fn tail_forming(bars: &Vec<crate::f1::CombinedBar>) -> Option<FractalPoint> {
    if bars.len() < 3 {
        return None;
    }
    let center = bars.len() - 2;
    match shape_at(bars, center) {
        Some(kind) => Some(FractalPoint {
            kind,
            center_index: Nat::from_u64(center as u64),
            detected_known_at: bars[center + 1].known_at.clone(),
        }),
        None => None,
    }
}
fn copy_confirmed(value: &ConfirmedFractal) -> ConfirmedFractal {
    ConfirmedFractal {
        point: copy_point(&value.point),
        confirmed_known_at: value.confirmed_known_at.clone(),
    }
}
#[doc = " One call consumes one newly granted closed Bar; rejected calls keep state."]
pub fn advance_f1(
    mut state: F1State,
    bar: &QualityBar,
) -> Result<(F1State, F1Delta), (F1State, F1InputError)> {
    if !bar.closed {
        return Err((state, F1InputError::ActiveBar));
    }
    if bar.stream != state.stream {
        return Err((state, F1InputError::StreamMismatch));
    }
    if !is_valid_quality_bar(bar) {
        return Err((state, F1InputError::InvalidBar));
    }
    if let Some(last) = state.combined.last() {
        match bar.slot.cmp_exact(&last.last_slot) {
            Ordering::Greater => {}
            _ => return Err((state, F1InputError::OutOfOrder)),
        }
    }
    let raw = raw_from_closed(bar);
    let mut finalized = None;
    let mut confirmed = None;
    let mut tail_updated = false;
    if state.combined.is_empty() {
        state
            .combined
            .push(single_combined(&raw, &state.initial_direction));
    } else {
        let last = state.combined.last().unwrap();
        if combined_absorbs(last, &raw) {
            let replacement = absorb(last, &raw);
            state.combined.pop();
            state.combined.push(replacement);
            tail_updated = true;
        } else {
            let next = fresh_combined(last, &raw);
            finalized = Some(copy_combined(last));
            if let Some(point) = state.forming.take() {
                let fixed = ConfirmedFractal {
                    point,
                    confirmed_known_at: next.formed_known_at.clone(),
                };
                state.confirmed.push(copy_confirmed(&fixed));
                confirmed = Some(fixed);
            }
            state.combined.push(next);
        }
    }
    state.forming = tail_forming(&state.combined);
    let forming = match &state.forming {
        Some(point) => Some(copy_point(point)),
        None => None,
    };
    {};
    {};
    {};
    {};
    {};
    {};
    {};
    Ok((
        state,
        F1Delta {
            finalized_combined: finalized,
            confirmed_fractal: confirmed,
            forming,
            tail_updated,
        },
    ))
}
