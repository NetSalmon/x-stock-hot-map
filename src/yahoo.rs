// Copyright (C) 2026 NetSalmon
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashSet;
use std::time::Duration;

use serde_json::Value;

use crate::model::{Dir, Event, Kind};

const PAGES: i64 = 4;
const PAGE_SIZE: i64 = 40;

pub async fn collect(
    client: &reqwest::Client,
    username: &str,
) -> Result<(Vec<Event>, u32, String, String), String> {
    let user = username.to_ascii_lowercase();
    let inbound = format!("@{user}");
    let outbound = format!("ID:{user}");

    let (inn, out) = tokio::join!(search_all(client, &inbound), search_all(client, &outbound),);
    let inn = inn?;
    let out = out?;

    let mut seen = HashSet::new();
    let mut events = Vec::new();
    let mut display_name = String::new();
    let mut avatar = String::new();
    let mut tweets = 0u32;

    for entry in &out {
        if seen.insert(entry_id(entry)) {
            tweets += 1;
        }
        if display_name.is_empty() {
            if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    display_name = name.to_string();
                }
            }
        }
        if avatar.is_empty() {
            avatar = profile_image(entry);
        }
        push_outbound(&mut events, entry, &user);
    }
    for entry in &inn {
        let id = entry_id(entry);
        if !seen.insert(id) {
            continue;
        }
        tweets += 1;
        push_inbound(&mut events, entry, &user);
    }

    if display_name.is_empty() {
        display_name = user.clone();
    }
    Ok((events, tweets, display_name, avatar))
}

async fn search_all(client: &reqwest::Client, query: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for page in 0..PAGES {
        let start = 1 + page * PAGE_SIZE;
        let batch = search_page(client, query, start).await?;
        let n = batch.len();
        out.extend(batch);
        if (n as i64) < PAGE_SIZE {
            break;
        }
    }
    Ok(out)
}

async fn search_page(
    client: &reqwest::Client,
    query: &str,
    start: i64,
) -> Result<Vec<Value>, String> {
    let response = client
        .get("https://search.yahoo.co.jp/realtime/api/v1/pagination")
        .query(&[
            ("p", query),
            ("results", "40"),
            ("start", &start.to_string()),
        ])
        .header("accept", "application/json")
        .header("accept-language", "ja,en;q=0.8,zh;q=0.7")
        .timeout(Duration::from_secs(18))
        .send()
        .await
        .map_err(|err| format!("公开互动源暂时不可用：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("公开互动源返回 {}（{query}）", response.status()));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|err| format!("公开互动源数据无法解析：{err}"))?;
    let entries = body
        .pointer("/timeline/entry")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(entries)
}

fn push_inbound(events: &mut Vec<Event>, entry: &Value, user: &str) {
    let author = screen(entry);
    if author.is_empty() || author == user {
        return;
    }
    let at = created_at(entry);
    let engagement = engagement_of(entry);
    let name = text_field(entry, "name");
    let avatar = profile_image(entry);
    let quoted = quoted_screen(entry);
    let reply_targets = names(entry, "replyMentions");
    let mentions = names(entry, "mentions");
    let text = clean_text(
        entry
            .get("displayText")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );
    let replies_to_user = reply_targets.iter().any(|name| name == user);

    let mut peers: Vec<(String, Kind)> = Vec::new();
    if quoted.as_deref() == Some(user) {
        peers.push((author.clone(), Kind::Quote));
    } else if replies_to_user {
        peers.push((author.clone(), Kind::Reply));
    } else if mentions.iter().any(|name| name == user) || text_mentions(&text, user) {
        peers.push((author.clone(), Kind::Mention));
    } else if is_repost_of(&text, user) {
        peers.push((author, Kind::Repost));
    }
    for (peer, kind) in peers {
        events.push(Event {
            peer,
            name: name.clone(),
            avatar: avatar.clone(),
            dir: Dir::In,
            kind,
            at,
            engagement,
        });
    }
}

fn push_outbound(events: &mut Vec<Event>, entry: &Value, user: &str) {
    let author = screen(entry);
    if !author.is_empty() && author != user {
        return;
    }
    let at = created_at(entry);
    let engagement = engagement_of(entry);
    let text = clean_text(
        entry
            .get("displayText")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );
    let mut peers: Vec<(String, Kind, String, String)> = Vec::new();

    if let Some(quoted) = quoted_block(entry) {
        let peer = screen(&quoted);
        if !peer.is_empty() && peer != user {
            peers.push((
                peer,
                Kind::Quote,
                text_field(&quoted, "name"),
                profile_image(&quoted),
            ));
        }
    }
    let reply_targets = names(entry, "replyMentions");
    let is_reply = entry
        .get("inReplyTo")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty())
        || !reply_targets.is_empty();
    if is_reply {
        for peer in reply_targets {
            if peer != user && !peers.iter().any(|(name, _, _, _)| name == &peer) {
                peers.push((peer, Kind::Reply, String::new(), String::new()));
            }
        }
    }
    for peer in names(entry, "mentions") {
        if peer != user && !peers.iter().any(|(name, _, _, _)| name == &peer) {
            let kind = if is_reply { Kind::Reply } else { Kind::Mention };
            peers.push((peer, kind, String::new(), String::new()));
        }
    }
    if let Some(peer) = repost_target(&text) {
        if peer != user && !peers.iter().any(|(name, _, _, _)| name == &peer) {
            peers.push((peer, Kind::Repost, String::new(), String::new()));
        }
    }

    for (peer, kind, name, avatar) in peers {
        events.push(Event {
            peer,
            name,
            avatar,
            dir: Dir::Out,
            kind,
            at,
            engagement,
        });
    }
}

fn entry_id(entry: &Value) -> String {
    entry
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn screen(entry: &Value) -> String {
    text_field(entry, "screenName")
        .trim()
        .trim_start_matches('@')
        .to_ascii_lowercase()
}

fn text_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn profile_image(entry: &Value) -> String {
    text_field(entry, "profileImage")
}

fn created_at(entry: &Value) -> i64 {
    entry.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(0)
}

fn engagement_of(entry: &Value) -> f64 {
    fn num(entry: &Value, key: &str) -> f64 {
        entry
            .get(key)
            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)))
            .unwrap_or(0.0)
    }
    num(entry, "likesCount")
        + num(entry, "rtCount")
        + num(entry, "qtCount")
        + num(entry, "replyCount")
}

