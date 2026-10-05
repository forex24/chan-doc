//! 去包含：每次只将新闭合 Bar 与活动合并尾比较，不重建冻结前缀。

use crate::f1::{CombinedBar, RawBar};
use crate::input::MarketDirection;

/// 判断两个价格闭区间是否相含；完全相等也属于包含。
pub fn combined_absorbs(last: &CombinedBar, next: &RawBar) -> bool {
    (last.high >= next.high && last.low <= next.low)
        || (next.high >= last.high && next.low <= last.low)
}
/// 用一根原始 Bar 建立合并 K 线，保留调用方给出的初始方向。
pub fn single_combined(bar: &RawBar, initial: &MarketDirection) -> CombinedBar {
    CombinedBar {
        first_slot: bar.slot,
        last_slot: bar.slot,
        low: bar.low,
        high: bar.high,
        direction: *initial,
        formed_known_at: bar.known_at,
        known_at: bar.known_at,
        low_slot: bar.slot,
        high_slot: bar.slot,
    }
}
/// 按当前方向吸收包含 Bar；同价保留旧极值来源。
///
/// 向上取高低价的较大值，其他方向沿用较小值。已有明确方向时，顺向极值处的
/// 一字板只延长来源和获知时间，不压扁原区间：向上匹配最高价，向下匹配最低价。
/// 该特例与 chan-core-2026-final 的 merge_klu_into_klc 一致。
pub fn absorb(last: &CombinedBar, next: &RawBar) -> CombinedBar {
    let preserve_range = next.high == next.low
        && match last.direction {
            MarketDirection::Upward => next.high == last.high,
            MarketDirection::Downward => next.low == last.low,
            MarketDirection::NoNetDisplacement => false,
        };
    if preserve_range {
        return CombinedBar {
            last_slot: next.slot,
            known_at: last.known_at.max(next.known_at),
            ..*last
        };
    }
    let upward = matches!(last.direction, MarketDirection::Upward);
    let choose_old_low = if upward {
        last.low >= next.low
    } else {
        last.low <= next.low
    };
    let choose_old_high = if upward {
        last.high >= next.high
    } else {
        last.high <= next.high
    };
    CombinedBar {
        first_slot: last.first_slot,
        last_slot: next.slot,
        low: if choose_old_low { last.low } else { next.low },
        high: if choose_old_high {
            last.high
        } else {
            next.high
        },
        direction: if upward {
            MarketDirection::Upward
        } else {
            MarketDirection::Downward
        },
        formed_known_at: last.formed_known_at,
        known_at: last.known_at.max(next.known_at),
        low_slot: if choose_old_low {
            last.low_slot
        } else {
            next.slot
        },
        high_slot: if choose_old_high {
            last.high_slot
        } else {
            next.slot
        },
    }
}
/// 建立非包含的新尾；由新旧高低价关系确定方向，并重新记录形成时间。
pub fn fresh_combined(last: &CombinedBar, next: &RawBar) -> CombinedBar {
    let upward = next.high >= last.high && next.low >= last.low;
    CombinedBar {
        first_slot: next.slot,
        last_slot: next.slot,
        low: next.low,
        high: next.high,
        direction: if upward {
            MarketDirection::Upward
        } else {
            MarketDirection::Downward
        },
        formed_known_at: next.known_at,
        known_at: next.known_at,
        low_slot: next.slot,
        high_slot: next.slot,
    }
}
