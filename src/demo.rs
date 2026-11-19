use crate::model::{Circle, Dir, Event, Kind};
use crate::score::{build_users, market_of};

/// A fixed sample board. Daily counts are dense enough that the 1-day color
/// scale shows the same mild reds and greens as a market map, while a few
/// names sit outside ±4% and go fully saturated.
pub fn demo_circle(now: i64) -> Circle {
    let names = [
        ("orbit_ada", "艾达"),
        ("orbit_lin", "林深"),
        ("orbit_qiao", "乔麦"),
        ("orbit_he", "何晚"),
        ("orbit_su", "苏小小"),
        ("orbit_chen", "陈北"),
        ("orbit_zhou", "周南"),
        ("orbit_wu", "吴澄"),
        ("orbit_zheng", "郑野"),
        ("orbit_xu", "许星"),
        ("orbit_sun", "孙桥"),
        ("orbit_ma", "马灯"),
        ("orbit_hu", "胡桃"),
        ("orbit_gao", "高岭"),
        ("orbit_tang", "唐果"),
        ("orbit_feng", "冯迟"),
        ("orbit_cao", "曹白"),
        ("orbit_deng", "邓川"),
        ("orbit_peng", "彭远"),
        ("orbit_luo", "罗盘"),
        ("orbit_song", "宋纸"),
        ("orbit_han", "韩潮"),
        ("orbit_jiang", "江帆"),
        ("orbit_shen", "沈舟"),
    ];
    // Most names move less than 4%. A few are the deep blocks.
    let drift = [
        0.012, -0.018, 0.031, -0.007, 0.004, -0.055, 0.062, 0.0, -0.026, 0.015, -0.011, 0.008,
        -0.003, 0.022, -0.041, 0.002, -0.016, 0.009, 0.048, -0.033,
    ];
    let mut events = Vec::new();

    for i in 0..108 {
        let (handle, name) = names[i % names.len()];
        let handle = if i < names.len() {
            handle.to_string()
        } else {
            format!("{handle}{:02}", i / names.len())
        };
        let name = if i < names.len() {
            name.to_string()
        } else {
            format!("{name}{}", i / names.len())
        };
        let size = 70.0 / (i as f64 + 1.0).powf(0.78);
        let pct = drift[i % drift.len()];
        let flow_tilt: f64 = [
            0.06, -0.05, 0.11, -0.08, 0.03, -0.2, 0.24, -0.02, 0.15, -0.13,
        ][i % 10];
        let damp = if i < 14 { 0.42 } else { 1.0 };
        let in_bias: f64 = (0.5 + flow_tilt * damp).clamp(0.08, 0.92);
        let kind = match i % 5 {
            0 | 1 => Kind::Reply,
            2 => Kind::Mention,
            3 => Kind::Quote,
            _ => Kind::Repost,
        };

        for day in 0..50 {
            let n = if day <= 1 {
                let base = (10.0 + size * 0.45).round() as i32;
                let mut delta = (base as f64 * pct).round() as i32;
                if delta == 0 && pct.abs() > 0.001 {
                    delta = if pct > 0.0 { 1 } else { -1 };
                }
                (if day == 0 { base + delta } else { base }).max(0) as u32
            } else {
                let fade = if day < 10 { 1.0 } else { 0.28 };
                let tilt = 1.0 + pct * (1.0 - day as f64 / 5.0).clamp(-1.2, 1.2);
                (size * 0.22 * fade * tilt).round().max(0.0) as u32
            };
            for k in 0..n {
                let mix = (i as u32)
                    .wrapping_mul(2_246_822_519)
                    .wrapping_add((day as u32).wrapping_mul(3_266_489_917))
                    .wrapping_add(k.wrapping_mul(668_265_263));
                let dir = if mix % 100 < (in_bias * 100.0) as u32 {
                    Dir::In
                } else {
                    Dir::Out
                };
                let kind = if k == 0 {
                    kind
                } else {
                    match (i + k as usize + day as usize) % 10 {
                        0 | 1 | 2 | 3 => Kind::Reply,
                        4 | 5 => Kind::Quote,
                        6 | 7 | 8 => Kind::Mention,
                        _ => Kind::Repost,
                    }
                };
                events.push(Event {
                    peer: handle.clone(),
                    name: name.clone(),
                    avatar: String::new(),
                    dir,
                    kind,
                    at: now - day * 86_400 - (i as i64 * 17 + k as i64 * 13) % 70_000,
                    engagement: 4.0 + (i as f64 * 1.7 + day as f64) % 40.0,
                });
            }
        }
    }

    let users = build_users(&events, now);
    let market = market_of(&users);
    Circle {
        id: "demo".to_string(),
        username: "demo".to_string(),
        display_name: "示例盘面".to_string(),
        avatar: String::new(),
        created_at: now,
        persist: false,
        source: "demo".to_string(),
        tweet_count: events.len() as u32,
        users,
        market,
    }
}