fn names(entry: &Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("screenName").and_then(|v| v.as_str()))
                .map(|s| s.trim().trim_start_matches('@').to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn quoted_block(entry: &Value) -> Option<Value> {
    entry.get("quotedTweet").filter(|v| v.is_object()).cloned()
}

fn quoted_screen(entry: &Value) -> Option<String> {
    quoted_block(entry)
        .map(|q| screen(&q))
        .filter(|s| !s.is_empty())
}

fn clean_text(raw: &str) -> String {
    raw.replace("\tSTART\t", "").replace("\tEND\t", "")
}

fn text_mentions(text: &str, user: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let needle = format!("@{user}");
    lower.contains(&needle)
}

fn is_repost_of(text: &str, user: &str) -> bool {
    repost_target(text).as_deref() == Some(user)
}

fn repost_target(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    let rest = trimmed
        .strip_prefix("RT @")
        .or_else(|| trimmed.strip_prefix("rt @"))
        .or_else(|| trimmed.strip_prefix("Rt @"))?;
    let handle: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if handle.is_empty() {
        None
    } else {
        Some(handle.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inbound_reply_and_outbound_quote() {
        let inbound = json!({
            "id": "1",
            "screenName": "Ada",
            "name": "Ada Lovelace",
            "profileImage": "https://example.test/a.jpg",
            "createdAt": 100,
            "inReplyTo": "9",
            "displayText": "@nasa hello",
            "mentions": [{"screenName": "NASA"}],
            "replyMentions": [{"screenName": "nasa"}],
            "likesCount": 3,
            "rtCount": 1,
            "qtCount": 0,
            "replyCount": 0
        });
        let outbound = json!({
            "id": "2",
            "screenName": "nasa",
            "name": "NASA",
            "createdAt": 200,
            "displayText": "quote",
            "quotedTweet": {"screenName": "Bob", "name": "Bob"},
            "mentions": [],
            "likesCount": 4
        });
        let mut events = Vec::new();
        push_inbound(&mut events, &inbound, "nasa");
        push_outbound(&mut events, &outbound, "nasa");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].peer, "ada");
        assert_eq!(events[0].kind, Kind::Reply);
        assert_eq!(events[0].dir, Dir::In);
        assert_eq!(events[0].engagement, 4.0);
        assert_eq!(events[1].peer, "bob");
        assert_eq!(events[1].kind, Kind::Quote);
    }
}
