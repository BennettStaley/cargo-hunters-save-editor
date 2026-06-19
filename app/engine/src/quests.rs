//! Mission (quest) deciphering. The save's `AccountQuests` lists in-progress
//! missions by an opaque `DataId` only - no names, no objectives. This module
//! maps each `DataId` to a human-readable name/category via `quests_catalog.csv`
//! (extracted from the game's `quests` table + localization string tables) and
//! builds the read-only view the Missions tab renders.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct QuestMeta {
    pub name: String,
    pub category: String,
    /// Internal/telemetry quests the game hides from the player's order list.
    pub hidden: bool,
}

pub type QuestCatalog = HashMap<String, QuestMeta>;

fn parse<R: std::io::Read>(mut rdr: csv::Reader<R>) -> QuestCatalog {
    let mut out = QuestCatalog::new();
    let Ok(h) = rdr.headers().cloned() else {
        return out;
    };
    let col = |name: &str| h.iter().position(|c| c == name);
    let (Some(id_idx), Some(name_idx)) = (col("DataId"), col("Name")) else {
        return out;
    };
    let (cat_idx, hid_idx) = (col("Category"), col("Hidden"));
    for rec in rdr.records().flatten() {
        let id = rec.get(id_idx).unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        let name = rec.get(name_idx).unwrap_or("").trim();
        let category = cat_idx.and_then(|i| rec.get(i)).unwrap_or("").trim().to_string();
        let hidden = hid_idx.and_then(|i| rec.get(i)).map(|s| s.trim() == "1").unwrap_or(false);
        out.insert(
            id.to_string(),
            QuestMeta { name: name.to_string(), category, hidden },
        );
    }
    out
}

pub fn load_quest_catalog_str(content: &str) -> QuestCatalog {
    parse(csv::Reader::from_reader(content.as_bytes()))
}

pub fn load_quest_catalog(path: &Path) -> QuestCatalog {
    match csv::Reader::from_path(path) {
        Ok(r) => parse(r),
        Err(_) => QuestCatalog::new(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionView {
    pub data_id: String,
    pub name: String,
    pub category: String,
    pub hidden: bool,
    /// True once we have a real catalog name (vs. an unknown `DataId`).
    pub known: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MissionsView {
    /// In-progress missions (the save's `ActiveQuests`).
    pub active: Vec<MissionView>,
    pub active_count: i64,
    /// Active missions the player can actually see (not hidden telemetry).
    pub visible_count: i64,
    pub ready_count: i64,
    pub completed_count: i64,
    pub available_count: i64,
}

fn view(data_id: &str, cat: &QuestCatalog) -> MissionView {
    match cat.get(data_id) {
        Some(m) => MissionView {
            data_id: data_id.to_string(),
            name: m.name.clone(),
            category: m.category.clone(),
            hidden: m.hidden,
            known: true,
        },
        None => MissionView {
            data_id: data_id.to_string(),
            name: format!("Unknown mission ({})", &data_id[..8.min(data_id.len())]),
            category: "UNKNOWN".into(),
            hidden: false,
            known: false,
        },
    }
}

/// Build the Missions view from a loaded save + the quest catalog.
pub fn build_missions(data: &Value, cat: &QuestCatalog) -> MissionsView {
    let aq = data.get("AccountQuests");
    let arr = |key: &str| -> &[Value] {
        aq.and_then(|q| q.get(key)).and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[])
    };
    let active_raw = arr("ActiveQuests");
    let mut active: Vec<MissionView> = active_raw
        .iter()
        .filter_map(|q| q.get("DataId").and_then(|v| v.as_str()))
        .map(|id| view(id, cat))
        .collect();
    // Visible (real) missions first, alphabetical; hidden telemetry after.
    active.sort_by(|a, b| {
        a.hidden
            .cmp(&b.hidden)
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let visible_count = active.iter().filter(|m| !m.hidden).count() as i64;
    MissionsView {
        active_count: active.len() as i64,
        visible_count,
        ready_count: arr("ReadyToGiveRewardQuests").len() as i64,
        completed_count: arr("CompletedQuests").len() as i64,
        available_count: arr("AvailableQuestsDataId").len() as i64,
        active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_builds() {
        let cat = load_quest_catalog_str(
            "DataId,Name,Category,Alias,Hidden\nabc,Mousetrap: Part 1,MISSIONS,x,0\nzzz,Ping,OTHER/ANALYTICS,y,1\n",
        );
        assert_eq!(cat.get("abc").unwrap().name, "Mousetrap: Part 1");
        assert!(cat.get("zzz").unwrap().hidden);
        let save = serde_json::json!({
            "AccountQuests": {
                "ActiveQuests": [{"DataId":"abc"},{"DataId":"zzz"},{"DataId":"missing"}],
                "CompletedQuests": [{"QuestDataId":"q"}],
                "ReadyToGiveRewardQuests": [],
                "AvailableQuestsDataId": []
            }
        });
        let mv = build_missions(&save, &cat);
        assert_eq!(mv.active_count, 3);
        // "abc" (known, non-hidden) + "missing" (unknown, surfaced) = 2; "zzz" hidden.
        assert_eq!(mv.visible_count, 2);
        assert_eq!(mv.completed_count, 1);
        // unknown DataId is surfaced, not dropped
        assert!(mv.active.iter().any(|m| !m.known && m.data_id == "missing"));
    }
}
