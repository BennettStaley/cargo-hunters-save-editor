//! The read-only view of a save that the frontend renders: items (with catalog
//! dims, assembled-weapon footprints, condition, slot classification),
//! containers, and the account/skills. Pure data - icon resolution and grid
//! layout happen in the UI.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

use crate::model;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub id: String,
    pub parent_id: Option<String>,
    pub template_id: String,
    pub name: String,
    pub visual_name: String,
    /// Grid position; `None`/negative means equipped/special (not on a grid).
    pub i: Option<i64>,
    pub j: Option<i64>,
    /// Catalog footprint.
    pub base_w: i64,
    pub base_h: i64,
    /// Assembled-weapon part-grid extents (`BaseComponent_width/_height`), if any.
    pub asm_w: Option<i64>,
    pub asm_h: Option<i64>,
    /// TRUE grid footprint to render/reserve (assembled weapons recovered from
    /// packing; normal items = catalog). Matches the engine's occupancy.
    pub grid_w: i64,
    pub grid_h: i64,
    pub qty: Option<i64>,
    pub condition_d: Option<f64>,
    pub condition_mt: Option<f64>,
    pub durability: Option<f64>,
    pub durability_md: Option<f64>,
    pub is_container: bool,
    /// Slot classification for top-level equipment (paperdoll); empty otherwise.
    pub slot: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    pub id: i64,
    pub level: Option<i64>,
    pub next_goal: Option<i64>,
    /// Display name from the game's skill table (falls back to "Skill #id").
    pub name: String,
    /// `IconSkill_*` sprite stem, if known.
    pub icon: Option<String>,
    /// Max level this skill can reach.
    pub max_level: Option<i64>,
    /// Deprecated handling skill the game keeps for back-compat.
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub nickname: Option<String>,
    pub level: Option<i64>,
    pub xp: Option<i64>,
    pub next_goal: Option<i64>,
    pub skill_points: Option<i64>,
    pub skills: Vec<SkillView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub save_path: String,
    pub backpack_id: Option<String>,
    pub containers: Vec<model::Container>,
    pub inventory: Vec<ItemView>,
    pub equipment: Vec<ItemView>,
    pub shelter: Vec<ItemView>,
    pub account: Account,
    /// Set by the command layer: are there unsaved staged edits?
    pub dirty: bool,
}

/// One browsable catalog item (for the Add-items screen).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub template_id: String,
    pub name: String,
    pub visual_name: String,
    pub w: i64,
    pub h: i64,
    pub stack_capacity: Option<i64>,
}

