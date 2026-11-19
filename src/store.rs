// Copyright (C) 2026 NetSalmon
// SPDX-License-Identifier: AGPL-3.0-only

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{BoardItem, Circle, Stats};

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("circles"))?;
        Ok(Self { root })
    }

    pub fn save(&self, circle: &Circle) -> std::io::Result<()> {
        let path = self.circle_path(&circle.id)?;
        let json = serde_json::to_vec_pretty(circle).map_err(std::io::Error::other)?;
        fs::write(path, json)
    }

    pub fn load(&self, id: &str) -> std::io::Result<Option<Circle>> {
        let path = self.circle_path(id)?;
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        let circle: Circle = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        if !circle.persist {
            let week = 7 * 86_400;
            if now_secs().saturating_sub(circle.created_at) > week {
                let _ = fs::remove_file(self.circle_path(id)?);
                return Ok(None);
            }
        }
        Ok(Some(circle))
    }

    pub fn leaderboard(&self) -> std::io::Result<Vec<BoardItem>> {
        let mut items = Vec::new();
        for circle in self.persisted()? {
            items.push(BoardItem {
                id: circle.id,
                username: circle.username,
                display_name: circle.display_name,
                user_count: circle.users.len(),
                total_score: circle.market.total_score,
                created_at: circle.created_at,
                persist: circle.persist,
            });
        }
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items.truncate(40);
        Ok(items)
    }

    pub fn stats(&self) -> std::io::Result<Stats> {
        let today = now_secs().div_euclid(86_400);
        let mut circles = 0u64;
        let mut today_n = 0u64;
        let mut interactions = 0u64;
        for circle in self.persisted()? {
            circles += 1;
            if circle.created_at.div_euclid(86_400) == today {
                today_n += 1;
            }
            interactions += u64::from(circle.market.total_count);
        }
        Ok(Stats {
            circles,
            today: today_n,
            interactions,
        })
    }

    pub fn feedback(&self, text: &str, contact: &str) -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("feedback.jsonl"))?;
        let line = serde_json::json!({
            "at": now_secs(),
            "text": text,
            "contact": contact,
        });
        writeln!(file, "{line}")
    }

    fn persisted(&self) -> std::io::Result<Vec<Circle>> {
        let mut out = Vec::new();
        let dir = self.root.join("circles");
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if let Some(circle) = self.load(id)? {
                if circle.persist {
                    out.push(circle);
                }
            }
        }
        Ok(out)
    }

    fn circle_path(&self, id: &str) -> std::io::Result<PathBuf> {
        if !valid_id(id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid circle id",
            ));
        }
        Ok(self.root.join("circles").join(format!("{id}.json")))
    }
}

pub fn valid_id(id: &str) -> bool {
    id.len() == 8
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b.is_ascii_lowercase() && b != b'l' && b != b'o'))
        || id == "demo"
}

pub fn new_id() -> String {
    const ABC: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut n = now_secs() as u128;
    n ^= (std::process::id() as u128) << 48;
    n = n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut s = String::with_capacity(8);
    for _ in 0..8 {
        let idx = (n % 32) as usize;
        s.push(ABC[idx] as char);
        n /= 32;
        n ^= n >> 11;
        n = n.wrapping_mul(0x1000_0000_01B3);
    }
    s
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
