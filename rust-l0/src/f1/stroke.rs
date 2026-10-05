//! 在已确认分型账本上增量构建笔；同类极值延伸，合格的反向笔确认前笔。

use crate::f1::{CombinedBar, ConfirmedFractal, F1State, FractalKind, FractalPoint};
use chrono::{DateTime, Utc};

/// 笔的分型几何下标；三个下标都指向 F1State.confirmed。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrokeGeometry {
    /// 起点分型下标。
    pub begin_index: usize,
    /// 首次成笔时的终点分型下标，后续延伸不修改它。
    pub formation_end_index: usize,
    /// 当前终点分型下标，可因同类更强极值而后移。
    pub end_index: usize,
}
/// 已成形但尚未被反向笔确认的活动笔。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TentativeStroke {
    /// 初始与当前成笔几何。
    pub geometry: StrokeGeometry,
    /// 首次成笔所依赖 K 线的最大获知时间。
    pub formed_known_at: DateTime<Utc>,
    /// 考虑端点延伸后的最大获知时间。
    pub current_known_at: DateTime<Utc>,
}
/// 被合格反向笔确认后冻结的笔。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfirmedStroke {
    /// 冻结时的成笔几何。
    pub geometry: StrokeGeometry,
    /// 首次成笔获知时间。
    pub formed_known_at: DateTime<Utc>,
    /// 冻结前最后一次端点更新的获知时间。
    pub current_known_at: DateTime<Utc>,
    /// 使反向笔成立的确认分型下标。
    pub confirmed_by_point_index: usize,
    /// 前笔可被发布的确认时间。
    pub confirmed_known_at: DateTime<Utc>,
}
/// 成笔阶段；反向笔与前笔确认在同一次调用内完成。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokePhase {
    /// 尚无可用确认分型。
    AwaitingFirstPoint,
    /// 已有锚点，等待合格的异类分型。
    SeekingFirstStroke {
        /// 首笔起点候选在确认分型账本中的下标。
        anchor_point_index: usize,
    },
    /// 正在延伸已成形的活动笔。
    BuildingStroke {
        /// 唯一活动笔。
        active: TentativeStroke,
    },
}
/// 笔的增量状态和不可改写的确认前缀。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrokeState {
    /// 按发布顺序排列的已确认笔。
    pub confirmed: Vec<ConfirmedStroke>,
    /// 当前成笔阶段。
    pub phase: StrokePhase,
    /// 已经处理的确认分型数，也是下一次待消费的分型下标。
    pub processed_point_count: usize,
}
/// 建立等待首个确认分型的笔状态。
pub fn new_stroke_state() -> StrokeState {
    StrokeState {
        confirmed: Vec::new(),
        phase: StrokePhase::AwaitingFirstPoint,
        processed_point_count: 0,
    }
}
/// 下标直接寻址，越界时返回 None；不再构造大整数并线性扫描数组。
pub(crate) fn center_position(bars: &[CombinedBar], point: &FractalPoint) -> Option<usize> {
    (point.center_index < bars.len()).then_some(point.center_index)
}
/// 比较同类分型的端点极值；必须严格更高或更低，同价不后移。
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
        FractalKind::Top => f1.combined[candidate_center].high > f1.combined[current_center].high,
        FractalKind::Bottom => f1.combined[candidate_center].low < f1.combined[current_center].low,
    }
}
/// 验证异类分型、中心间距至少 4、三 K 邻域严格极值以及内部 K 线不越过终点。
fn can_form_stroke(f1: &F1State, begin_index: usize, end_index: usize) -> Option<(usize, usize)> {
    if begin_index >= end_index || end_index >= f1.confirmed.len() {
        return None;
    }
    let begin = &f1.confirmed[begin_index].point;
    let end = &f1.confirmed[end_index].point;
    if begin.kind == end.kind {
        return None;
    }
    let begin_center = center_position(&f1.combined, begin)?;
    let end_center = center_position(&f1.combined, end)?;
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
            let begin_low = bars[begin_center].low;
            let end_high = bars[end_center].high;
            begin_low < bars[end_center - 1].low
                && begin_low < bars[end_center].low
                && begin_low < bars[end_center + 1].low
                && end_high > bars[begin_center - 1].high
                && end_high > bars[begin_center].high
                && end_high > bars[begin_center + 1].high
        }
        FractalKind::Top => {
            let begin_high = bars[begin_center].high;
            let end_low = bars[end_center].low;
            begin_high > bars[end_center - 1].high
                && begin_high > bars[end_center].high
                && begin_high > bars[end_center + 1].high
                && end_low < bars[begin_center - 1].low
                && end_low < bars[begin_center].low
                && end_low < bars[begin_center + 1].low
        }
    };
    if !strict_patterns {
        return None;
    }
    let mut index = begin_center + 1;
    while index < end_center {
        let clean = match &begin.kind {
            FractalKind::Bottom => bars[index].high <= bars[end_center].high,
            FractalKind::Top => bars[index].low >= bars[end_center].low,
        };
        if !clean {
            return None;
        }
        index += 1;
    }
    Some((begin_center, end_center))
}
/// 取起点左邻至终点右邻所需合并 K 线的最大获知时间。
fn formation_known_at(f1: &F1State, begin_center: usize, end_center: usize) -> DateTime<Utc> {
    let mut known_at = DateTime::<Utc>::MIN_UTC;
    let mut index = begin_center - 1;
    loop {
        if index >= f1.combined.len() {
            break;
        }
        known_at = known_at.max(f1.combined[index].known_at);
        if index > end_center {
            break;
        }
        index += 1;
    }
    known_at
}
/// 建立活动笔，同时固定首次成笔终点及形成时间。
fn new_tentative(
    f1: &F1State,
    begin_index: usize,
    end_index: usize,
    begin_center: usize,
    end_center: usize,
) -> TentativeStroke {
    let formed = formation_known_at(f1, begin_center, end_center);
    TentativeStroke {
        geometry: StrokeGeometry {
            begin_index,
            formation_end_index: end_index,
            end_index,
        },
        formed_known_at: formed,
        current_known_at: formed,
    }
}
/// 以严格更强的同类分型延伸活动终点，保留初始成笔几何。
fn replace_active(
    active: TentativeStroke,
    point_index: usize,
    detected_known_at: &DateTime<Utc>,
) -> TentativeStroke {
    TentativeStroke {
        geometry: StrokeGeometry {
            begin_index: active.geometry.begin_index,
            formation_end_index: active.geometry.formation_end_index,
            end_index: point_index,
        },
        formed_known_at: active.formed_known_at,
        current_known_at: active.current_known_at.max(*detected_known_at),
    }
}
/// 反向笔成立时冻结前笔；确认时间不得早于前笔最后一次端点延伸。
fn freeze(
    predecessor: TentativeStroke,
    confirmed_by_point_index: usize,
    confirmed_known_at: &DateTime<Utc>,
) -> ConfirmedStroke {
    ConfirmedStroke {
        geometry: predecessor.geometry,
        formed_known_at: predecessor.formed_known_at,
        current_known_at: predecessor.current_known_at,
        confirmed_by_point_index,
        confirmed_known_at: predecessor.current_known_at.max(*confirmed_known_at),
    }
}

