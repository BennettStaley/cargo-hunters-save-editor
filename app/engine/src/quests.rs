//! Mission (quest) deciphering + completion. The save's `AccountQuests` lists
//! in-progress missions by an opaque `DataId` only - no names, no objectives,
//! no rewards. `quests_catalog.json` (extracted from the game's `quests` table +
//! localization string tables) maps each `DataId` to a readable name/category
//! and its rewards (XP + item drops), so the Missions tab can both DECIPHER
//! what's in progress and SKIP a mission while banking its reward.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardItem {
    pub template_id: String,
    pub count: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuestMeta {
    pub name: String,
    pub category: String,
    pub hidden: bool,
    #[serde(default)]
    pub xp: i64,
    #[serde(default)]
    pub items: Vec<RewardItem>,
}

pub type QuestCatalog = HashMap<String, QuestMeta>;

pub fn load_quest_catalog_str(content: &str) -> QuestCatalog {
    serde_json::from_str(content).unwrap_or_default()
}

pub fn load_quest_catalog(path: &Path) -> QuestCatalog {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionView {
    /// The quest INSTANCE id (`ActiveQuests[].Id`) - the handle for skipping.
    pub id: String,
    pub data_id: String,
    pub name: String,
    pub category: String,
    pub hidden: bool,
    pub known: bool,
    pub xp: i64,
    /// Short reward summary for the row, e.g. "6000 XP · 4400x Cash".
    pub reward: String,
    /// True if completing it grants anything (XP or items).
    pub claimable: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MissionsView {
    pub active: Vec<MissionView>,
    pub active_count: i64,
    pub visible_count: i64,
    pub ready_count: i64,
    pub completed_count: i64,
    pub available_count: i64,
}

fn reward_text(m: &QuestMeta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if m.xp > 0 {
        parts.push(format!("{} XP", m.xp));
    }
    for it in &m.items {
        let nm = if it.name.is_empty() { it.template_id.chars().take(6).collect() } else { it.name.clone() };
        parts.push(format!("{}x {}", it.count, nm));
    }
    parts.join(" · ")
}

fn view(id: &str, data_id: &str, cat: &QuestCatalog) -> MissionView {
    match cat.get(data_id) {
        Some(m) => MissionView {
            id: id.to_string(),
            data_id: data_id.to_string(),
            name: m.name.clone(),
            category: m.category.clone(),
            hidden: m.hidden,
            known: true,
            xp: m.xp,
            reward: reward_text(m),
            claimable: m.xp > 0 || !m.items.is_empty(),
        },
        None => MissionView {
            id: id.to_string(),
            data_id: data_id.to_string(),
            name: format!("Unknown mission ({})", &data_id[..8.min(data_id.len())]),
            category: "UNKNOWN".into(),
            hidden: false,
            known: false,
            xp: 0,
            reward: String::new(),
            claimable: false,
        },
    }
}

/// Build the read-only Missions view from a loaded save + the quest catalog.
pub fn build_missions(data: &Value, cat: &QuestCatalog) -> MissionsView {
    let aq = data.get("AccountQuests");
    let arr = |key: &str| -> &[Value] {
        aq.and_then(|q| q.get(key)).and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[])
    };
    let mut active: Vec<MissionView> = arr("ActiveQuests")
        .iter()
        .map(|q| {
            let id = q.get("Id").and_then(|v| v.as_str()).unwrap_or("");
            let data_id = q.get("DataId").and_then(|v| v.as_str()).unwrap_or("");
            view(id, data_id, cat)
        })
        .collect();
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
    fn parses_json_and_builds() {
        let cat = load_quest_catalog_str(
            r#"{"abc":{"name":"Rogue AI","category":"OTHER","hidden":false,"xp":6000,"items":[{"templateId":"cash","count":4400,"name":"Cash"}]},
                "zzz":{"name":"Ping","category":"OTHER/ANALYTICS","hidden":true,"xp":0,"items":[]}}"#,
        );
        assert_eq!(cat.get("abc").unwrap().xp, 6000);
        assert_eq!(cat.get("abc").unwrap().items[0].count, 4400);
        assert!(cat.get("zzz").unwrap().hidden);
        let save = serde_json::json!({
            "AccountQuests": {
                "ActiveQuests": [{"Id":"i1","DataId":"abc"},{"Id":"i2","DataId":"zzz"},{"Id":"i3","DataId":"missing"}],
                "CompletedQuests": [{"QuestDataId":"q"}],
                "ReadyToGiveRewardQuests": [],
                "AvailableQuestsDataId": []
            }
        });
        let mv = build_missions(&save, &cat);
        assert_eq!(mv.active_count, 3);
        assert_eq!(mv.visible_count, 2); // abc (known) + missing (surfaced); zzz hidden
        let rogue = mv.active.iter().find(|m| m.data_id == "abc").unwrap();
        assert!(rogue.claimable && rogue.reward.contains("6000 XP") && rogue.reward.contains("4400x Cash"));
    }
}
