use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Reply,
    Quote,
    Mention,
    Repost,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Reply => "回复",
            Kind::Quote => "引用",
            Kind::Mention => "提及",
            Kind::Repost => "转发",
        }
    }

    pub fn weight(self) -> f64 {
        match self {
            Kind::Reply => 5.0,
            Kind::Quote => 3.5,
            Kind::Mention => 2.0,
            Kind::Repost => 1.2,
        }
    }
}

/// One public interaction between the target account and a peer.
#[derive(Debug, Clone)]
pub struct Event {
    pub peer: String,
    pub name: String,
    pub avatar: String,
    pub dir: Dir,
    pub kind: Kind,
    pub at: i64,
    pub engagement: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zdf {
    pub d1: f64,
    pub d3: f64,
    pub d5: f64,
    pub d10: f64,
    pub d20: f64,
    pub d60: f64,
    pub ytd: f64,
    pub flow: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserOut {
    pub handle: String,
    pub name: String,
    pub avatar: String,
    pub sector1: String,
    pub sector2: String,
    pub sector3: String,
    pub score: f64,
    pub count: u32,
    pub in_score: f64,
    pub out_score: f64,
    pub in_count: u32,
    pub out_count: u32,
    pub heat: f64,
    pub heat_ratio: f64,
    pub last_at: i64,
    pub recency_days: i64,
    /// Raw daily interaction counts, index 0 = today, length 120.
    pub series: Vec<f64>,
    pub zdf: Zdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketAgg {
    pub d1: f64,
    pub d3: f64,
    pub d5: f64,
    pub d10: f64,
    pub d20: f64,
    pub d60: f64,
    pub ytd: f64,
    pub flow: f64,
    pub total_score: f64,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Circle {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar: String,
    pub created_at: i64,
    pub persist: bool,
    pub source: String,
    pub tweet_count: u32,
    pub users: Vec<UserOut>,
    pub market: MarketAgg,
}

#[derive(Debug, Deserialize)]
pub struct CreateReq {
    pub username: String,
    #[serde(default = "default_true")]
    pub persist: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct FeedbackReq {
    pub text: String,
    #[serde(default)]
    pub contact: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardItem {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub user_count: usize,
    pub total_score: f64,
    pub created_at: i64,
    pub persist: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub circles: u64,
    pub today: u64,
    pub interactions: u64,
}
