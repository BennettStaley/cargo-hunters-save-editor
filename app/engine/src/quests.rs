//! Mission (quest) deciphering. The save's `AccountQuests` lists in-progress
//! missions by an opaque `DataId` only - no names, no objective progress.
//! `quests_catalog.json` (extracted from the game's `quests` table +
//! localization string tables) maps each `DataId` to a readable name, its
//! reward, and its OBJECTIVES (what the mission requires). The Missions tab is
//! read-only: it shows what each mission needs (and what you currently hold) so
//! you can add the materials and complete it in-game - it never edits quest
//! state itself.

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
#[serde(rename_all = "camelCase")]
pub struct ReqItem {
    pub template_id: String,
    #[serde(default)]
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Objective {
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub items: Vec<ReqItem>,
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
    #[serde(default)]
    pub objectives: Vec<Objective>,
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
pub struct ReqItemView {
    pub template_id: String,
    pub name: String,
    /// How many the objective needs.
    pub need: i64,
    /// How many the player currently holds anywhere in the inventory source.
    pub have: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveView {
    pub desc: String,
    pub items: Vec<ReqItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionView {
    pub id: String,
    pub data_id: String,
    pub name: String,
    pub category: String,
    pub hidden: bool,
    pub known: bool,
    pub xp: i64,
    /// Short reward summary, e.g. "6000 XP · 4400x Cash".
    pub reward: String,
    pub objectives: Vec<ObjectiveView>,
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

/// Count how many of each template the player holds in the inventory source.
fn held_counts(data: &Value) -> HashMap<String, i64> {
    let mut held: HashMap<String, i64> = HashMap::new();
    if let Some(items) = data
        .get("InventoryDto")
        .and_then(|d| d.get("ItemsContainerDto"))
        .and_then(|c| c.get("Items"))
        .and_then(|v| v.as_array())
    {
        for it in items {
            if let Some(t) = it.get("TemplateId").and_then(|v| v.as_str()) {
                *held.entry(t.to_string()).or_insert(0) += 1;
            }
        }
    }
    held
}

fn view(id: &str, data_id: &str, cat: &QuestCatalog, held: &HashMap<String, i64>) -> MissionView {
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
            objectives: m
                .objectives
                .iter()
                .map(|o| ObjectiveView {
                    desc: o.desc.clone(),
                    items: o
                        .items
                        .iter()
                        .map(|i| ReqItemView {
                            template_id: i.template_id.clone(),
                            name: i.name.clone(),
                            need: i.count,
                            have: held.get(&i.template_id).copied().unwrap_or(0),
                        })
                        .collect(),
                })
                .collect(),
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
            objectives: Vec::new(),
        },
    }
}

/// Build the read-only Missions view from a loaded save + the quest catalog.
pub fn build_missions(data: &Value, cat: &QuestCatalog) -> MissionsView {
    let held = held_counts(data);
    let aq = data.get("AccountQuests");
    let arr = |key: &str| -> &[Value] {
        aq.and_then(|q| q.get(key)).and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[])
    };
    let mut active: Vec<MissionView> = arr("ActiveQuests")
        .iter()
        .map(|q| {
            let id = q.get("Id").and_then(|v| v.as_str()).unwrap_or("");
            let data_id = q.get("DataId").and_then(|v| v.as_str()).unwrap_or("");
            view(id, data_id, cat, &held)
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
    fn objectives_and_have_counts() {
        let cat = load_quest_catalog_str(
            r#"{"abc":{"name":"Rogue AI","category":"OTHER","hidden":false,"xp":6000,
                "items":[{"templateId":"cash","count":4400,"name":"Cash"}],
                "objectives":[{"desc":"Hand in the chip","items":[{"templateId":"chip","name":"OSMA5 Chip","count":1}]}]}}"#,
        );
        let save = serde_json::json!({
            "InventoryDto": { "ItemsContainerDto": { "Items": [
                {"Id":"x","TemplateId":"chip"}, {"Id":"y","TemplateId":"chip"}
            ]}},
            "AccountQuests": { "ActiveQuests": [{"Id":"i1","DataId":"abc"}],
                "CompletedQuests": [], "ReadyToGiveRewardQuests": [], "AvailableQuestsDataId": [] }
        });
        let mv = build_missions(&save, &cat);
        let m = &mv.active[0];
        assert_eq!(m.reward, "6000 XP · 4400x Cash");
        let req = &m.objectives[0].items[0];
        assert_eq!(req.name, "OSMA5 Chip");
        assert_eq!(req.need, 1);
        assert_eq!(req.have, 2); // two "chip" items held
    }
}
