//! Read model over a loaded save `Value`, ported from `save_io.py`.
//!
//! Items live in three parallel arrays addressed by a `source` string. A
//! "container" is any item that already has ≥1 child, plus the well-known roots
//! (backpack / shelter). Grid placement uses `Position{I,J}` + catalog `dims`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

pub const SOURCE_INVENTORY: &str = "inventory";
pub const SOURCE_EQUIPMENT: &str = "equipment";
pub const SOURCE_SHELTER: &str = "shelter";
pub const SOURCES: [&str; 3] = [SOURCE_INVENTORY, SOURCE_EQUIPMENT, SOURCE_SHELTER];
pub const CONDITION_FULL_VALUE: f64 = 4.0;

// ---------- source / item access ----------

/// JSON path (sequence of object keys) to a source's `Items` array.
fn source_path(source: &str) -> Option<&'static [&'static str]> {
    match source {
        SOURCE_INVENTORY => Some(&["InventoryDto", "ItemsContainerDto", "Items"]),
        SOURCE_EQUIPMENT => Some(&["EquipmentDto", "Items"]),
        SOURCE_SHELTER => Some(&["ShelterItemDto", "Container", "Items"]),
        _ => None,
    }
}

/// Borrow a source's items array, if present.
pub fn items_list<'a>(data: &'a Value, source: &str) -> Option<&'a Vec<Value>> {
    let mut cur = data;
    for key in source_path(source)? {
        cur = cur.get(key)?;
    }
    cur.as_array()
}

/// Mutably borrow a source's items array, if present.
pub fn items_list_mut<'a>(data: &'a mut Value, source: &str) -> Option<&'a mut Vec<Value>> {
    let mut cur = data;
    for key in source_path(source)? {
        cur = cur.get_mut(key)?;
    }
    cur.as_array_mut()
}

pub fn str_field<'a>(item: &'a Value, key: &str) -> Option<&'a str> {
    item.get(key).and_then(|v| v.as_str())
}

pub fn item_id(item: &Value) -> Option<&str> {
    str_field(item, "Id")
}
pub fn parent_id(item: &Value) -> Option<&str> {
    str_field(item, "ParentId").filter(|s| !s.is_empty())
}
pub fn template_id(item: &Value) -> &str {
    str_field(item, "TemplateId").unwrap_or("")
}

/// `(I, J)` from `Position`, matching the Python `int(pos.get("I", 0))` rule: a
/// missing/null component (or a missing/null `Position`) defaults to 0. The save
/// stores backpack weapons/containers with partial positions (e.g. `I=null`),
/// and the game places those at 0 on that axis. Negative means an equipped /
/// off-grid slot (callers skip those for grid layout & occupancy).
pub fn pos_ij(item: &Value) -> (i64, i64) {
    let pos = item.get("Position");
    let read = |k: &str| {
        pos.and_then(|p| p.get(k))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };
    (read("I"), read("J"))
}

/// True when an item sits on a grid (both axes non-negative).
pub fn is_on_grid(item: &Value) -> bool {
    let (i, j) = pos_ij(item);
    i >= 0 && j >= 0
}

/// The flat `AdditionalData._data` object, if present.
pub fn additional_data(item: &Value) -> Option<&serde_json::Map<String, Value>> {
    item.get("AdditionalData")?.get("_data")?.as_object()
}

/// `BaseComponent_width/_height` from an item's AdditionalData (container grid /
/// assembled-weapon footprint), if present.
pub fn base_component_wh(item: &Value) -> (Option<i64>, Option<i64>) {
    let Some(ad) = additional_data(item) else {
        return (None, None);
    };
    let w = ad.get("BaseComponent_width").and_then(|v| v.as_i64());
    let h = ad.get("BaseComponent_height").and_then(|v| v.as_i64());
    (w, h)
}

// ---------- backpack / roots ----------

/// The single backpack item Id (the one item parented directly to the inventory
/// container's `OwnerItemId`). Returns None if not exactly one exists.
pub fn backpack_id(data: &Value) -> Option<String> {
    let container = data.get("InventoryDto")?.get("ItemsContainerDto")?;
    let owner = container.get("OwnerItemId")?.as_str()?;
    let items = container.get("Items")?.as_array()?;
    let mut cands = items.iter().filter(|it| parent_id(it) == Some(owner));
    let first = cands.next()?;
    if cands.next().is_some() {
        return None; // expected exactly one
    }
    item_id(first).map(|s| s.to_string())
}

// ---------- containers ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub label: String,
    pub source: String,
    pub owner_item_id: String,
    pub grid_width: Option<i64>,
    pub grid_height: Option<i64>,
    pub template_id: Option<String>,
}