/// 消费下一个已确认分型，先推进活动笔，再在同一事务内发布前笔。
///
/// 只有符合成笔条件的反向笔才能确认前笔。中间的待确认反向状态不跨调用保存，
/// 因此只需一个已处理分型计数；分型不匹配或锚点无效时原样返回状态。
pub fn accept_confirmed_point(
    mut state: StrokeState,
    f1: &F1State,
    point: &ConfirmedFractal,
) -> (StrokeState, Option<ConfirmedStroke>) {
    let index = state.processed_point_count;
    if f1.confirmed.get(index) != Some(point) {
        return (state, None);
    }
    if let StrokePhase::SeekingFirstStroke { anchor_point_index } = &state.phase
        && *anchor_point_index >= index
    {
        return (state, None);
    }
    let mut published = None;
    state.phase = match state.phase {
        StrokePhase::AwaitingFirstPoint => StrokePhase::SeekingFirstStroke {
            anchor_point_index: index,
        },
        StrokePhase::SeekingFirstStroke { anchor_point_index } => {
            let anchor = &f1.confirmed[anchor_point_index].point;
            if anchor.kind == point.point.kind {
                StrokePhase::SeekingFirstStroke {
                    anchor_point_index: if strictly_more_extreme(f1, &point.point, anchor) {
                        index
                    } else {
                        anchor_point_index
                    },
                }
            } else if let Some((begin_center, end_center)) =
                can_form_stroke(f1, anchor_point_index, index)
            {
                StrokePhase::BuildingStroke {
                    active: new_tentative(f1, anchor_point_index, index, begin_center, end_center),
                }
            } else {
                StrokePhase::SeekingFirstStroke { anchor_point_index }
            }
        }
        StrokePhase::BuildingStroke { active } => {
            if active.geometry.end_index >= index {
                StrokePhase::BuildingStroke { active }
            } else {
                let current = &f1.confirmed[active.geometry.end_index].point;
                if current.kind == point.point.kind {
                    let active = if strictly_more_extreme(f1, &point.point, current) {
                        replace_active(active, index, &point.point.detected_known_at)
                    } else {
                        active
                    };
                    StrokePhase::BuildingStroke { active }
                } else if let Some((begin_center, end_center)) =
                    can_form_stroke(f1, active.geometry.end_index, index)
                {
                    let next = new_tentative(
                        f1,
                        active.geometry.end_index,
                        index,
                        begin_center,
                        end_center,
                    );
                    let confirmed = freeze(active, index, &point.confirmed_known_at);
                    state.confirmed.push(confirmed);
                    published = Some(confirmed);
                    StrokePhase::BuildingStroke { active: next }
                } else {
                    StrokePhase::BuildingStroke { active }
                }
            }
        }
    };
    state.processed_point_count += 1;
    (state, published)
}
