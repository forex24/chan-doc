//! 以真实确认笔构造特征序列，按方向去包含并保留完整来源。

use crate::f1::stroke::{ConfirmedStroke, center_position};
use crate::f1::{F1State, FractalKind};

/// 已成形线段的方向，只有向上或向下。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentDirection {
    /// 向上线段。
    Up,
    /// 向下线段。
    Down,
}
/// 价格闭区间；正式管线中两端均有限且 low ≤ high。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriceInterval {
    /// 区间最低价。
    pub low: f64,
    /// 区间最高价。
    pub high: f64,
}
/// 一个去包含特征元素；完整来源与端点极值来源分别保留。
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureElement {
    /// 参与合并的全部笔下标，按原始顺序排列。
    pub source_stroke_indices: Vec<usize>,
    /// 最近一次吸收的笔下标，即来源列表尾部，用作增量游标。
    pub source_stroke_index: usize,
    /// 去包含后的特征价格区间。
    pub interval: PriceInterval,
    /// 方向极值的原始笔下标；同价保留最早来源。
    pub extreme_source_stroke_index: usize,
}
/// 通过确认笔两端的底顶类型确定方向，源下标或端点类型非法时返回 None。
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
/// 读取确认笔的真实端点 K 线极值；不使用首次成笔几何替代当前端点。
pub fn stroke_interval(f1: &F1State, stroke: &ConfirmedStroke) -> Option<PriceInterval> {
    let begin = stroke.geometry.begin_index;
    let end = stroke.geometry.end_index;
    if begin >= f1.confirmed.len() || end >= f1.confirmed.len() {
        return None;
    }
    let begin_center = center_position(&f1.combined, &f1.confirmed[begin].point)?;
    let end_center = center_position(&f1.combined, &f1.confirmed[end].point)?;
    let interval = match stroke_direction(f1, stroke) {
        Some(SegmentDirection::Up) => PriceInterval {
            low: f1.combined[begin_center].low,
            high: f1.combined[end_center].high,
        },
        Some(SegmentDirection::Down) => PriceInterval {
            low: f1.combined[end_center].low,
            high: f1.combined[begin_center].high,
        },
        None => return None,
    };
    if interval.low > interval.high {
        None
    } else {
        Some(interval)
    }
}
/// 将一条确认笔转换为单来源特征元素。
pub fn feature_for_stroke(
    f1: &F1State,
    stroke: &ConfirmedStroke,
    index: usize,
) -> Option<FeatureElement> {
    let interval = stroke_interval(f1, stroke)?;
    Some(FeatureElement {
        source_stroke_indices: vec![index],
        source_stroke_index: index,
        interval,
        extreme_source_stroke_index: index,
    })
}
/// 返回相反的线段方向。
pub fn opposite(direction: &SegmentDirection) -> SegmentDirection {
    match direction {
        SegmentDirection::Up => SegmentDirection::Down,
        SegmentDirection::Down => SegmentDirection::Up,
    }
}
/// 判断笔是否与当前线段方向相反。
pub fn stroke_opposes(direction: &SegmentDirection, stroke_direction: &SegmentDirection) -> bool {
    direction != stroke_direction
}
/// 判断两个价格闭区间是否相交，端点相等也算相交。
pub fn intervals_intersect(a: &PriceInterval, b: &PriceInterval) -> bool {
    a.low <= b.high && b.low <= a.high
}
/// 判断区间是否相互包含，包含等区间。
pub fn interval_inclusion(a: &PriceInterval, b: &PriceInterval) -> bool {
    (a.high >= b.high && a.low <= b.low) || (b.high >= a.high && b.low <= a.low)
}
/// 向上合并分别取高低价的较大值，向下分别取较小值。
pub fn merge_interval(
    a: &PriceInterval,
    b: &PriceInterval,
    direction: &SegmentDirection,
) -> PriceInterval {
    match direction {
        SegmentDirection::Up => PriceInterval {
            low: a.low.max(b.low),
            high: a.high.max(b.high),
        },
        SegmentDirection::Down => PriceInterval {
            low: a.low.min(b.low),
            high: a.high.min(b.high),
        },
    }
}
/// 只合并同处切分点一侧的包含元素；拼接全部来源，严格新极值才替换极值来源。
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
        sources.extend(next.source_stroke_indices);
        let new_extreme = match direction {
            SegmentDirection::Up => next.interval.high > last.interval.high,
            SegmentDirection::Down => next.interval.low < last.interval.low,
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
/// 判断第一特征序列的目标分型，保留既有右侧等高或等低的非对称规则。
pub fn primary_target(
    left: &FeatureElement,
    middle: &FeatureElement,
    right: &FeatureElement,
    direction: &SegmentDirection,
) -> bool {
    match direction {
        SegmentDirection::Up => {
            middle.interval.high > left.interval.high
                && middle.interval.high >= right.interval.high
                && middle.interval.low > right.interval.low
        }
        SegmentDirection::Down => {
            middle.interval.low < left.interval.low
                && middle.interval.high < right.interval.high
                && middle.interval.low <= right.interval.low
        }
    }
}
/// 判断第二特征序列的严格反向分型；高低价均须严格满足镜像条件。
pub fn standard_second_target(
    left: &FeatureElement,
    middle: &FeatureElement,
    right: &FeatureElement,
    direction: &SegmentDirection,
) -> bool {
    match direction {
        SegmentDirection::Up => {
            middle.interval.high < left.interval.high
                && middle.interval.high < right.interval.high
                && middle.interval.low < left.interval.low
                && middle.interval.low < right.interval.low
        }
        SegmentDirection::Down => {
            middle.interval.high > left.interval.high
                && middle.interval.high > right.interval.high
                && middle.interval.low > left.interval.low
                && middle.interval.low > right.interval.low
        }
    }
}
