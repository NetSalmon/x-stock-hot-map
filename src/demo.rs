// Copyright (C) 2026 NetSalmon
// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::OnceLock;

use crate::model::Circle;

static DEMO_CACHE: OnceLock<Circle> = OnceLock::new();

/// The bundled demo circle (Elon Musk's interaction network).
pub fn demo_circle(now: i64) -> Circle {
    let template = DEMO_CACHE.get_or_init(|| {
        serde_json::from_str(include_str!("demo_circle.json"))
            .expect("valid embedded demo circle JSON")
    });
    let mut circle = template.clone();
    circle.created_at = now;
    circle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_elonmusk_demo() {
        let circle = demo_circle(1_700_000_000);
        assert_eq!(circle.id, "demo");
        assert_eq!(circle.username, "elonmusk");
        assert_eq!(circle.display_name, "Elon Musk");
        assert_eq!(circle.source, "demo");
        assert!(!circle.users.is_empty());
    }
}
