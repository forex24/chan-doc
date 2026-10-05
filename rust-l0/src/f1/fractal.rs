//! 严格二维分型：中间 K 线高低价均胜过左右邻居，右邻冻结后才确认。

use crate::f1::inclusion::{absorb, combined_absorbs, fresh_combined, single_combined};
use crate::f1::{
    ConfirmedFractal, F1Delta, F1InputError, F1State, FractalKind, FractalPoint, raw_from_closed,
};
use crate::input::{MarketDirection, QualityBar};
use crate::quality::is_valid_quality_bar;
use std::cmp::Ordering;

/// 检查中心及左右邻居；高低价均严格较高为顶，均严格较低为底，越界或等号不成型。
pub fn shape_at(bars: &[crate::f1::CombinedBar], center: usize) -> Option<FractalKind> {
    if bars.len() < 3 || center == 0 || center >= bars.len() - 1 {
        return None;
    }
    let left = &bars[center - 1];
    let middle = &bars[center];
    let right = &bars[center + 1];
    if middle.high > left.high
        && middle.high > right.high
        && middle.low > left.low
        && middle.low > right.low
    {
        Some(FractalKind::Top)
    } else if middle.high < left.high
        && middle.high < right.high
        && middle.low < left.low
        && middle.low < right.low
    {
        Some(FractalKind::Bottom)
    } else {
        None
    }
}
/// 只检测倒数第二根为中心的尾部分型；右邻仍活动，因此只返回候选。
pub fn tail_forming(bars: &[crate::f1::CombinedBar]) -> Option<FractalPoint> {
    if bars.len() < 3 {
        return None;
    }
    let center = bars.len() - 2;
    shape_at(bars, center).map(|kind| FractalPoint {
        kind,
        center_index: center,
        detected_known_at: bars[center + 1].known_at,
    })
}
/// 消费一根新获准的闭合 Bar；先校验输入，拒绝时原样返回状态。
#[expect(
    clippy::result_large_err,
    reason = "成功分支已经携带更大的状态与增量；错误按值归还状态，避免额外分配"
)]
pub fn advance_f1(
    mut state: F1State,
    bar: &QualityBar,
) -> Result<(F1State, F1Delta), (F1State, F1InputError)> {
    if state.initial_direction == MarketDirection::NoNetDisplacement {
        return Err((state, F1InputError::InvalidInitialDirection));
    }
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
        match bar.slot.cmp(&last.last_slot) {
            Ordering::Greater => {}
            _ => return Err((state, F1InputError::OutOfOrder)),
        }
    }
    let raw = raw_from_closed(bar);
    state.raw.push(raw);
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
            finalized = Some(*last);
            if let Some(point) = state.forming.take() {
                let fixed = ConfirmedFractal {
                    point,
                    confirmed_known_at: next.formed_known_at,
                };
                state.confirmed.push(fixed);
                confirmed = Some(fixed);
            }
            state.combined.push(next);
        }
    }
    state.forming = tail_forming(&state.combined);
    let forming = state.forming;

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
