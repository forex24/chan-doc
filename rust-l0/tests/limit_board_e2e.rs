//! 一字板从质量门进入完整 L0 管线的回归。
//!
//! 实现前固定失败面：顺向边缘一字板压扁原区间、连续一字板改写极值来源、
//! 漏记尾部时间、把内部一字板或普通 K 线误当特例、误发分型/笔/L0、上下镜像不一致。

use chan_l0::input::{BarStreamEvent, BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{
    CompleteBoundary, F1PipelineDelta, F1PipelineState, advance_pipeline, new_pipeline_state,
};
use chan_l0::quality::{
    BarQualityState, QualityGateDecision, apply_quality_event, empty_bar_quality_state,
};
use chrono::{DateTime, TimeDelta, Utc};
use std::fmt::Write;
use std::path::Path;

fn time(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 123_456_789).unwrap()
}

fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "SYNTHETIC".into(),
        instrument: "LIMIT_BOARD_E2E".into(),
        timeframe: TimeDelta::seconds(10),
    }
}

fn bar(slot: i64, low: f64, high: f64) -> QualityBar {
    QualityBar {
        stream: stream(),
        slot: time(slot),
        open: low,
        high,
        low,
        close: high,
        volume: 1,
        turnover: None,
        known_at: time(slot + 10),
        closed: true,
    }
}

struct Replay {
    quality: BarQualityState,
    pipeline: F1PipelineState,
}

impl Replay {
    fn new(direction: MarketDirection, root: usize) -> Self {
        Self {
            quality: empty_bar_quality_state(stream()).unwrap(),
            pipeline: new_pipeline_state(
                stream(),
                CompleteBoundary {
                    root_point: root,
                    initial_inclusion_direction: direction,
                    known_at: time(root as i64 * 30),
                },
            ),
        }
    }

    fn push(mut self, bar: QualityBar, trace: &mut String) -> (Self, F1PipelineDelta) {
        let event_sequence = self.quality.last_event_sequence + 1;
        let QualityGateDecision::GateAccepted {
            next,
            structure_material: Some(material),
            ..
        } = apply_quality_event(
            self.quality,
            BarStreamEvent::Append {
                event_sequence,
                bar,
            },
        )
        else {
            panic!("valid closed Bar rejected");
        };
        self.quality = next;
        let frozen = self.pipeline.lowest_movements.clone();
        let (next, delta) = advance_pipeline(self.pipeline, &material).unwrap();
        assert_eq!(&next.lowest_movements[..frozen.len()], &frozen);
        self.pipeline = next;
        let f1 = &self.pipeline.morphology.f1;
        let tail = f1.combined.last().unwrap();
        writeln!(
            trace,
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            material.slot.to_rfc3339(),
            material.low,
            material.high,
            f1.combined.len(),
            tail.low,
            tail.high,
            tail.low_slot.to_rfc3339(),
            tail.high_slot.to_rfc3339(),
            f1.confirmed.len(),
            self.pipeline.morphology.stroke.confirmed.len(),
            self.pipeline.lowest_movements.len(),
        )
        .unwrap();
        for movement in &delta.lowest_movements {
            writeln!(trace, "L0|{movement:?}").unwrap();
        }
        (self, delta)
    }
}

fn save(name: &str, trace: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/e2e/{name}.txt"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, trace).unwrap();
}

