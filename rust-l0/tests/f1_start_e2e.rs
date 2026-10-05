//! 完整左边界失败面：首三笔无重叠仍发布、静默改选起点、未知方向被当作向下。
//! 所有场景从质量门逐 Bar 进入正式管线，保留可重放轨迹。

use chan_l0::input::{BarStreamEvent, BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{CompleteBoundary, advance_pipeline, new_pipeline_state};
use chan_l0::quality::{QualityGateDecision, apply_quality_event, empty_bar_quality_state};
use chrono::{DateTime, TimeDelta, Utc};
use std::fmt::Write;

fn time(index: usize) -> DateTime<Utc> {
    DateTime::from_timestamp(index as i64, 0).unwrap()
}

fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "SYNTHETIC".into(),
        instrument: "F1_START_CONTRACT".into(),
        timeframe: TimeDelta::seconds(1),
    }
}

fn input(index: usize, low: f64, high: f64) -> QualityBar {
    QualityBar {
        stream: stream(),
        slot: time(index),
        open: low,
        high,
        low,
        close: high,
        volume: 1,
        turnover: None,
        known_at: time(index + 1),
        closed: true,
    }
}

fn artifact(name: &str, trace: &str) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/e2e");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(name), trace).unwrap();
}

#[test]
fn invalid_first_three_cannot_publish_or_move_the_declared_root() {
    // 第65、77课：首三笔必须重叠。八根独立K保证这些拐点真正形成严格笔。
    // 10→20→0→5 的前三笔无公共交集；后续高点30不能把这个起点变合法。
    let pivots = [
        10.0, 20.0, 0.0, 5.0, -1.0, 30.0, 22.0, 28.0, 21.0, 26.0, 23.0, 29.0, 20.0, 27.0, 18.0,
        28.0,
    ];
    let mut prices = vec![11.0, pivots[0]];
    for pair in pivots.windows(2) {
        for step in 1..=8 {
            prices.push(pair[0] + (pair[1] - pair[0]) * step as f64 / 8.0);
        }
    }
    let mut trace = String::new();
    let mut results = Vec::new();
    for mirror in [false, true] {
        let boundary = CompleteBoundary {
            root_point: 1,
            initial_inclusion_direction: if mirror {
                MarketDirection::Downward
            } else {
                MarketDirection::Upward
            },
            known_at: time(1),
        };
        let mut pipeline = new_pipeline_state(stream(), boundary);
        let mut quality = empty_bar_quality_state(stream()).unwrap();
        let mut errors = Vec::new();
        for (index, price) in prices.iter().copied().enumerate() {
            let (low, high) = if mirror {
                (-price - 0.125, -price)
            } else {
                (price, price + 0.125)
            };
            let QualityGateDecision::GateAccepted {
                next,
                structure_material: Some(material),
                ..
            } = apply_quality_event(
                quality,
                BarStreamEvent::Append {
                    event_sequence: index as u64 + 1,
                    bar: input(index, low, high),
                },
            )
            else {
                panic!("valid Bar rejected");
            };
            quality = next;
            let prior_segment = pipeline.segment.clone();
            let prior_lowest = pipeline.lowest_movements.clone();
            match advance_pipeline(pipeline, &material) {
                Ok((next, _)) => pipeline = next,
                Err((next, reason)) => {
                    assert_eq!(next.segment, prior_segment);
                    assert_eq!(next.lowest_movements, prior_lowest);
                    assert_eq!(next.left_boundary, boundary);
                    assert_eq!(
                        next.morphology.f1.combined.last().unwrap().last_slot,
                        time(index)
                    );
                    errors.push((index, format!("{reason:?}")));
                    pipeline = next;
                }
            }
            writeln!(
                trace,
                "{mirror}|{index}|{low}|{high}|strokes={}|segments={}|l0={}|error={:?}",
                pipeline.morphology.stroke.confirmed.len(),
                pipeline.segment.completed.len(),
                pipeline.lowest_movements.len(),
                errors.last()
            )
            .unwrap();
        }
        results.push((errors, pipeline.lowest_movements.len()));
    }
    artifact("f1-invalid-first-three.txt", &trace);
    for (errors, published) in results {
        assert_eq!(
            published, 0,
            "an invalid initial segment must never become L0"
        );
        assert_eq!(errors.first().map(|(index, _)| *index), Some(35));
        assert!(
            errors
                .iter()
                .all(|(_, error)| error == "Segment(InvalidInitialOverlap)")
        );
        assert_eq!(errors.len(), prices.len() - 35);
    }
}

#[test]
fn unknown_inclusion_direction_is_rejected_without_assuming_downward() {
    let pipeline = new_pipeline_state(
        stream(),
        CompleteBoundary {
            root_point: 1,
            initial_inclusion_direction: MarketDirection::NoNetDisplacement,
            known_at: time(1),
        },
    );
    let quality = empty_bar_quality_state(stream()).unwrap();
    let QualityGateDecision::GateAccepted {
        structure_material: Some(material),
        ..
    } = apply_quality_event(
        quality,
        BarStreamEvent::Append {
            event_sequence: 1,
            bar: input(0, 10.0, 20.0),
        },
    )
    else {
        panic!("valid Bar rejected");
    };
    let result = advance_pipeline(pipeline.clone(), &material);
    artifact("f1-unknown-direction.txt", &format!("{result:?}\n"));
    let Err((preserved, error)) = result else {
        panic!("unknown boundary direction was accepted");
    };
    assert_eq!(preserved, pipeline);
    assert_eq!(format!("{error:?}"), "F1(InvalidInitialDirection)");
}
