//! Single F1 owner: closed quality material -> inclusion, fractal and stroke deltas.

use crate::f1::fractal::advance_f1;
use crate::f1::stroke::{ConfirmedStroke, StrokeState, accept_confirmed_point, new_stroke_state};
use crate::f1::{F1Delta, F1InputError, F1State, new_f1_state};
use crate::input::{BarStreamIdentity, MarketDirection, QualityBar};

#[derive(Debug, PartialEq, Eq)]
pub struct MorphologyState {
    pub f1: F1State,
    pub stroke: StrokeState,
}
#[derive(Debug, PartialEq, Eq)]
pub struct MorphologyDelta {
    pub f1: F1Delta,
    pub confirmed_stroke: Option<ConfirmedStroke>,
}
pub fn new_morphology_state(
    stream: BarStreamIdentity,
    initial_direction: MarketDirection,
) -> MorphologyState {
    MorphologyState {
        f1: new_f1_state(stream, initial_direction),
        stroke: new_stroke_state(),
    }
}
pub fn advance_morphology(
    state: MorphologyState,
    bar: &QualityBar,
) -> Result<(MorphologyState, MorphologyDelta), (MorphologyState, F1InputError)> {
    let f1 = state.f1;
    let stroke = state.stroke;
    match advance_f1(f1, bar) {
        Err((f1, reason)) => Err((MorphologyState { f1, stroke }, reason)),
        Ok((f1, delta)) => {
            let (stroke, confirmed_stroke) = match &delta.confirmed_fractal {
                Some(point) => accept_confirmed_point(stroke, &f1, point),
                None => (stroke, None),
            };
            Ok((
                MorphologyState { f1, stroke },
                MorphologyDelta {
                    f1: delta,
                    confirmed_stroke,
                },
            ))
        }
    }
}
pub fn fold_morphology(
    mut state: MorphologyState,
    bars: &[QualityBar],
) -> Result<(MorphologyState, Vec<MorphologyDelta>), (MorphologyState, usize, F1InputError)> {
    let mut deltas = Vec::new();
    let mut index = 0usize;
    while index < bars.len() {
        match advance_morphology(state, &bars[index]) {
            Ok((next, delta)) => {
                state = next;
                deltas.push(delta);
            }
            Err((preserved, reason)) => return Err((preserved, index, reason)),
        }
        index += 1;
    }
    Ok((state, deltas))
}
