//! 统一推进去包含、分型和笔；每次只接受一根新闭合的质量材料。

use crate::f1::fractal::advance_f1;
use crate::f1::stroke::{ConfirmedStroke, StrokeState, accept_confirmed_point, new_stroke_state};
use crate::f1::{F1Delta, F1InputError, F1State, new_f1_state};
use crate::input::{BarStreamIdentity, MarketDirection, QualityBar};

/// 形态层的唯一组合状态。
#[derive(Clone, Debug, PartialEq)]
pub struct MorphologyState {
    /// 去包含和分型账本。
    pub f1: F1State,
    /// 已确认笔及活动笔状态。
    pub stroke: StrokeState,
}
/// 单根闭合 Bar 的形态增量。
#[derive(Clone, Debug, PartialEq)]
pub struct MorphologyDelta {
    /// 去包含和分型变化。
    pub f1: F1Delta,
    /// 本次反向笔成立后新确认的前一笔。
    pub confirmed_stroke: Option<ConfirmedStroke>,
}
/// 建立空形态状态，并明确首次去包含方向。
pub fn new_morphology_state(
    stream: BarStreamIdentity,
    initial_direction: MarketDirection,
) -> MorphologyState {
    MorphologyState {
        f1: new_f1_state(stream, initial_direction),
        stroke: new_stroke_state(),
    }
}
/// 先推进分型层；仅在新增确认分型时推进笔层。输入拒绝时返回原状态。
#[expect(
    clippy::result_large_err,
    reason = "成功分支已经携带更大的状态与增量；错误按值归还状态，避免额外分配"
)]
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
/// 按顺序折叠闭合 Bar 切片；遇错返回失败前状态、切片内失败下标及原因。
#[expect(
    clippy::result_large_err,
    reason = "成功分支已经携带更大的状态与增量；错误按值归还状态，避免额外分配"
)]
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
