//! The only external market input accepted by the Rust Core.

use crate::number::{Int, Nat};

#[derive(Debug, PartialEq, Eq)]
pub enum MarketDirection {
    Upward,
    Downward,
    NoNetDisplacement,
}
#[derive(Debug, PartialEq, Eq)]
pub struct CompleteBoundary {
    pub root_point_market_order: Nat,
    pub initial_inclusion_direction: MarketDirection,
    pub known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LeftMarketBoundary {
    CompleteBoundaryAtFirstMovement(CompleteBoundary),
    EarlierMarketContextUnknown,
}
#[derive(Debug, PartialEq, Eq)]
pub struct BarStreamIdentity {
    pub market: String,
    pub instrument: String,
    pub timeframe: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct QualityBar {
    pub stream: BarStreamIdentity,
    pub slot: Nat,
    pub open: Int,
    pub high: Int,
    pub low: Int,
    pub close: Int,
    pub volume: Nat,
    pub turnover: Option<Nat>,
    pub known_at: Nat,
    pub closed: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum BarStreamEvent {
    Append {
        event_sequence: Nat,
        bar: QualityBar,
    },
    UpdateActiveTail {
        event_sequence: Nat,
        bar: QualityBar,
    },
}