/// Find every place an item could live: the backpack, the shelter root, and any
/// item that already has children. Mirrors `discover_containers`.
pub fn discover_containers(data: &Value, names: &HashMap<String, String>) -> Vec<Container> {
    let mut out: Vec<Container> = Vec::new();

    // Backpack.
    if let Some(bp_id) = backpack_id(data) {
        if let Some(items) = items_list(data, SOURCE_INVENTORY) {
            let bp_item = items.iter().find(|it| item_id(it) == Some(bp_id.as_str()));
            let (gw, gh) = bp_item.map(base_component_wh).unwrap_or((None, None));
            out.push(Container {
                label: "Backpack".into(),
                source: SOURCE_INVENTORY.into(),
                owner_item_id: bp_id,
                grid_width: gw,
                grid_height: gh,
                template_id: bp_item.map(|it| template_id(it).to_string()),
            });
        }
    }

    // Shelter root.
    if let Some(owner) = data
        .get("ShelterItemDto")
        .and_then(|s| s.get("Container"))
        .and_then(|c| c.get("OwnerItemId"))
        .and_then(|v| v.as_str())
    {
        let shelter_item = data.get("ShelterItemDto").and_then(|s| s.get("Item"));
        let (gw, gh) = shelter_item.map(base_component_wh).unwrap_or((None, None));
        out.push(Container {
            label: "Shelter".into(),
            source: SOURCE_SHELTER.into(),
            owner_item_id: owner.to_string(),
            grid_width: gw,
            grid_height: gh,
            template_id: shelter_item
                .and_then(|it| it.get("TemplateId"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        });
    }

    // Equipment & nested containers (anything that already has children).
    for source in SOURCES {
        let Some(items) = items_list(data, source) else {
            continue;
        };
        let mut child_count: HashMap<&str, usize> = HashMap::new();
        for it in items {
            if let Some(pid) = parent_id(it) {
                *child_count.entry(pid).or_insert(0) += 1;
            }
        }
        for it in items {
            let Some(iid) = item_id(it) else { continue };
            if child_count.get(iid).copied().unwrap_or(0) == 0 {
                continue;
            }
            if out
                .iter()
                .any(|c| c.source == source && c.owner_item_id == iid)
            {
                continue; // already added (backpack / shelter root)
            }
            let (gw, gh) = base_component_wh(it);
            let tid = template_id(it);
            let label = names
                .get(tid)
                .cloned()
                .unwrap_or_else(|| tid.chars().take(8).collect());
            let prefix = match source {
                SOURCE_INVENTORY => "Inv",
                SOURCE_EQUIPMENT => "Equip",
                _ => "Shelter",
            };
            out.push(Container {
                label: format!("{prefix}: {label}"),
                source: source.into(),
                owner_item_id: iid.to_string(),
                grid_width: gw,
                grid_height: gh,
                template_id: Some(tid.to_string()),
            });
        }
    }
    out
}

// ---------- occupancy & free-slot search ----------

pub type Dims = HashMap<String, (i64, i64)>;

pub fn item_size(template_id: &str, dims: &Dims) -> (i64, i64) {
    dims.get(template_id).copied().unwrap_or((1, 1))
}

/// Cells occupied by items directly parented to `owner_id` (negative positions
/// are equipped/special and skipped).
pub fn compute_occupancy(items: &[Value], owner_id: &str, dims: &Dims) -> HashSet<(i64, i64)> {
    let mut occ = HashSet::new();
    for it in items {
        if parent_id(it) != Some(owner_id) {
            continue;
        }
        let (i, j) = pos_ij(it);
        if i < 0 || j < 0 {
            continue;
        }
        let (w, h) = item_size(template_id(it), dims);
        for di in 0..w {
            for dj in 0..h {
                occ.insert((i + di, j + dj));
            }
        }
    }
    occ
}

/// First free top-left `(I,J)` for a WxH item, row-major (J outer, I inner).
pub fn find_free_slot(
    occ: &HashSet<(i64, i64)>,
    w: i64,
    h: i64,
    grid_width: i64,
    max_rows: i64,
) -> Option<(i64, i64)> {
    if w > grid_width {
        return None;
    }
    for j in 0..max_rows {
        for i in 0..=(grid_width - w) {
            if (0..w).all(|di| (0..h).all(|dj| !occ.contains(&(i + di, j + dj)))) {
                return Some((i, j));
            }
        }
    }
    None
}

// ---------- catalog (CSV) ----------

/// Parsed catalog: everything the read model needs from `all_items_detailed.csv`
/// in one pass. `names` includes the two built-ins missing from the CSV.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub names: HashMap<String, String>,
    pub dims: Dims,
    pub visuals: HashMap<String, String>,
    /// Known stack caps (`StackCapacity`) for top-off / add, by template.
    pub stack_capacity: HashMap<String, i64>,
}

impl Catalog {
    fn with_builtins() -> Self {
        let mut c = Catalog::default();
        c.names.insert(
            "cb567810-cc82-424f-893f-299c704ffb12".into(),
            "Cash".into(),
        );
        c.names.insert(
            "fd72a971-80d2-4dd3-9d56-22dbbd066642".into(),
            "Lockpick".into(),
        );
        c
    }
}

fn parse_catalog<R: std::io::Read>(mut rdr: csv::Reader<R>) -> Catalog {
    let mut cat = Catalog::with_builtins();
    let Ok(h) = rdr.headers().cloned() else {
        return cat;
    };
    let col = |name: &str| h.iter().position(|c| c == name);
    let (id_idx, name_idx) = (col("ItemID"), col("ItemName"));
    let (w_idx, h_idx, vis_idx, cap_idx) =
        (col("Width"), col("Height"), col("VisualName"), col("StackCapacity"));
    let Some(id_idx) = id_idx else { return cat };
    for rec in rdr.records().flatten() {
        let tid = rec.get(id_idx).unwrap_or("").trim();
        if tid.is_empty() {
            continue;
        }
        if let Some(ni) = name_idx {
            let name = rec.get(ni).unwrap_or("").trim();
            if !name.is_empty() {
                cat.names.entry(tid.to_string()).or_insert_with(|| name.to_string());
            }
        }
        let parse_dim = |idx: Option<usize>| -> i64 {
            idx.and_then(|i| rec.get(i))
                .and_then(|s| s.trim().parse::<i64>().ok())
                .map(|v| v.max(1))
                .unwrap_or(1)
        };
        cat.dims
            .entry(tid.to_string())
            .or_insert_with(|| (parse_dim(w_idx), parse_dim(h_idx)));
        if let Some(vi) = vis_idx {
            let vis = rec.get(vi).unwrap_or("").trim();
            cat.visuals.entry(tid.to_string()).or_insert_with(|| vis.to_string());
        }
        if let Some(ci) = cap_idx {
            if let Some(cap) = rec.get(ci).and_then(|s| s.trim().parse::<i64>().ok()) {
                cat.stack_capacity.entry(tid.to_string()).or_insert(cap);
            }
        }
    }
    cat
}

/// Parse the catalog from a CSV file on disk.
pub fn load_catalog(csv_path: &Path) -> Catalog {
    match csv::Reader::from_path(csv_path) {
        Ok(rdr) => parse_catalog(rdr),
        Err(_) => Catalog::with_builtins(),
    }
}

/// Parse the catalog from an in-memory CSV string (e.g. an `include_str!` blob).
pub fn load_catalog_str(content: &str) -> Catalog {
    parse_catalog(csv::Reader::from_reader(content.as_bytes()))
}

// ---------- equipment slot classification (paperdoll) ----------

/// Where a top-level equipment item belongs on the character screen. The
/// frontend lays these onto the robot silhouette / gear slots.
pub fn classify_slot(visual_name: &str) -> &'static str {
    let parts: Vec<&str> = visual_name.split('/').collect();
    let top = parts.first().copied().unwrap_or("");
    let sub = parts.get(1).copied().unwrap_or("");
    match top {
        "BodyParts" => match sub {
            "Heads" => "bodypart_head",
            "Torsos" => "bodypart_torso",
            "LeftArms" => "bodypart_arm_left",
            "RightArms" => "bodypart_arm_right",
            "LeftLegs" => "bodypart_leg_left",
            "RightLegs" => "bodypart_leg_right",
            _ => "bodypart_other",
        },
        "Outfits" => {
            if sub == "Helmets" || sub == "Hats" {
                "gear_helmet"
            } else if sub == "Backpacks" {
                "gear_backpack"
            } else if sub.contains("Vest") || sub.contains("Armor") {
                "gear_vest"
            } else {
                "gear_outfit"
            }
        }
        "Weapons" => {
            if sub == "Melee" {
                "gear_melee"
            } else {
                "gear_weapon"
            }
        }
        "Tools" => "gear_tool",
        "Items" => {
            if sub == "Droid" {
                "gear_safestash"
            } else if sub == "Weapons" {
                "gear_weapon"
            } else {
                "gear_other"
            }
        }
        "" => "meta", // Phantom / Buff containers carry no VisualName — hidden
        _ => "gear_other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_body_and_gear() {
        assert_eq!(classify_slot("BodyParts/Heads/Head_01.prefab"), "bodypart_head");
        assert_eq!(classify_slot("BodyParts/LeftArms/X.prefab"), "bodypart_arm_left");
        assert_eq!(classify_slot("Outfits/Helmets/He1.prefab"), "gear_helmet");
        assert_eq!(classify_slot("Outfits/AssaultVests/Z.prefab"), "gear_vest");
        assert_eq!(classify_slot("Outfits/Backpacks/Bag.prefab"), "gear_backpack");
        assert_eq!(classify_slot("Weapons/Melee/Baton.prefab"), "gear_melee");
        assert_eq!(classify_slot("Weapons/AssaultRifles/KA74.prefab"), "gear_weapon");
        assert_eq!(classify_slot("Items/Droid/SafeBox.prefab"), "gear_safestash");
        assert_eq!(classify_slot(""), "meta");
    }

    #[test]
    fn find_slot_row_major() {
        let occ = HashSet::new();
        assert_eq!(find_free_slot(&occ, 2, 1, 8, 256), Some((0, 0)));
        let mut occ = HashSet::new();
        occ.insert((0, 0));
        occ.insert((1, 0));
        assert_eq!(find_free_slot(&occ, 1, 1, 8, 256), Some((2, 0)));
    }
}
