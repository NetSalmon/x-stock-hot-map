// Copyright (C) 2026 NetSalmon
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;

use crate::model::{Dir, Event, Kind, MarketAgg, UserOut, Zdf};

const HALF_LIFE_DAYS: f64 = 14.0;
const SERIES_DAYS: usize = 120;
const MUTUAL_BONUS: f64 = 1.2;

pub fn build_users(events: &[Event], now: i64) -> Vec<UserOut> {
    struct Acc {
        name: String,
        avatar: String,
        in_score: f64,
        out_score: f64,
        in_count: u32,
        out_count: u32,
        heat: f64,
        last_at: i64,
        kind_score: [f64; 4],
        series: [f64; SERIES_DAYS],
        ytd_count: f64,
        before_count: f64,
    }

    let mut map: HashMap<String, Acc> = HashMap::new();
    let year_start = year_start_unix(now);

    for ev in events {
        let handle = ev.peer.trim().trim_start_matches('@').to_ascii_lowercase();
        if handle.is_empty() {
            continue;
        }
        let age_days = ((now - ev.at) as f64 / 86400.0).max(0.0);
        let decayed = ev.kind.weight() * 0.5_f64.powf(age_days / HALF_LIFE_DAYS);
        let acc = map.entry(handle).or_insert_with(|| Acc {
            name: String::new(),
            avatar: String::new(),
            in_score: 0.0,
            out_score: 0.0,
            in_count: 0,
            out_count: 0,
            heat: 0.0,
            last_at: 0,
            kind_score: [0.0; 4],
            series: [0.0; SERIES_DAYS],
            ytd_count: 0.0,
            before_count: 0.0,
        });
        if acc.name.is_empty() && !ev.name.is_empty() {
            acc.name = ev.name.clone();
        }
        if acc.avatar.is_empty() && !ev.avatar.is_empty() {
            acc.avatar = ev.avatar.clone();
        }
        match ev.dir {
            Dir::In => {
                acc.in_score += decayed;
                acc.in_count += 1;
            }
            Dir::Out => {
                acc.out_score += decayed;
                acc.out_count += 1;
            }
        }
        acc.heat += ev.engagement.max(0.0);
        acc.last_at = acc.last_at.max(ev.at);
        acc.kind_score[kind_index(ev.kind)] += decayed;
        if ev.at >= year_start {
            acc.ytd_count += 1.0;
        } else if ev.at > 0 {
            acc.before_count += 1.0;
        }
        if ev.at > 0 && now >= ev.at {
            let day = ((now - ev.at) / 86400) as usize;
            if day < SERIES_DAYS {
                acc.series[day] += 1.0;
            }
        }
    }

    let mut users: Vec<UserOut> = map
        .into_iter()
        .filter_map(|(handle, acc)| {
            let mut score = acc.in_score + acc.out_score;
            if score <= 0.0 {
                return None;
            }
            if acc.in_count > 0 && acc.out_count > 0 {
                score *= MUTUAL_BONUS;
            }
            let total_dir = acc.in_score + acc.out_score;
            let flow = if total_dir <= f64::EPSILON {
                0.0
            } else {
                (acc.in_score - acc.out_score) / total_dir * 100.0
            };
            let count = acc.in_count + acc.out_count;
            let sector2 = dominant_kind(&acc.kind_score).label().to_string();
            let sector1 = sector_direction(acc.in_count, acc.out_count, flow, count, &sector2);
            let recency_days = if acc.last_at <= 0 {
                999
            } else {
                ((now - acc.last_at) / 86400).max(0)
            };
            let sector3 = if recency_days <= 3 {
                "近端"
            } else if recency_days <= 20 {
                "中端"
            } else {
                "远端"
            }
            .to_string();
            let series = acc.series.to_vec();
            let zdf = Zdf {
                d1: window_change(&series, 1),
                d3: window_change(&series, 3),
                d5: window_change(&series, 5),
                d10: window_change(&series, 10),
                d20: window_change(&series, 20),
                d60: window_change(&series, 60),
                ytd: ytd_change(acc.ytd_count, acc.before_count, now, year_start),
                flow,
            };
            Some(UserOut {
                handle,
                name: if acc.name.is_empty() {
                    String::new()
                } else {
                    acc.name
                },
                avatar: acc.avatar,
                sector1,
                sector2,
                sector3,
                score: round2(score),
                count,
                in_score: round2(acc.in_score),
                out_score: round2(acc.out_score),
                in_count: acc.in_count,
                out_count: acc.out_count,
                heat: round2(acc.heat),
                heat_ratio: 1.0,
                last_at: acc.last_at,
                recency_days,
                series,
                zdf,
            })
        })
        .collect();

    let mut ratios: Vec<f64> = users
        .iter()
        .map(|u| {
            if u.count == 0 {
                0.0
            } else {
                u.heat / f64::from(u.count)
            }
        })
        .filter(|v| *v > 0.0)
        .collect();
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if ratios.is_empty() {
        1.0
    } else {
        ratios[ratios.len() / 2].max(0.01)
    };
    for u in &mut users {
        let per = if u.count == 0 {
            0.0
        } else {
            u.heat / f64::from(u.count)
        };
        u.heat_ratio = round2(per / median);
        if u.name.is_empty() {
            u.name = u.handle.clone();
        }
    }

    balance_large_sectors(&mut users);

    users.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.handle.cmp(&b.handle))
    });
    users
}

