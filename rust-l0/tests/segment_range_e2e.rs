//! 全区间失败面：不足成笔的反向波动、包含折叠掉的原始极值、终点以后来源误计入。
//! 原文78课要求保留实际区间，不能只取已选笔端点或标准化后的K线。

use chan_l0::input::{BarStreamEvent, BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{CompleteBoundary, advance_pipeline, new_pipeline_state};
use chan_l0::quality::{QualityGateDecision, apply_quality_event, empty_bar_quality_state};
use chrono::{DateTime, TimeDelta, Utc};
use std::fmt::Write;

fn time(index: usize) -> DateTime<Utc> {
    DateTime::from_timestamp(index as i64, 0).unwrap()
}

#[test]
fn full_range_keeps_unselected_raw_extremes_and_excludes_post_endpoint_bars() {
    let pivots = [0., 10., -1., 12., 5., 15., 4., 14., 3., 13., 2.];
    let mut trace = String::new();
    let mut failures = Vec::new();
    for absorb_extreme in [false, true] {
        let mut ranges = vec![(1., 1.125), (0., 0.125)];
        for (leg, pair) in pivots.windows(2).enumerate() {
            // 10→-1只隔两根合并K，不能形成反向笔；后续12会延伸0→10活动笔。
            let steps = if leg == 1 { 2 } else { 8 };
            for step in 1..=steps {
                let price = pair[0] + (pair[1] - pair[0]) * step as f64 / steps as f64;
                ranges.push((price, price + 0.125));
            }
            if absorb_extreme && leg == 2 {
                // 上升包含将这根的-2去掉，但它发生在本段端点之前，必须计入全区间。
                ranges.push((-2., 12.125));
            }
            if absorb_extreme && leg == 4 {
                // 原始终点15.125已在前一根出现；同价保持最早来源，这根在终点之后。
                ranges.push((-3., 15.125));
            }
        }
        for mirror in [false, true] {
            let stream = BarStreamIdentity {
                market: "SYNTHETIC".into(),
                instrument: "RAW_RANGE".into(),
                timeframe: TimeDelta::seconds(1),
            };
            let mut quality = empty_bar_quality_state(stream.clone()).unwrap();
            let mut pipeline = new_pipeline_state(
                stream.clone(),
                CompleteBoundary {
                    root_point: 1,
                    initial_inclusion_direction: if mirror {
                        MarketDirection::Downward
                    } else {
                        MarketDirection::Upward
                    },
                    known_at: time(1),
                },
            );
            for (index, (low, high)) in ranges.iter().copied().enumerate() {
                let (low, high) = if mirror { (-high, -low) } else { (low, high) };
                let input = QualityBar {
                    stream: stream.clone(),
                    slot: time(index),
                    open: low,
                    high,
                    low,
                    close: high,
                    volume: 1,
                    turnover: None,
                    known_at: time(index + 1),
                    closed: true,
                };
                let QualityGateDecision::GateAccepted {
                    next,
                    structure_material: Some(material),
                    ..
                } = apply_quality_event(
                    quality,
                    BarStreamEvent::Append {
                        event_sequence: index as u64 + 1,
                        bar: input,
                    },
                )
                else {
                    panic!("valid Bar rejected");
                };
                quality = next;
                let (next, delta) = advance_pipeline(pipeline, &material).unwrap();
                for movement in delta.lowest_movements {
                    writeln!(trace, "{absorb_extreme}|{mirror}|{index}|{movement:?}").unwrap();
                }
                pipeline = next;
            }
            let first_stroke = &pipeline.morphology.stroke.confirmed[0];
            assert_ne!(
                first_stroke.geometry.formation_end_index,
                first_stroke.geometry.end_index
            );
            let movement = pipeline.lowest_movements.first().unwrap();
            let expected_low = if absorb_extreme { -2. } else { -1. };
            let (low, high) = if mirror {
                (-15.125, -expected_low)
            } else {
                (expected_low, 15.125)
            };
            if movement.full_range.low != low || movement.full_range.high != high {
                failures.push(format!("absorb={absorb_extreme} mirror={mirror} expected=[{low},{high}] actual={movement:?}"));
            }
            assert_eq!(movement.start_endpoint.open_time, time(1));
            assert_eq!(
                movement.end_endpoint.open_time,
                time(if absorb_extreme { 36 } else { 35 })
            );
        }
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/e2e");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("f1-raw-segment-range.txt"), trace).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
