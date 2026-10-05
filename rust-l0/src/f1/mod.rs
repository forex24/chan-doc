//! Incremental F1 inclusion and fractals, consuming only closed quality material.

pub mod feature;
pub mod fractal;
pub mod inclusion;
pub mod morphology;
pub mod segment;
pub mod stroke;

use crate::input::{BarStreamIdentity, MarketDirection, QualityBar};
use crate::number::{Int, Nat};

#[derive(Debug, PartialEq, Eq)]
pub struct RawBar {
    pub slot: Nat,
    pub low: Int,
    pub high: Int,
    pub known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct CombinedBar {
    pub first_slot: Nat,
    pub last_slot: Nat,
    pub low: Int,
    pub high: Int,
    pub direction: MarketDirection,
    pub formed_known_at: Nat,
    pub known_at: Nat,
    pub low_slot: Nat,
    pub high_slot: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub enum FractalKind {
    Top,
    Bottom,
}
#[derive(Debug, PartialEq, Eq)]
pub struct FractalPoint {
    pub kind: FractalKind,
    pub center_index: Nat,
    pub detected_known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmedFractal {
    pub point: FractalPoint,
    pub confirmed_known_at: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub struct F1State {
    pub stream: BarStreamIdentity,
    pub initial_direction: MarketDirection,
    pub combined: Vec<CombinedBar>,
    pub confirmed: Vec<ConfirmedFractal>,
    pub forming: Option<FractalPoint>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct F1Delta {
    pub finalized_combined: Option<CombinedBar>,
    pub confirmed_fractal: Option<ConfirmedFractal>,
    pub forming: Option<FractalPoint>,
    pub tail_updated: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum F1InputError {
    ActiveBar,
    InvalidBar,
    StreamMismatch,
    OutOfOrder,
}
pub fn copy_direction(direction: &MarketDirection) -> MarketDirection {
    match direction {
        MarketDirection::Upward => MarketDirection::Upward,
        MarketDirection::Downward => MarketDirection::Downward,
        MarketDirection::NoNetDisplacement => MarketDirection::NoNetDisplacement,
    }
}
pub fn copy_combined(bar: &CombinedBar) -> CombinedBar {
    CombinedBar {
        first_slot: bar.first_slot.clone(),
        last_slot: bar.last_slot.clone(),
        low: bar.low.clone(),
        high: bar.high.clone(),
        direction: copy_direction(&bar.direction),
        formed_known_at: bar.formed_known_at.clone(),
        known_at: bar.known_at.clone(),
        low_slot: bar.low_slot.clone(),
        high_slot: bar.high_slot.clone(),
    }
}
pub fn copy_point(point: &FractalPoint) -> FractalPoint {
    FractalPoint {
        kind: match point.kind {
            FractalKind::Top => FractalKind::Top,
            FractalKind::Bottom => FractalKind::Bottom,
        },
        center_index: point.center_index.clone(),
        detected_known_at: point.detected_known_at.clone(),
    }
}
pub fn new_f1_state(stream: BarStreamIdentity, initial_direction: MarketDirection) -> F1State {
    F1State {
        stream,
        initial_direction,
        combined: Vec::new(),
        confirmed: Vec::new(),
        forming: None,
    }
}
pub fn raw_from_closed(bar: &QualityBar) -> RawBar {
    RawBar {
        slot: bar.slot.clone(),
        low: bar.low.clone(),
        high: bar.high.clone(),
        known_at: bar.known_at.clone(),
    }
}