pub fn market_of(users: &[UserOut]) -> MarketAgg {
    let mut series = vec![0.0; SERIES_DAYS];
    let mut flow_num = 0.0;
    let mut flow_den = 0.0;
    let mut ytd_num = 0.0;
    let mut ytd_den = 0.0;
    let mut total_score = 0.0;
    let mut total_count = 0u32;
    for u in users {
        for (i, v) in u.series.iter().enumerate() {
            if i < series.len() {
                series[i] += v;
            }
        }
        flow_num += u.in_score - u.out_score;
        flow_den += u.in_score + u.out_score;
        ytd_num += u.zdf.ytd * f64::from(u.count);
        ytd_den += f64::from(u.count);
        total_score += u.score;
        total_count += u.count;
    }
    let flow = if flow_den <= f64::EPSILON {
        0.0
    } else {
        flow_num / flow_den * 100.0
    };
    MarketAgg {
        d1: window_change(&series, 1),
        d3: window_change(&series, 3),
        d5: window_change(&series, 5),
        d10: window_change(&series, 10),
        d20: window_change(&series, 20),
        d60: window_change(&series, 60),
        ytd: if ytd_den <= f64::EPSILON {
            0.0
        } else {
            round2(ytd_num / ytd_den)
        },
        flow: round2(flow),
        total_score: round2(total_score),
        total_count,
    }
}

fn sector_direction(in_count: u32, out_count: u32, flow: f64, count: u32, kind: &str) -> String {
    // One-off mentions stay in their own channel so a celebrity reply-cloud
    // does not swallow the whole map. Repeated ties split by net flow, with a
    // narrow balanced band so mild reds and greens can still sit in 流入/流出.
    if count < 2 && (in_count == 0 || out_count == 0) {
        kind.to_string()
    } else if in_count > 0 && out_count > 0 && flow.abs() <= 4.0 {
        "双向核心".to_string()
    } else if flow > 8.0 {
        "流入为主".to_string()
    } else if flow < -8.0 {
        "流出为主".to_string()
    } else {
        kind.to_string()
    }
}

