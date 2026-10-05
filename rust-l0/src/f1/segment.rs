//! 增量线段识别：每个候选切分点独立推进第一、第二特征序列，并要求真实笔破坏。

use crate::f1::feature::{
    FeatureElement, PriceInterval, SegmentDirection, feature_for_stroke, intervals_intersect,
    normalized_push, opposite, primary_target, standard_second_target, stroke_direction,
    stroke_interval, stroke_opposes,
};
use crate::f1::stroke::{ConfirmedStroke, StrokeState, center_position};
use crate::f1::{F1State, FractalKind};
use chrono::{DateTime, Utc};

/// 线段确认所依据的识别分支。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecognitionReason {
    /// 第一特征序列无缺口，且已获得真实笔破坏证据。
    ImmediateNoGap,
    /// 第一特征序列有缺口，经第二特征序列分型和真实笔破坏确认。
    RepairedBySecondSequence,
}
/// 线段候选与确认的获知时间，均使用 UTC。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentTimes {
    /// 第一特征序列形成目标分型的获知时间。
    pub candidate_known_at: DateTime<Utc>,
    /// 全部确认条件满足的获知时间，不早于候选或前一线段。
    pub confirmed_at: DateTime<Utc>,
}
/// 已确认线段，保存端点、全区间和特征序列证据。
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmedSegment {
    /// 该线段在完成账本中的下标。
    pub ordinal: usize,
    /// 本段首笔在确认笔账本中的下标。
    pub begin_stroke_index: usize,
    /// 下一段首笔下标；其起点是本段终点，属于右开边界。
    pub end_stroke_index: usize,
    /// 本段方向。
    pub direction: SegmentDirection,
    /// 用于确认的三元素特征分型快照。
    pub elements: Vec<FeatureElement>,
    /// 两端实际来源时间之间的原始 Bar 价格区间，包含未入选笔端点的内部极值。
    pub price_interval: PriceInterval,
    /// 本段起点价格。
    pub start_price: f64,
    /// 本段终点价格。
    pub end_price: f64,
    /// 候选与确认时间。
    pub times: SegmentTimes,
    /// 无缺口或第二特征序列确认分支。
    pub reason: RecognitionReason,
}
/// 真实笔区间产生破坏的来源证据。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActualBreakWitness {
    /// 第一特征序列中心元素的源笔下标。
    pub center_source_stroke_index: usize,
    /// 实际产生破坏的已观察笔下标。
    pub observed_stroke_index: usize,
}
/// 一个切分候选的识别阶段；所需证据放在对应枚举分支内。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStage {
    /// 第一特征序列还未构成目标分型。
    SeekingPrimary,
    /// 无缺口分型已成形，仍待真实笔破坏。
    SeekingActualBreak {
        /// 目标分型第一次成立的获知时间。
        candidate_known_at: DateTime<Utc>,
    },
    /// 有缺口分型已成形，继续等待第二特征序列及真实破坏。
    SeekingSecond {
        /// 目标分型第一次成立的获知时间。
        candidate_known_at: DateTime<Utc>,
        /// 已出现的真实破坏证据；保留后继续等待第二序列。
        actual_break: Option<ActualBreakWitness>,
    },
}
/// 独立切分候选，避免不同候选的包含状态相互污染。
#[derive(Clone, Debug, PartialEq)]
pub struct PrimarySplitProbe {
    /// 候选切分笔下标。
    pub split_stroke_index: usize,
    /// 该切分点的第一特征序列。
    pub primary: Vec<FeatureElement>,
    /// 该切分点的第二特征序列。
    pub second: Vec<FeatureElement>,
    /// 第二序列已识别目标分型的中心下标。
    pub second_target: Option<usize>,
    /// 当前等待的条件与已收集证据。
    pub stage: ProbeStage,
}
/// 尚待确认的当前线段。
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentPending {
    /// 候选线段首笔下标。
    pub begin_stroke_index: usize,
    /// 候选线段方向。
    pub direction: SegmentDirection,
    /// 当前线段完整的第一特征序列。
    pub primary: Vec<FeatureElement>,
    /// 已消费确认笔的最大获知时间。
    pub known_at: DateTime<Utc>,
    /// 按创建顺序排列的独立切分候选。
    pub probes: Vec<PrimarySplitProbe>,
}
/// 线段账本与待处理笔游标。
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentState {
    /// 已确认线段前缀。
    pub completed: Vec<ConfirmedSegment>,
    /// 当前未确认线段，无候选时为 None。
    pub pending: Option<SegmentPending>,
    /// 下一次需要处理的确认笔下标；确认切分后可回退以回放新段。
    pub next_stroke_index: usize,
}
/// 线段推进失败原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentError {
    /// 当前声明起点的前三笔没有公共重叠；后续笔不能修复该起点。
    InvalidInitialOverlap,
    /// 源笔、分型下标或端点价格不合法。
    InvalidStrokeSource,
    /// 线段游标、候选窗口或端点顺序不合法。
    InvalidSegmentState,
}
/// 切分候选满足全部条件后的识别结果。
#[derive(Clone, Debug, PartialEq)]
pub struct Recognition {
    /// 获确认的切分笔下标。
    pub split_stroke_index: usize,
    /// 第一序列目标分型的三元素快照。
    pub window: Vec<FeatureElement>,
    /// 第一序列目标分型获知时间。
    pub candidate_known_at: DateTime<Utc>,
    /// 全部识别条件满足的获知时间。
    pub confirmed_known_at: DateTime<Utc>,
    /// 此次确认走过的识别分支。
    pub reason: RecognitionReason,
    /// 必须具备的真实笔破坏证据。
    pub actual_break: ActualBreakWitness,
}
/// 单根确认笔对一个切分候选的推进结果。
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeStep {
    /// 候选已失效。
    Discarded,
    /// 仍需后续材料的候选。
    Waiting(PrimarySplitProbe),
    /// 所有识别条件已满足。
    Recognized(Recognition),
}
/// 建立空线段账本与从零开始的确认笔游标。
pub fn new_segment_state() -> SegmentState {
    SegmentState {
        completed: Vec::new(),
        pending: None,
        next_stroke_index: 0,
    }
}
/// 从源笔起点分型读取真实极值来源时间和价格，下标无效时返回 None。
fn stroke_begin_endpoint(f1: &F1State, stroke: &ConfirmedStroke) -> Option<(DateTime<Utc>, f64)> {
    let index = stroke.geometry.begin_index;
    if index >= f1.confirmed.len() {
        return None;
    }
    let point = &f1.confirmed[index].point;
    let center = center_position(&f1.combined, point)?;
    let bar = &f1.combined[center];
    Some(match point.kind {
        FractalKind::Top => (bar.high_slot, bar.high),
        FractalKind::Bottom => (bar.low_slot, bar.low),
    })
}
/// 从源笔起点分型读取真实极值价格。
fn stroke_begin_price(f1: &F1State, stroke: &ConfirmedStroke) -> Option<f64> {
    stroke_begin_endpoint(f1, stroke).map(|(_, price)| price)
}
/// 检查从 first 开始的三根真实笔是否存在公共交集；缺失来源时返回 None。
fn first_three_overlap(f1: &F1State, strokes: &[ConfirmedStroke], first: usize) -> Option<bool> {
    let three = strokes.get(first..)?.get(..3)?;
    let first = stroke_interval(f1, &three[0])?;
    let second = stroke_interval(f1, &three[1])?;
    let third = stroke_interval(f1, &three[2])?;
    let low = first.low.max(second.low).max(third.low);
    let high = first.high.min(second.high).min(third.high);
    Some(low.is_finite() && high.is_finite() && low <= high)
}
/// 同时检查源下标顺序、前三笔公共重叠、中心破坏和两项方向消歧条件。
pub fn actual_break(
    f1: &F1State,
    strokes: &[ConfirmedStroke],
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
    if first_three_overlap(f1, strokes, split) != Some(true) {
        return false;
    }
    let center_break = match direction {
        SegmentDirection::Up => challenge.low < center.low,
        SegmentDirection::Down => challenge.high > center.high,
    };
    center_break
        && first_stroke_directional_resolution(&previous, &first, &challenge, direction)
        && initial_three_direction_resolved(&first, &third, &challenge, direction)
}
/// 若首笔已突破前一对应笔，则后续挑战还必须突破首笔本身。
pub fn first_stroke_directional_resolution(
    previous: &PriceInterval,
    first: &PriceInterval,
    challenge: &PriceInterval,
    direction: &SegmentDirection,
) -> bool {
    let prior_break = match direction {
        SegmentDirection::Up => first.low < previous.low,
        SegmentDirection::Down => first.high > previous.high,
    };
    let first_break = match direction {
        SegmentDirection::Up => challenge.low < first.low,
        SegmentDirection::Down => challenge.high > first.high,
    };
    !prior_break || first_break
}
/// 前三笔方向须由第三笔或后续挑战对首笔的突破得到确认。
pub fn initial_three_direction_resolved(
    first: &PriceInterval,
    third: &PriceInterval,
    challenge: &PriceInterval,
    direction: &SegmentDirection,
) -> bool {
    let first_break = match direction {
        SegmentDirection::Up => challenge.low < first.low,
        SegmentDirection::Down => challenge.high > first.high,
    };
    let third_break = match direction {
        SegmentDirection::Up => third.low < first.low,
        SegmentDirection::Down => third.high > first.high,
    };
    third_break || first_break
}
/// 检查第一特征序列末尾三个元素是否满足目标分型。
fn primary_tail_target(primary: &[FeatureElement], direction: &SegmentDirection) -> bool {
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
/// 先检查右侧同价极值能否构成目标分型，再做包含合并，避免证据被提前吸收。
fn advance_split_primary(
    primary: Vec<FeatureElement>,
    feature: &FeatureElement,
    direction: &SegmentDirection,
    split: usize,
) -> Vec<FeatureElement> {
    if primary.len() == 2 {
        let appended = vec![primary[0].clone(), primary[1].clone(), feature.clone()];
        if primary_tail_target(&appended, direction) {
            return appended;
        }
    }
    normalized_push(primary, feature.clone(), direction, split)
}
/// 更新第二序列目标；活动尾被包含替换时重新校验尚未冻结的尾部分型。
fn second_target_after(
    previous_count: usize,
    second: &[FeatureElement],
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
/// 冻结候选窗口和真实破坏证据，形成可交给线段构造器的结果。
fn recognition(
    split: usize,
    primary: &[FeatureElement],
    candidate: &DateTime<Utc>,
    known: &DateTime<Utc>,
    reason: RecognitionReason,
    witness: ActualBreakWitness,
) -> Recognition {
    let window = primary.to_vec();
    Recognition {
        split_stroke_index: split,
        window,
        candidate_known_at: *candidate,
        confirmed_known_at: *known,
        reason,
        actual_break: witness,
    }
}
/// 先推进独立第二序列，再按阶段检查失效、真实破坏和目标分型；不混用其他切分点的材料。
pub fn advance_probe(
    mut probe: PrimarySplitProbe,
    feature: &FeatureElement,
    opposes: bool,
    direction: &SegmentDirection,
    known_at: &DateTime<Utc>,
    has_actual_break: bool,
) -> ProbeStep {
    if probe.primary.len() < 2 {
        return ProbeStep::Discarded;
    }
    let previous_second_count = probe.second.len();
    if !opposes {
        probe.second = normalized_push(
            probe.second,
            feature.clone(),
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
        SegmentDirection::Up => feature.interval.high > center.interval.high,
        SegmentDirection::Down => feature.interval.low < center.interval.low,
    };
    let evidence = match &probe.stage {
        ProbeStage::SeekingSecond {
            actual_break: Some(value),
            ..
        } => Some(*value),
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
            // 右元素同一根笔也可越过旧极值：已有严格第二分型时先确认。
            // 78课的延续条件是“未形成第二分型又直接新高/新低”，不能省略前半句。
            if probe.second_target.is_some()
                && let Some(witness) = evidence
            {
                return ProbeStep::Recognized(recognition(
                    probe.split_stroke_index,
                    &probe.primary,
                    &candidate_known_at,
                    known_at,
                    RecognitionReason::RepairedBySecondSequence,
                    witness,
                ));
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
                        candidate_known_at: *known_at,
                    };
                } else {
                    if probe.second_target.is_some()
                        && let Some(witness) = evidence
                    {
                        return ProbeStep::Recognized(recognition(
                            probe.split_stroke_index,
                            &probe.primary,
                            known_at,
                            known_at,
                            RecognitionReason::RepairedBySecondSequence,
                            witness,
                        ));
                    }
                    probe.stage = ProbeStage::SeekingSecond {
                        candidate_known_at: *known_at,
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
/// 读取起终端点实际来源 Bar 之间的闭区间；不以笔端点或去包含后的区间代替原始价格。
fn span_range(
    f1: &F1State,
    strokes: &[ConfirmedStroke],
    first: usize,
    end: usize,
) -> Option<PriceInterval> {
    if first > end || end >= strokes.len() {
        return None;
    }
    let (start_slot, _) = stroke_begin_endpoint(f1, &strokes[first])?;
    let (end_slot, _) = stroke_begin_endpoint(f1, &strokes[end])?;
    let start = f1
        .raw
        .binary_search_by_key(&start_slot, |bar| bar.slot)
        .ok()?;
    let end = f1
        .raw
        .binary_search_by_key(&end_slot, |bar| bar.slot)
        .ok()?;
    let bars = f1.raw.get(start..=end)?;
    let mut range = PriceInterval {
        low: bars[0].low,
        high: bars[0].high,
    };
    for bar in &bars[1..] {
        range.low = range.low.min(bar.low);
        range.high = range.high.max(bar.high);
    }
    Some(range)
}
/// 校验端点方向并构造全价格区间，确认时间取候选、识别和前一段确认时间的最大值。
fn build_segment(
    f1: &F1State,
    strokes: &[ConfirmedStroke],
    completed: &[ConfirmedSegment],
    pending: &SegmentPending,
    recognized: Recognition,
) -> Option<ConfirmedSegment> {
    if pending.begin_stroke_index >= strokes.len()
        || recognized.split_stroke_index >= strokes.len()
        || pending.begin_stroke_index >= recognized.split_stroke_index
    {
        return None;
    }
    let count = recognized.split_stroke_index - pending.begin_stroke_index;
    if count < 3
        || count.is_multiple_of(2)
        || first_three_overlap(f1, strokes, pending.begin_stroke_index) != Some(true)
        || stroke_direction(f1, &strokes[recognized.split_stroke_index - 1])
            != Some(pending.direction)
    {
        return None;
    }
    let start_price = stroke_begin_price(f1, &strokes[pending.begin_stroke_index])?;
    let end_price = stroke_begin_price(f1, &strokes[recognized.split_stroke_index])?;
    let advancing = match pending.direction {
        SegmentDirection::Up => start_price < end_price,
        SegmentDirection::Down => start_price > end_price,
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
        low: span.low.min(start_price).min(end_price),
        high: span.high.max(start_price).max(end_price),
    };
    let previous_known = match completed.last() {
        Some(value) => value.times.confirmed_at,
        None => DateTime::<Utc>::MIN_UTC,
    };
    let confirmed_at = recognized
        .candidate_known_at
        .max(recognized.confirmed_known_at)
        .max(previous_known);
    Some(ConfirmedSegment {
        ordinal: completed.len(),
        begin_stroke_index: pending.begin_stroke_index,
        end_stroke_index: recognized.split_stroke_index,
        direction: pending.direction,
        elements: recognized.window,
        price_interval,
        start_price,
        end_price,
        times: SegmentTimes {
            candidate_known_at: recognized.candidate_known_at,
            confirmed_at,
        },
        reason: recognized.reason,
    })
}
/// 先推进独立切分候选；已形成分型的较早候选未确认或失效前，后续候选不得抢先发布。
fn step_segment(
    mut state: SegmentState,
    f1: &F1State,
    strokes: &[ConfirmedStroke],
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
            known_at: strokes[index].confirmed_known_at,
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
    // 声明起点不能靠后面的合法三笔替代。第三笔冻结后即可判定，无需等到发布时。
    if index >= pending.begin_stroke_index && index - pending.begin_stroke_index == 2 {
        let overlap = first_three_overlap(f1, strokes, pending.begin_stroke_index);
        if overlap != Some(true) {
            state.pending = Some(pending);
            return Err((
                state,
                match overlap {
                    Some(false) => SegmentError::InvalidInitialOverlap,
                    _ => SegmentError::InvalidStrokeSource,
                },
            ));
        }
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
    let known_at = pending.known_at.max(strokes[index].confirmed_known_at);
    let mut next_probes = Vec::new();
    let mut selected = None;
    let mut earlier_boundary_waiting = false;
    let mut probe_index = 0usize;
    while probe_index < pending.probes.len() {
        let probe = pending.probes[probe_index].clone();
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
            ProbeStep::Waiting(waiting) => {
                earlier_boundary_waiting |= !matches!(waiting.stage, ProbeStage::SeekingPrimary);
                next_probes.push(waiting);
            }
            ProbeStep::Recognized(found) => {
                if found.window.len() != 3 {
                    state.pending = Some(pending);
                    return Err((state, SegmentError::InvalidSegmentState));
                }
                // 较早候选若最终确认，会从其切分点重放后续笔；若被新极值取消，
                // 同一极值也已取消这些较低高点/较高低点。无需缓存被阻挡的识别结果。
                if selected.is_none() && !earlier_boundary_waiting {
                    let start_price = stroke_begin_price(f1, &strokes[pending.begin_stroke_index]);
                    if let Some(start_price) = start_price {
                        let endpoint = match pending.direction {
                            SegmentDirection::Up => found.window[1].interval.high,
                            SegmentDirection::Down => found.window[1].interval.low,
                        };
                        let advances = match pending.direction {
                            SegmentDirection::Up => start_price < endpoint,
                            SegmentDirection::Down => start_price > endpoint,
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
        if let Some(last) = state.completed.last()
            && end_index <= last.end_stroke_index
        {
            state.pending = Some(pending);
            return Err((state, SegmentError::InvalidSegmentState));
        }
        state.completed.push(segment.clone());
        state.pending = None;
        state.next_stroke_index = end_index;
        return Ok((state, Some(segment)));
    }
    if opposes && !pending.primary.is_empty() {
        let primary = vec![pending.primary.last().unwrap().clone(), feature.clone()];
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
/// 消费尚未处理的确认笔；切分端点可能早于当前笔，因此新段从切分笔开始回放。
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
