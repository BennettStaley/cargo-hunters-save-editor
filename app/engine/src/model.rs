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

/// `BaseComponent_rotated` from the save: the item is stood up (W/H swapped on
/// the grid), e.g. a 2x1 stock placed as 1x2 in a 1-wide column.
pub fn base_component_rotated(item: &Value) -> bool {
    additional_data(item)
        .and_then(|ad| ad.get("BaseComponent_rotated"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
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

// ---------- backpack / pages / roots ----------

/// The inventory "page" container ids - the items parented directly to the
/// inventory container's `OwnerItemId`. The game splits the stash into pages
/// (`Inventory_1`, `Inventory_2`, ...); each page is its own grid. Ordered by
/// the page's `Position.I` (page index), then by save order. A normal save has
/// one page; this returns it as a single-element list.
pub fn inventory_page_ids(data: &Value) -> Vec<String> {
    let Some(container) = data.get("InventoryDto").and_then(|d| d.get("ItemsContainerDto")) else {
        return Vec::new();
    };
    let Some(owner) = container.get("OwnerItemId").and_then(|v| v.as_str()) else {
        return Vec::new();
    };
    let Some(items) = container.get("Items").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut pages: Vec<(i64, usize, String)> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| parent_id(it) == Some(owner))
        .filter_map(|(ord, it)| item_id(it).map(|id| (pos_ij(it).0, ord, id.to_string())))
        .collect();
    pages.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    pages.into_iter().map(|(_, _, id)| id).collect()
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

    // Inventory pages (the stash is split into one grid per page).
    let page_ids = inventory_page_ids(data);
    let multi_page = page_ids.len() > 1;
    if let Some(items) = items_list(data, SOURCE_INVENTORY) {
        for (idx, pid) in page_ids.iter().enumerate() {
            let item = items.iter().find(|it| item_id(it) == Some(pid.as_str()));
            let (gw, gh) = item.map(base_component_wh).unwrap_or((None, None));
            let label = if multi_page {
                format!("Vault - Page {}", idx + 1)
            } else {
                "Vault (backpack)".into()
            };
            out.push(Container {
                label,
                source: SOURCE_INVENTORY.into(),
                owner_item_id: pid.clone(),
                grid_width: gw,
                grid_height: gh,
                template_id: item.map(|it| template_id(it).to_string()),
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

/// The footprint an item ACTUALLY occupies on a grid. Assembled weapons carry a
/// `BaseComponent_width/_height` part-grid that is larger than their catalog
/// base size, so the true footprint is the max of the two. Using catalog dims
/// alone under-reserves weapons and lets new items land on top of them.
pub fn occupied_size(item: &Value, dims: &Dims) -> (i64, i64) {
    let (cw, ch) = item_size(template_id(item), dims);
    let (bw, bh) = base_component_wh(item);
    (cw.max(bw.unwrap_or(0)).max(1), ch.max(bh.unwrap_or(0)).max(1))
}

/// True grid footprint of each child of `owner_id`, keyed by item Id.
///
/// Assembled weapons render larger than their catalog/part-grid size; the game
/// encodes the real size by packing items tightly, so we recover each weapon's
/// footprint as the gap to the next item in its row (width) / column (height),
/// and - when it's the last item in a row - out to the grid edge (long guns
/// like the Ramon fill the row). Normal (non-assembled) items keep catalog dims.
/// Display and occupancy both use this so what you see is exactly what's reserved.
pub fn container_footprints(items: &[Value], owner_id: &str, cat: &Catalog) -> HashMap<String, (i64, i64)> {
    struct P {
        id: String,
        i: i64,
        j: i64,
        cw: i64, // base Size (game item_templates)
        ch: i64,
        mw: i64, // MaxSize (fully-kitted footprint)
        mh: i64,
        resizable: bool,
        rotated: bool,
    }
    let mut ps: Vec<P> = Vec::new();
    for it in items {
        if parent_id(it) != Some(owner_id) {
            continue;
        }
        let (i, j) = pos_ij(it);
        if i < 0 || j < 0 {
            continue;
        }
        let tid = template_id(it);
        let (cw, ch) = item_size(tid, &cat.dims);
        let (mw, mh) = cat.max_dims.get(tid).copied().unwrap_or((cw, ch));
        ps.push(P {
            id: item_id(it).unwrap_or("").to_string(),
            i,
            j,
            cw,
            ch,
            mw,
            mh,
            resizable: cat.resizable.contains(tid),
            rotated: base_component_rotated(it),
        });
    }
    // Base occupancy: every item's MINIMUM footprint (catalog Size, swapped if
    // rotated), keyed cell -> owner id. Non-resizable items occupy exactly this;
    // resizable weapons occupy at least this. Used to find where a growing
    // weapon is blocked by another item (incl. rotated multi-cell parts).
    let mut occ: HashMap<(i64, i64), &str> = HashMap::new();
    for p in &ps {
        let (bw, bh) = if p.rotated { (p.ch, p.cw) } else { (p.cw, p.ch) };
        for di in 0..bw {
            for dj in 0..bh {
                occ.insert((p.i + di, p.j + dj), p.id.as_str());
            }
        }
    }

    let mut out = HashMap::new();
    for p in &ps {
        // Non-resizable: exactly catalog Size (rotated). Resizable weapons grow
        // from Size toward MaxSize until blocked by another item's cells (the
        // gap-free packing pins the real assembled size; MaxSize caps it).
        let (w, h) = if p.resizable {
            let (maxw, maxh) = if p.rotated { (p.mh, p.mw) } else { (p.mw, p.mh) };
            let (basew, baseh) = if p.rotated { (p.ch, p.cw) } else { (p.cw, p.ch) };
            let mut w = maxw;
            for c in (p.i + 1)..=(p.i + maxw) {
                if occ.get(&(c, p.j)).is_some_and(|o| *o != p.id.as_str()) {
                    w = c - p.i;
                    break;
                }
            }
            let mut h = maxh;
            for r in (p.j + 1)..=(p.j + maxh) {
                if occ.get(&(p.i, r)).is_some_and(|o| *o != p.id.as_str()) {
                    h = r - p.j;
                    break;
                }
            }
            (w.max(basew), h.max(baseh))
        } else if p.rotated {
            (p.ch, p.cw)
        } else {
            (p.cw, p.ch)
        };
        out.insert(p.id.clone(), (w.max(1), h.max(1)));
    }
    out
}

/// The declared `BaseComponent_width` of the container item itself, if any.
pub fn declared_width(items: &[Value], owner_id: &str) -> Option<i64> {
    items
        .iter()
        .find(|it| item_id(it) == Some(owner_id))
        .and_then(|owner| base_component_wh(owner).0)
}

/// Cells occupied by items directly parented to `owner_id`, using each item's
/// true footprint (`container_footprints`) so placement never overlaps a weapon.
pub fn compute_occupancy(items: &[Value], owner_id: &str, cat: &Catalog) -> HashSet<(i64, i64)> {
    let fps = container_footprints(items, owner_id, cat);
    let mut occ = HashSet::new();
    for it in items {
        if parent_id(it) != Some(owner_id) {
            continue;
        }
        let (i, j) = pos_ij(it);
        if i < 0 || j < 0 {
            continue;
        }
        let (w, h) = fps
            .get(item_id(it).unwrap_or(""))
            .copied()
            .unwrap_or_else(|| occupied_size(it, &cat.dims));
        for di in 0..w {
            for dj in 0..h {
                occ.insert((i + di, j + dj));
            }
        }
    }
    occ
}

/// The grid width to use when placing into a container: its declared
/// `BaseComponent_width` if known, else inferred from the widest occupied cell
/// of its current contents (so the backpack resolves to its real 8, not a
/// default 10), else a sane fallback.
pub fn effective_grid_width(
    items: &[Value],
    owner_id: &str,
    declared: Option<i64>,
    dims: &Dims,
) -> i64 {
    if let Some(w) = declared {
        if w > 0 {
            return w;
        }
    }
    let mut max_w = 0;
    for it in items {
        if parent_id(it) != Some(owner_id) {
            continue;
        }
        let (i, j) = pos_ij(it);
        if i < 0 || j < 0 {
            continue;
        }
        let (w, _) = occupied_size(it, dims);
        max_w = max_w.max(i + w);
    }
    if max_w > 0 {
        max_w
    } else {
        8
    }
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
    /// `MaxSize` (fully-kitted footprint) for resizable weapons, by template.
    pub max_dims: HashMap<String, (i64, i64)>,
    /// Templates whose footprint grows with attachments (`IsResizable`).
    pub resizable: HashSet<String>,
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
    let (mw_idx, mh_idx, rz_idx) = (col("MaxWidth"), col("MaxHeight"), col("Resizable"));
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
        if mw_idx.is_some() || mh_idx.is_some() {
            let (mw, mh) = (parse_dim(mw_idx), parse_dim(mh_idx));
            cat.max_dims.entry(tid.to_string()).or_insert((mw, mh));
        }
        if let Some(ri) = rz_idx {
            if rec.get(ri).map(|s| s.trim()) == Some("1") {
                cat.resizable.insert(tid.to_string());
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
        "" => "meta", // Phantom / Buff containers carry no VisualName - hidden
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