/// A celebrity mention-cloud would otherwise be one slab. Split an oversized
/// board into heat bands so the map still has several sectors.
fn balance_large_sectors(users: &mut [UserOut]) {
    if users.len() < 12 {
        return;
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for user in users.iter() {
        *counts.entry(user.sector1.clone()).or_default() += 1;
    }
    let Some((big, n)) = counts.into_iter().max_by_key(|(_, n)| *n) else {
        return;
    };
    if n < 12 || n * 2 < users.len() || !matches!(big.as_str(), "提及" | "回复" | "引用" | "转发")
    {
        return;
    }
    let mut heats: Vec<f64> = users
        .iter()
        .filter(|user| user.sector1 == big)
        .map(|user| user.heat)
        .collect();
    heats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo = heats[heats.len() / 3];
    let hi = heats[heats.len() * 2 / 3];
    let by_heat = hi > lo;
    let mut ages: Vec<i64> = users
        .iter()
        .filter(|user| user.sector1 == big)
        .map(|user| user.recency_days)
        .collect();
    ages.sort_unstable();
    let age_lo = ages[ages.len() / 3];
    let age_hi = ages[ages.len() * 2 / 3];
    for user in users.iter_mut().filter(|user| user.sector1 == big) {
        let band = if by_heat {
            if user.heat >= hi {
                "高热"
            } else if user.heat <= lo {
                "低热"
            } else {
                "中热"
            }
        } else if age_hi > age_lo {
            if user.recency_days <= age_lo {
                "近端"
            } else if user.recency_days >= age_hi {
                "远端"
            } else {
                "中端"
            }
        } else {
            ""
        };
        if !band.is_empty() {
            user.sector1 = format!("{band}{big}");
        }
    }
}

fn dominant_kind(scores: &[f64; 4]) -> Kind {
    let mut best = 0;
    for i in 1..4 {
        if scores[i] > scores[best] {
            best = i;
        }
    }
    [Kind::Reply, Kind::Quote, Kind::Mention, Kind::Repost][best]
}

fn kind_index(kind: Kind) -> usize {
    match kind {
        Kind::Reply => 0,
        Kind::Quote => 1,
        Kind::Mention => 2,
        Kind::Repost => 3,
    }
}

/// Percent change of interaction counts: recent window versus the window before it.
/// Previous window empty and recent activity present reads as +100.
pub fn window_change(series: &[f64], days: usize) -> f64 {
    if days == 0 {
        return 0.0;
    }
    let cur: f64 = series.iter().take(days).sum();
    let prev: f64 = series.iter().skip(days).take(days).sum();
    round2(ratio_change(cur, prev))
}

fn ytd_change(ytd: f64, before: f64, now: i64, year_start: i64) -> f64 {
    let ytd_days = ((now - year_start) / 86400).max(1) as f64;
    let before_days = 365.0;
    let ytd_rate = ytd / ytd_days;
    let before_rate = before / before_days;
    round2(ratio_change(ytd_rate, before_rate))
}

fn ratio_change(cur: f64, prev: f64) -> f64 {
    if cur <= 0.0 && prev <= 0.0 {
        0.0
    } else if prev <= 0.0 {
        100.0
    } else {
        (cur - prev) / prev * 100.0
    }
}

fn year_start_unix(now: i64) -> i64 {
    // Civil year in UTC. Good enough for a public-interaction window.
    let days = now.div_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let year = if mp < 10 { y } else { y + 1 };
    civil_to_unix(year, 1, 1)
}

fn civil_to_unix(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    let days = era * 146097 + doe as i64 - 719468;
    days * 86400
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(peer: &str, dir: Dir, kind: Kind, at: i64) -> Event {
        Event {
            peer: peer.into(),
            name: peer.into(),
            avatar: String::new(),
            dir,
            kind,
            at,
            engagement: 10.0,
        }
    }

    #[test]
    fn mutual_bonus_and_inflow_color() {
        let now = 1_763_000_000;
        let events = vec![
            ev("Ada", Dir::In, Kind::Reply, now - 3600),
            ev("ada", Dir::Out, Kind::Reply, now - 7200),
            ev("bob", Dir::Out, Kind::Quote, now - 86_400 * 10),
            ev("bob", Dir::Out, Kind::Quote, now - 86_400 * 11),
            ev("bob", Dir::Out, Kind::Quote, now - 86_400 * 12),
        ];
        let users = build_users(&events, now);
        let ada = users.iter().find(|u| u.handle == "ada").unwrap();
        assert_eq!(ada.sector1, "双向核心");
        assert!(ada.score > ada.in_score + ada.out_score);
        assert!(ada.zdf.flow > 0.0);
        let bob = users.iter().find(|u| u.handle == "bob").unwrap();
        assert_eq!(bob.sector1, "流出为主");
        assert_eq!(bob.sector2, "引用");
        assert!(bob.zdf.flow < 0.0);
    }

    #[test]
    fn window_change_compares_adjacent_spans() {
        let mut series = vec![0.0; 10];
        series[0] = 3.0;
        series[1] = 1.0;
        assert_eq!(window_change(&series, 1), 200.0);
        series[1] = 0.0;
        assert_eq!(window_change(&series, 1), 100.0);
    }

    #[test]
    fn year_start_is_utc_jan_1() {
        // 2024-06-15T00:00:00Z
        assert_eq!(year_start_unix(1_718_409_600), 1_704_067_200);
    }

    #[test]
    fn weak_one_off_is_not_a_core_sector() {
        let now = 1_700_000_000;
        let users = build_users(&[ev("solo", Dir::In, Kind::Mention, now - 100)], now);
        assert_eq!(users[0].sector1, "提及");
        assert_eq!(users[0].count, 1);
    }
}
