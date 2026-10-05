//! 原文规则的 Bar→质量门→F1→L0 回放。
//!
//! 失败面先固定：有缺口提前结束、第二序列递归确认、包含/同价制造假分型、
//! 首笔破坏后过早分段、候选新高不失效、内部极值遗漏、镜像及冻结前缀不一致。
//! 价格是原文结构的数值实例，不把示意图像素当作真实行情价格。

use chan_l0::f1::feature::{SegmentDirection, stroke_interval};
use chan_l0::f1::segment::RecognitionReason;
use chan_l0::input::{BarStreamEvent, BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{CompleteBoundary, advance_pipeline, new_pipeline_state};
use chan_l0::quality::{QualityGateDecision, apply_quality_event, empty_bar_quality_state};
use chrono::{DateTime, TimeDelta, Utc};
use std::fmt::Write;

struct Scenario {
    name: &'static str,
    pivots: &'static [f64],
    end_pivot: usize,
    first_publish_bar: usize,
    candidate_bar: usize,
    reason: RecognitionReason,
    full_low: f64,
}

fn time(index: usize) -> DateTime<Utc> {
    DateTime::from_timestamp(index as i64, 0).unwrap()
}

#[test]
fn original_segment_rules_hold_through_the_complete_bar_pipeline() {
    use RecognitionReason::{ImmediateNoGap, RepairedBySecondSequence};
    let scenarios = [
        Scenario {
            // 67/71课：E1=[5,10.125]、E2=[4,12.125]跨分界包含，不合并。
            name: "67-71-no-gap-cross-boundary-inclusion",
            pivots: &[0., 10., 5., 12., 4., 11., 3., 10., 2.],
            end_pivot: 3,
            first_publish_bar: 59,
            candidate_bar: 59,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // 71/81课及选定工程口径：第一序列右侧等高且更低仍能形成分界。
            name: "71-81-primary-equal-high",
            pivots: &[0., 10., 5., 12., 4., 12., 3., 10., 2.],
            end_pivot: 3,
            first_publish_bar: 59,
            candidate_bar: 59,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // 67/77课：旧缺口[10.125,15]未封闭，第二序列底分型已经足够。
            name: "67-77-gap-open-second-confirms",
            pivots: &[
                0., 10., 5., 20., 15., 18., 12., 17., 13., 19., 11., 18., 10., 22., 14.,
            ],
            end_pivot: 3,
            first_publish_bar: 83,
            candidate_bar: 59,
            reason: RepairedBySecondSequence,
            full_low: 0.,
        },
        Scenario {
            // 第二序列右元素越过旧顶时也完成严格底分型；反向段已获确认，不能抹掉。
            name: "78-second-fractal-and-new-high-in-same-stroke",
            pivots: &[
                0., 10., 5., 20., 15., 18., 12., 17., 13., 21., 11., 19., 10.,
            ],
            end_pivot: 3,
            first_publish_bar: 83,
            candidate_bar: 59,
            reason: RepairedBySecondSequence,
            full_low: 0.,
        },
        Scenario {
            // E1=[5,10.125]、E2=[10.125,20.125]单价相触，没有价格缺口。
            name: "67-first-two-touch-is-no-gap",
            pivots: &[0., 10., 5., 20., 10.125, 18., 9., 17., 8.],
            end_pivot: 3,
            first_publish_bar: 59,
            candidate_bar: 59,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // 77课：第二序列前两元素[15,18.125]、[11,14.125]有缺口，不能递归。
            name: "77-second-gap-does-not-recurse",
            pivots: &[0., 10., 5., 20., 15., 18., 11., 14., 12., 17., 11.5],
            end_pivot: 3,
            first_publish_bar: 83,
            candidate_bar: 59,
            reason: RepairedBySecondSequence,
            full_low: 0.,
        },
        Scenario {
            // 78课：第二序列普通包含先吸收[12,17.125]、[13,16.125]。
            name: "78-second-ordinary-inclusion",
            pivots: &[
                0., 10., 5., 20., 15., 18., 12., 17., 13., 16., 11., 15., 12., 16., 10.,
            ],
            end_pivot: 3,
            first_publish_bar: 115,
            candidate_bar: 59,
            reason: RepairedBySecondSequence,
            full_low: 0.,
        },
        Scenario {
            // 较早20.125候选仍在等待第二序列；内部19.125分界不能抢先发布。
            name: "67-pending-gap-blocks-later-lower-peak",
            pivots: &[
                0., 10., 5., 20., 15., 18., 12., 19., 13., 18., 11., 17., 12., 18., 10.,
            ],
            end_pivot: 3,
            first_publish_bar: 115,
            candidate_bar: 59,
            reason: RepairedBySecondSequence,
            full_low: 0.,
        },
        Scenario {
            // 较早12.125候选仍待真实突破3；内部11.125的分界同样不能抢先。
            name: "71-pending-real-break-blocks-later-lower-peak",
            pivots: &[
                0., 10., 5., 12., 3., 11., 4., 10., 3.5, 11., 3.25, 10., 3.125, 9., 2., 8., 1.,
            ],
            end_pivot: 3,
            first_publish_bar: 123,
            candidate_bar: 75,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // F16普通严格分型：[12,17.125]与[13,17.125]等高不能直接确认。
            name: "78-second-equal-high-is-not-fractal",
            pivots: &[
                0., 10., 5., 20., 15., 18., 12., 17., 13., 17., 11., 16., 12., 18., 10.,
            ],
            end_pivot: 3,
            first_publish_bar: 115,
            candidate_bar: 59,
            reason: RepairedBySecondSequence,
            full_low: 0.,
        },
        Scenario {
            // 71课：首笔12→3已破坏前段，3.5低于归并低点4仍不充分；须真实突破3。
            name: "71-first-break-waits-for-real-endpoint",
            pivots: &[0., 10., 5., 12., 3., 11., 4., 10., 3.5, 9., 2., 8., 1.],
            end_pivot: 3,
            first_publish_bar: 91,
            candidate_bar: 75,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // 仅回到首笔低点3不构成严格突破，直到2才确认；区别于缺口相触。
            name: "71-real-break-equality-still-waits",
            pivots: &[0., 10., 5., 12., 3., 11., 4., 10., 3., 9., 2., 8., 1.],
            end_pivot: 3,
            first_publish_bar: 91,
            candidate_bar: 75,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // 78课：第二序列分型出现前直接新高21，原20端点必须失效并继续。
            name: "78-gap-candidate-invalidated-by-new-high",
            pivots: &[
                0., 10., 5., 20., 15., 18., 12., 21., 16., 19., 13., 18., 14.,
            ],
            end_pivot: 7,
            first_publish_bar: 91,
            candidate_bar: 91,
            reason: ImmediateNoGap,
            full_low: 0.,
        },
        Scenario {
            // 78课：端点10→30.125，内部5仍必须纳入完整区间。
            name: "78-internal-extreme-is-not-endpoint",
            pivots: &[
                10., 20., 12., 25., 5., 24., 8., 30., 22., 28., 21., 27., 23.,
            ],
            end_pivot: 7,
            first_publish_bar: 91,
            candidate_bar: 91,
            reason: ImmediateNoGap,
            full_low: 5.,
        },
        Scenario {
            // 闭区间公共交集恰为10；区别于首段无重叠的反例，不能一并拒绝。
            name: "65-77-first-three-touch-at-one-price",
            pivots: &[
                10., 20., 0., 9.875, -1., 30., 22., 28., 21., 26., 23., 29., 20.,
            ],
            end_pivot: 5,
            first_publish_bar: 99,
            candidate_bar: 75,
            reason: RepairedBySecondSequence,
            full_low: -1.,
        },
    ];
    let mut trace = String::new();
    let mut failures = Vec::new();
    for scenario in scenarios {
        for mirror in [false, true] {
            let stream = BarStreamIdentity {
                market: "SYNTHETIC".into(),
                instrument: scenario.name.into(),
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
            let mut prices = vec![scenario.pivots[0] + 1., scenario.pivots[0]];
            for pair in scenario.pivots.windows(2) {
                for step in 1..=8 {
                    prices.push(pair[0] + (pair[1] - pair[0]) * step as f64 / 8.);
                }
            }
            // 最后拐点后再给两根K，冻结其分型；价格幅度远小于最后一腿。
            let last = *scenario.pivots.last().unwrap();
            let rising = last > scenario.pivots[scenario.pivots.len() - 2];
            for step in 1..=2 {
                prices.push(
                    last + if rising {
                        -(step as f64) * 0.25
                    } else {
                        step as f64 * 0.25
                    },
                );
            }
            let mut first_publish = None;
            for (index, price) in prices.into_iter().enumerate() {
                let (low, high) = if mirror {
                    (-price - 0.125, -price)
                } else {
                    (price, price + 0.125)
                };
                let bar = QualityBar {
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
                        bar,
                    },
                )
                else {
                    panic!("valid Bar rejected");
                };
                quality = next;
                let old_segments = pipeline.segment.completed.clone();
                let old_lowest = pipeline.lowest_movements.clone();
                let (next, delta) = advance_pipeline(pipeline, &material).unwrap();
                assert_eq!(&next.segment.completed[..old_segments.len()], old_segments);
                assert_eq!(&next.lowest_movements[..old_lowest.len()], old_lowest);
                assert_eq!(delta.segments.len(), delta.lowest_movements.len());
                for segment in &delta.segments {
                    first_publish.get_or_insert(index);
                    writeln!(trace, "{}|{mirror}|{index}|SEG|{segment:?}", scenario.name).unwrap();
                    let first = segment.begin_stroke_index;
                    let intervals: Vec<_> = next.morphology.stroke.confirmed[first..first + 3]
                        .iter()
                        .map(|s| stroke_interval(&next.morphology.f1, s).unwrap())
                        .collect();
                    assert!(
                        intervals
                            .iter()
                            .map(|r| r.low)
                            .fold(f64::NEG_INFINITY, f64::max)
                            <= intervals
                                .iter()
                                .map(|r| r.high)
                                .fold(f64::INFINITY, f64::min)
                    );
                    assert!((segment.end_stroke_index - first) >= 3);
                    assert_eq!((segment.end_stroke_index - first) % 2, 1);
                }
                writeln!(
                    trace,
                    "{}|{mirror}|{index}|{low}|{high}|strokes={}|l0={}",
                    scenario.name,
                    next.morphology.stroke.confirmed.len(),
                    next.lowest_movements.len()
                )
                .unwrap();
                pipeline = next;
            }
            // 检查夹具实际走过预定的完整笔序列，避免小波动被笔规则过滤后误判覆盖。
            for (index, stroke) in pipeline.morphology.stroke.confirmed.iter().enumerate() {
                let begin = &pipeline.morphology.f1.confirmed[stroke.geometry.begin_index].point;
                let end = &pipeline.morphology.f1.confirmed[stroke.geometry.end_index].point;
                assert_eq!(begin.center_index, 1 + index * 8, "{}", scenario.name);
                assert_eq!(end.center_index, 1 + (index + 1) * 8, "{}", scenario.name);
            }
            let first = pipeline.segment.completed.first();
            if scenario.name == "67-77-gap-open-second-confirms" {
                let chain: Vec<_> = pipeline
                    .segment
                    .completed
                    .iter()
                    .take(3)
                    .map(|s| {
                        (
                            s.begin_stroke_index,
                            s.end_stroke_index,
                            s.times.confirmed_at,
                            s.reason,
                        )
                    })
                    .collect();
                assert_eq!(
                    chain,
                    vec![
                        (0, 3, time(84), RepairedBySecondSequence),
                        (3, 6, time(84), ImmediateNoGap),
                        (6, 9, time(108), ImmediateNoGap),
                    ]
                );
            }
            let expected_high = scenario.pivots[scenario.end_pivot] + 0.125;
            let valid = first_publish == Some(scenario.first_publish_bar)
                && first.is_some_and(|s| {
                    s.begin_stroke_index == 0
                        && s.end_stroke_index == scenario.end_pivot
                        && s.reason == scenario.reason
                        && s.direction
                            == if mirror {
                                SegmentDirection::Down
                            } else {
                                SegmentDirection::Up
                            }
                        && s.times.candidate_known_at == time(scenario.candidate_bar + 1)
                        && s.times.confirmed_at == time(scenario.first_publish_bar + 1)
                        && s.start_price
                            == if mirror {
                                -scenario.pivots[0]
                            } else {
                                scenario.pivots[0]
                            }
                        && s.end_price
                            == if mirror {
                                -expected_high
                            } else {
                                expected_high
                            }
                        && s.price_interval.low
                            == if mirror {
                                -expected_high
                            } else {
                                scenario.full_low
                            }
                        && s.price_interval.high
                            == if mirror {
                                -scenario.full_low
                            } else {
                                expected_high
                            }
                });
            if !valid {
                failures.push(format!("{} mirror={mirror}: expected end={} candidate_bar={} publication_bar={} reason={:?}, actual first_bar={first_publish:?} first={first:?}",
                    scenario.name, scenario.end_pivot, scenario.candidate_bar, scenario.first_publish_bar, scenario.reason));
            }
        }
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/e2e");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("f1-original-rules.txt"), trace).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
