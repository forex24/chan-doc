//! F1Inclusion: fold one closed RawBar against only the live combined tail.

use crate::f1::{CombinedBar, RawBar, copy_direction};
use crate::input::MarketDirection;
use std::cmp::Ordering;

pub fn combined_absorbs(last: &CombinedBar, next: &RawBar) -> bool {
    (last.high.cmp_exact(&next.high) != Ordering::Less
        && last.low.cmp_exact(&next.low) != Ordering::Greater)
        || (next.high.cmp_exact(&last.high) != Ordering::Less
            && next.low.cmp_exact(&last.low) != Ordering::Greater)
}
pub fn single_combined(bar: &RawBar, initial: &MarketDirection) -> CombinedBar {
    CombinedBar {
        first_slot: bar.slot.clone(),
        last_slot: bar.slot.clone(),
        low: bar.low.clone(),
        high: bar.high.clone(),
        direction: copy_direction(initial),
        formed_known_at: bar.known_at.clone(),
        known_at: bar.known_at.clone(),
        low_slot: bar.slot.clone(),
        high_slot: bar.slot.clone(),
    }
}
pub fn absorb(last: &CombinedBar, next: &RawBar) -> CombinedBar {
    let upward = match &last.direction {
        MarketDirection::Upward => true,
        _ => false,
    };
    let choose_old_low = if upward {
        last.low.cmp_exact(&next.low) != Ordering::Less
    } else {
        last.low.cmp_exact(&next.low) != Ordering::Greater
    };
    let choose_old_high = if upward {
        last.high.cmp_exact(&next.high) != Ordering::Less
    } else {
        last.high.cmp_exact(&next.high) != Ordering::Greater
    };
    let known_at = match last.known_at.cmp_exact(&next.known_at) {
        Ordering::Less => next.known_at.clone(),
        _ => last.known_at.clone(),
    };
    CombinedBar {
        first_slot: last.first_slot.clone(),
        last_slot: next.slot.clone(),
        low: if choose_old_low {
            last.low.clone()
        } else {
            next.low.clone()
        },
        high: if choose_old_high {
            last.high.clone()
        } else {
            next.high.clone()
        },
        direction: if upward {
            MarketDirection::Upward
        } else {
            MarketDirection::Downward
        },
        formed_known_at: last.formed_known_at.clone(),
        known_at,
        low_slot: if choose_old_low {
            last.low_slot.clone()
        } else {
            next.slot.clone()
        },
        high_slot: if choose_old_high {
            last.high_slot.clone()
        } else {
            next.slot.clone()
        },
    }
}
pub fn fresh_combined(last: &CombinedBar, next: &RawBar) -> CombinedBar {
    let upward = next.high.cmp_exact(&last.high) != Ordering::Less
        && next.low.cmp_exact(&last.low) != Ordering::Less;
    CombinedBar {
        first_slot: next.slot.clone(),
        last_slot: next.slot.clone(),
        low: next.low.clone(),
        high: next.high.clone(),
        direction: if upward {
            MarketDirection::Upward
        } else {
            MarketDirection::Downward
        },
        formed_known_at: next.known_at.clone(),
        known_at: next.known_at.clone(),
        low_slot: next.slot.clone(),
        high_slot: next.slot.clone(),
    }
}