/// Every known catalog item, sorted by name (for the Add-items browser).
pub fn catalog_entries(cat: &model::Catalog) -> Vec<CatalogEntry> {
    let mut out: Vec<CatalogEntry> = cat
        .names
        .iter()
        .map(|(tid, name)| {
            let (w, h) = cat.dims.get(tid).copied().unwrap_or((1, 1));
            CatalogEntry {
                template_id: tid.clone(),
                name: name.clone(),
                visual_name: cat.visuals.get(tid).cloned().unwrap_or_default(),
                w,
                h,
                stack_capacity: cat.stack_capacity.get(tid).copied(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn num(ad: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<f64> {
    ad?.get(key)?.as_f64()
}

fn build_items(
    items: &[Value],
    names: &HashMap<String, String>,
    dims: &model::Dims,
    visuals: &HashMap<String, String>,
) -> Vec<ItemView> {
    // Which ids act as containers within this source.
    let mut has_children: HashSet<&str> = HashSet::new();
    for it in items {
        if let Some(pid) = model::parent_id(it) {
            has_children.insert(pid);
        }
    }

    // True grid footprints per container (so display == occupancy). Computed
    // once per distinct parent that holds children.
    let mut footprints: HashMap<String, (i64, i64)> = HashMap::new();
    let parents: HashSet<String> = items
        .iter()
        .filter_map(|it| model::parent_id(it).map(|s| s.to_string()))
        .collect();
    for owner in &parents {
        footprints.extend(model::container_footprints(items, owner, dims));
    }

    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let Some(id) = model::item_id(it) else { continue };
        let tid = model::template_id(it);
        let ad = model::additional_data(it);
        let (asm_w, asm_h) = model::base_component_wh(it);
        let (base_w, base_h) = model::item_size(tid, dims);
        // Defaulted (missing axis -> 0) so backpack weapons with partial
        // positions still land on the grid; negatives stay off-grid.
        let (pi, pj) = model::pos_ij(it);
        let (i, j) = (Some(pi), Some(pj));
        let visual = visuals.get(tid).cloned().unwrap_or_default();
        // Slot only meaningful for top-level (un-parented) equipment items.
        let slot = if model::parent_id(it).is_none() {
            model::classify_slot(&visual).to_string()
        } else {
            String::new()
        };
        let (grid_w, grid_h) = footprints.get(id).copied().unwrap_or((base_w, base_h));
        out.push(ItemView {
            id: id.to_string(),
            parent_id: model::parent_id(it).map(|s| s.to_string()),
            template_id: tid.to_string(),
            name: names.get(tid).cloned().unwrap_or_else(|| tid.chars().take(8).collect()),
            visual_name: visual,
            i,
            j,
            base_w,
            base_h,
            asm_w,
            asm_h,
            grid_w,
            grid_h,
            qty: ad.and_then(|m| m.get("StackableComponent_quantity")).and_then(|v| v.as_i64()),
            condition_d: num(ad, "Condition_d"),
            condition_mt: num(ad, "Condition_mt"),
            durability: num(ad, "DurabilityComponent_durability"),
            durability_md: num(ad, "DurabilityComponent_md"),
            is_container: has_children.contains(id),
            slot,
        });
    }
    out
}

fn build_account(data: &Value) -> Account {
    let mut acc = Account::default();
    let Some(account) = data.get("AccountDto") else {
        return acc;
    };
    acc.nickname = account.get("Nickname").and_then(|v| v.as_str()).map(|s| s.to_string());
    if let Some(exp) = account.get("ExperienceDto") {
        acc.level = exp.get("Level").and_then(|v| v.as_i64());
        acc.xp = exp.get("ExperiencePoints").and_then(|v| v.as_i64());
        acc.next_goal = exp.get("NextLevelExperienceGoal").and_then(|v| v.as_i64());
    }
    if let Some(sk) = account.get("SkillsDto") {
        acc.skill_points = sk.get("SkillPointsCount").and_then(|v| v.as_i64());
        if let Some(list) = sk.get("Skills").and_then(|v| v.as_array()) {
            for s in list {
                if let Some(id) = s.get("Id").and_then(|v| v.as_i64()) {
                    let meta = crate::skills::skill_meta(id);
                    acc.skills.push(SkillView {
                        id,
                        level: s.get("Level").and_then(|v| v.as_i64()),
                        next_goal: s.get("NextLevelExperienceGoal").and_then(|v| v.as_i64()),
                        name: meta.as_ref().map(|m| m.name.to_string()).unwrap_or_else(|| format!("Skill #{id}")),
                        icon: meta.as_ref().map(|m| m.icon.to_string()),
                        max_level: meta.as_ref().map(|m| m.max_level),
                        disabled: meta.as_ref().map(|m| m.disabled).unwrap_or(false),
                    });
                }
            }
            // Active skills first, in the game's display order; then disabled.
            acc.skills.sort_by_key(|s| {
                let order = crate::skills::skill_meta(s.id).map(|m| m.order).unwrap_or(9999);
                (s.disabled, order, s.id)
            });
        }
    }
    acc
}

/// Build the full frontend snapshot from a loaded save.
pub fn build_snapshot(data: &Value, save_path: &str, cat: &model::Catalog) -> Snapshot {
    let empty: Vec<Value> = Vec::new();
    let inv = model::items_list(data, model::SOURCE_INVENTORY).unwrap_or(&empty);
    let eq = model::items_list(data, model::SOURCE_EQUIPMENT).unwrap_or(&empty);
    let sh = model::items_list(data, model::SOURCE_SHELTER).unwrap_or(&empty);
    Snapshot {
        save_path: save_path.to_string(),
        backpack_id: model::backpack_id(data),
        containers: model::discover_containers(data, &cat.names),
        inventory: build_items(inv, &cat.names, &cat.dims, &cat.visuals),
        equipment: build_items(eq, &cat.names, &cat.dims, &cat.visuals),
        shelter: build_items(sh, &cat.names, &cat.dims, &cat.visuals),
        account: build_account(data),
        dirty: false,
    }
}