#[test]
fn directional_limit_boards_preserve_range_without_suppressing_other_merges() {
    use MarketDirection::{Downward as Down, Upward as Up};
    // 期望直接来自参考项目的区间规则；同价极值仍遵守本库保留最早来源的合同。
    // 每项：方向、新区间、期望合并区间、低/高价来源秒数、期望合并 K 数量。
    let cases = [
        (Up, (20.0, 20.0), (10.0, 20.0), (0, 0), 1),
        (Down, (10.0, 10.0), (10.0, 20.0), (0, 0), 1),
        (Up, (15.0, 15.0), (15.0, 20.0), (10, 0), 1),
        (Down, (15.0, 15.0), (10.0, 15.0), (0, 10), 1),
        (Up, (10.0, 10.0), (10.0, 20.0), (0, 0), 1),
        (Down, (20.0, 20.0), (10.0, 20.0), (0, 0), 1),
        (Up, (12.0, 20.0), (12.0, 20.0), (10, 0), 1),
        (Down, (10.0, 18.0), (10.0, 18.0), (0, 10), 1),
        (Up, (10.0, 20.0), (10.0, 20.0), (0, 0), 1),
        (Down, (10.0, 20.0), (10.0, 20.0), (0, 0), 1),
        (Up, (21.0, 21.0), (21.0, 21.0), (10, 10), 2),
        (Down, (9.0, 9.0), (9.0, 9.0), (10, 10), 2),
    ];
    let mut trace = String::new();
    for (direction, input, expected, sources, count) in cases {
        writeln!(trace, "case|{direction:?}|{input:?}").unwrap();
        let (mut replay, _) = Replay::new(direction, 0).push(bar(0, 10.0, 20.0), &mut trace);
        for slot in [10, 20] {
            (replay, _) = replay.push(bar(slot, input.0, input.1), &mut trace);
            let combined = &replay.pipeline.morphology.f1.combined;
            let tail = combined.last().unwrap();
            assert_eq!((tail.low, tail.high), expected, "{direction:?}/{input:?}");
            assert_eq!(
                (tail.low_slot, tail.high_slot),
                (time(sources.0), time(sources.1))
            );
            assert_eq!(tail.last_slot, time(slot));
            assert_eq!(tail.known_at, time(slot + 10));
            assert_eq!(combined.len(), count);
            assert!(replay.pipeline.lowest_movements.is_empty());
        }
    }
    save("limit-board-boundaries", &trace);
}

#[test]
fn consecutive_limit_boards_preserve_multisegment_geometry_and_publication() {
    let prices = [
        -5, -6, -3, 1, 5, 9, 7, 4, 1, -1, 3, 7, 11, 14, 12, 9, 6, 4, 6, 8, 9, 10, 8, 5, 2, 0, 4, 8,
        12, 16, 14, 12,
    ];
    let mut trace = String::new();
    for mirror in [false, true] {
        let direction = if mirror {
            MarketDirection::Downward
        } else {
            MarketDirection::Upward
        };
        let mut control = Replay::new(direction, 1);
        let mut inserted = Replay::new(direction, 1);
        writeln!(trace, "mirror|{mirror}").unwrap();
        for index in 0..128 {
            let price =
                (prices[index % prices.len()] + (index / prices.len()) as i64 * 3) as f64 * 0.125;
            let (low, high) = if mirror {
                (-price - 0.125, -price)
            } else {
                (price, price + 0.125)
            };
            let slot = index as i64 * 30;
            let mut control_trace = String::new();
            let (next, expected_delta) = control.push(bar(slot, low, high), &mut control_trace);
            control = next;
            let (next, delta) = inserted.push(bar(slot, low, high), &mut trace);
            inserted = next;
            assert_eq!(delta.lowest_movements, expected_delta.lowest_movements);
            assert_eq!(
                inserted.pipeline.lowest_movements,
                control.pipeline.lowest_movements
            );
            let tail = *inserted.pipeline.morphology.f1.combined.last().unwrap();
            let limit = match tail.direction {
                MarketDirection::Upward => tail.high,
                MarketDirection::Downward => tail.low,
                MarketDirection::NoNetDisplacement => unreachable!(),
            };
            for offset in [10, 20] {
                let before = inserted.pipeline.clone();
                let (next, delta) = inserted.push(bar(slot + offset, limit, limit), &mut trace);
                inserted = next;
                let mut expected_combined = before.morphology.f1.combined;
                let expected_tail = expected_combined.last_mut().unwrap();
                expected_tail.last_slot = time(slot + offset);
                expected_tail.known_at = time(slot + offset + 10);
                assert_eq!(inserted.pipeline.morphology.f1.combined, expected_combined);
                assert_eq!(
                    inserted.pipeline.morphology.f1.confirmed,
                    before.morphology.f1.confirmed
                );
                assert_eq!(
                    inserted.pipeline.morphology.stroke,
                    before.morphology.stroke
                );
                assert_eq!(inserted.pipeline.segment, before.segment);
                assert!(delta.morphology.f1.tail_updated);
                assert!(delta.morphology.f1.confirmed_fractal.is_none());
                assert!(delta.morphology.confirmed_stroke.is_none());
                assert!(delta.lowest_movements.is_empty());
                assert_eq!(
                    inserted.pipeline.lowest_movements,
                    control.pipeline.lowest_movements
                );
            }
        }
        assert_eq!(inserted.quality.bars.len(), 384);
        assert_eq!(inserted.pipeline.lowest_movements.len(), 7);
    }
    save("limit-board-pipeline", &trace);
}
