//! In-place mutations over a loaded save `Value`, ported from `save_io.py` /
//! `add_item.py`. Every op mutates `data` and is proven against the Python
//! engine by the oracle (semantic equality after the same operation).
//!
//! Number construction note: under `arbitrary_precision`, `Number::from_f64`
//! formats via ryu, so `4.0_f64 -> "4.0"` (matching Python's `float` repr) and
//! integers via `i64 -> "4"`. Untouched numbers keep their original literal.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Number, Value};
use uuid::Uuid;

use crate::model;

pub const CONDITION_FULL_VALUE: f64 = 4.0;

fn jf(x: f64) -> Value {
    Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null)
}
fn ji(x: i64) -> Value {
    Value::Number(Number::from(x))
}

/// Find an item by Id within a source; returns a mutable reference.
fn find_item_mut<'a>(data: &'a mut Value, source: &str, item_id: &str) -> Option<&'a mut Value> {
    let items = model::items_list_mut(data, source)?;
    items
        .iter_mut()
        .find(|it| it.get("Id").and_then(|v| v.as_str()) == Some(item_id))
}

/// Borrow (creating if absent) an item's `AdditionalData._data` object. A
/// present-but-null/non-object `AdditionalData` or `_data` (only possible in a
/// hand-edited/corrupt save) is coerced to a fresh object rather than panicking
/// - `entry().or_insert_with` only fires when the key is ABSENT, so the explicit
/// coercion is what makes this safe on malformed input.
fn ad_data_mut(item: &mut Value) -> &mut Map<String, Value> {
    let obj = item.as_object_mut().expect("save item is a JSON object");
    let ad = obj
        .entry("AdditionalData")
        .or_insert_with(|| Value::Object(Map::new()));
    if !ad.is_object() {
        *ad = Value::Object(Map::new());
    }
    let data = ad
        .as_object_mut()
        .expect("AdditionalData coerced to object")
        .entry("_data")
        .or_insert_with(|| Value::Object(Map::new()));
    if !data.is_object() {
        *data = Value::Object(Map::new());
    }
    data.as_object_mut().expect("_data coerced to object")
}

/// Set an item's stack quantity / condition / durability (the APPLY action).
pub fn set_item_fields(
    data: &mut Value,
    source: &str,
    item_id: &str,
    quantity: Option<i64>,
    condition: Option<f64>,
    durability: Option<f64>,
) -> Result<(), String> {
    let item = find_item_mut(data, source, item_id)
        .ok_or_else(|| format!("item {item_id} not found in {source}"))?;
    let ad = ad_data_mut(item);
    if let Some(q) = quantity {
        ad.insert("StackableComponent_quantity".into(), ji(q));
    }
    if let Some(c) = condition {
        ad.insert("Condition_d".into(), jf(c));
        ad.insert("Condition_mt".into(), jf(c));
    }
    if let Some(d) = durability {
        ad.insert("DurabilityComponent_durability".into(), jf(d));
        ad.insert("DurabilityComponent_md".into(), jf(d));
    }
    Ok(())
}

/// Reposition an item within its container (drag-to-move on the grid).
pub fn move_item_position(
    data: &mut Value,
    source: &str,
    item_id: &str,
    i: i64,
    j: i64,
) -> Result<(), String> {
    let item = find_item_mut(data, source, item_id)
        .ok_or_else(|| format!("item {item_id} not found in {source}"))?;
    let obj = item.as_object_mut().ok_or("item is not a JSON object")?;
    let mut pos = Map::new();
    pos.insert("I".into(), ji(i));
    pos.insert("J".into(), ji(j));
    obj.insert("Position".into(), Value::Object(pos));
    Ok(())
}

/// Remove items by Id from every source, recursively removing descendants.
/// Returns the number removed.
pub fn remove_items_by_ids(data: &mut Value, ids: &HashSet<String>) -> usize {
    let mut total = 0;
    for source in model::SOURCES {
        let Some(items) = model::items_list_mut(data, source) else {
            continue;
        };
        // Expand to all descendants of the requested ids within this source.
        let mut to_remove: HashSet<String> = ids.clone();
        loop {
            let mut changed = false;
            for it in items.iter() {
                let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("");
                let pid = it.get("ParentId").and_then(|v| v.as_str()).unwrap_or("");
                if !pid.is_empty() && to_remove.contains(pid) && !to_remove.contains(id) {
                    to_remove.insert(id.to_string());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let before = items.len();
        items.retain(|it| {
            let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("");
            !to_remove.contains(id)
        });
        total += before - items.len();
    }
    total
}

// ---------- repair / refill / top-off ----------

/// Known full-use counts for items whose `DurabilityComponent_durability` is a
/// use count without a serialized `_md` (ported from `USE_COUNT_TEMPLATE_MAX`).
fn use_count_max(template_id: &str) -> Option<f64> {
    Some(match template_id {
        "9d991ab8-3a58-4751-a758-d86279872dd1" => 3.0,
        "755fa97a-40c0-4e85-ade5-c58bc63db4dd" => 3.0,
        "0a92b724-46a9-45b9-9837-67f3b2aecf1d" => 3.0,
        "76ad31a4-2fe1-4ca4-a2b6-a5133e776ff1" => 3.0,
        "2613c37e-2678-4627-a87d-37dc46274d8a" => 100.0,
        "6d249ffa-bd3b-43c2-89fa-4992be7af2a9" => 200.0,
        "b26c003f-496e-41b3-bc6f-70beaa76ac0e" => 500.0,
        "e6ec3bc6-a38e-4f1c-a04d-bf900bf082b3" => 1000.0,
        "fd065383-2b84-41e8-80fd-041bf8d19ab6" => 1600.0,
        "b1c818fa-0ae5-415c-b407-a9c1a92feb14" => 3.0,
        "d668d0fc-df69-4eda-b7c2-50c6ed818488" => 10.0,
        "43dfb4d7-5cf6-4cb0-908f-7111f9a33e32" => 5.0,
        "343734f4-6c77-4370-a73c-d9ccdc101a15" => 15.0,
        _ => return None,
    })
}

/// Explicit stack maxima (ported from `STACK_COUNT_TEMPLATE_MAX`).
fn stack_count_max_explicit(template_id: &str) -> Option<i64> {
    Some(match template_id {
        "cb567810-cc82-424f-893f-299c704ffb12" => 10_000,
        "fd72a971-80d2-4dd3-9d56-22dbbd066642" => 5,
        "98e1e51b-4f8b-4512-bd34-2a37a0eb2930"
        | "394783c8-3fa6-4573-a154-fa52921eeb15"
        | "4ec3fa7f-f8a9-4fce-bcdd-efda2dbf0826"
        | "0e9060f6-f0d4-4f62-9457-c9165a959b4d"
        | "cc5a5fde-6c82-45af-babf-3d6875a26911"
        | "e9fd9b62-e02b-435a-88fc-87dd5597a00a"
        | "e3e576c5-7cf4-4e9b-8283-2fd5eb4676d2"
        | "bb5ca07d-ad87-45c0-96da-dd153a03bcf7"
        | "d08d0179-5c5f-4ae4-bf1f-8032d52f3498"
        | "22d7f633-57d4-4906-b0b1-ea0299203826"
        | "3222f212-6e49-4391-8eff-a929474e1e4c"
        | "82700397-d829-4dbd-8e84-38120f0d0ba2"
        | "9f5c76b9-f09e-4bee-ada1-2ed0afe7ce17"
        | "bea27756-b4e4-4b61-b572-c8f81e3f7e8b"
        | "cb6d4579-12b9-478c-94e4-579f01a45a83"
        | "36c7a7d2-7eca-400f-a28e-68613070505c"
        | "a7721ae4-5bb3-4c09-8605-e18272b59ac6"
        | "deeb8cc6-ef24-4194-8139-ffe155d1b87f"
        | "2da75073-af27-4924-a4c1-d27d2b834df7"
        | "32720498-5feb-4bee-9186-90dd08311206"
        | "657ea17a-0c61-4ef3-b993-639e0791ab2d"
        | "6ea5a413-f9d8-421a-8465-d3c8f8802c72"
        | "80c9682f-bf58-4176-a457-050e581c80a4"
        | "c06bca0a-f8f3-4af8-a9e1-26c15e62c443" => 60,
        _ => return None,
    })
}

fn build_observed_stack_max(data: &Value) -> HashMap<String, i64> {
    let mut observed = HashMap::new();
    for source in model::SOURCES {
        let Some(items) = model::items_list(data, source) else {
            continue;
        };
        for it in items {
            let ad = model::additional_data(it);
            let Some(q) = ad.and_then(|m| m.get("StackableComponent_quantity")).and_then(|v| v.as_i64())
            else {
                continue;
            };
            let tid = model::template_id(it);
            if !tid.is_empty() {
                let e = observed.entry(tid.to_string()).or_insert(0);
                *e = (*e).max(q);
            }
        }
    }
    observed
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct RepairStats {
    pub matched: usize,
    pub changed: usize,
    pub condition: usize,
    pub durability: usize,
    pub uses: usize,
    pub stacks: usize,
}

/// Set selected items to full condition/durability and (optionally) top off
/// stacks. Faithful port of `set_items_condition_durability_full`.
pub fn repair_items(
    data: &mut Value,
    ids: &HashSet<String>,
    top_off_stacks: bool,
    catalog_stack_capacity: &HashMap<String, i64>,
) -> RepairStats {
    let mut stats = RepairStats::default();
    let observed = if top_off_stacks {
        build_observed_stack_max(data)
    } else {
        HashMap::new()
    };

    for source in model::SOURCES {
        let Some(items) = model::items_list_mut(data, source) else {
            continue;
        };
        for item in items.iter_mut() {
            let id = item.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !ids.contains(&id) {
                continue;
            }
            let tid = item.get("TemplateId").and_then(|v| v.as_str()).unwrap_or("").to_string();
            stats.matched += 1;
            let ad = ad_data_mut(item);
            let mut changed = false;

            let has_cond = ad.contains_key("Condition_d") || ad.contains_key("Condition_mt");
            if has_cond {
                if ad.get("Condition_mt").and_then(|v| v.as_f64()) != Some(CONDITION_FULL_VALUE) {
                    ad.insert("Condition_mt".into(), jf(CONDITION_FULL_VALUE));
                    changed = true;
                }
                if ad.get("Condition_d").and_then(|v| v.as_f64()) != Some(CONDITION_FULL_VALUE) {
                    ad.insert("Condition_d".into(), jf(CONDITION_FULL_VALUE));
                    changed = true;
                }
                stats.condition += 1;
            }

            let has_dur = ad.contains_key("DurabilityComponent_durability")
                || ad.contains_key("DurabilityComponent_md");
            let inferred_uses = if !has_dur && !has_cond {
                use_count_max(&tid)
            } else {
                None
            };
            if has_dur || inferred_uses.is_some() {
                match ad.get("DurabilityComponent_md").and_then(|v| v.as_f64()) {
                    None => {
                        let uses_target = inferred_uses.or_else(|| use_count_max(&tid));
                        if let Some(t) = uses_target {
                            if ad.get("DurabilityComponent_durability").and_then(|v| v.as_f64()) != Some(t) {
                                ad.insert("DurabilityComponent_durability".into(), jf(t));
                                changed = true;
                            }
                            stats.uses += 1;
                        }
                    }
                    Some(target) => {
                        if ad.get("DurabilityComponent_durability").and_then(|v| v.as_f64()) != Some(target) {
                            // Preserve the exact _md literal rather than reformat.
                            let md = ad.get("DurabilityComponent_md").cloned().unwrap();
                            ad.insert("DurabilityComponent_durability".into(), md);
                            changed = true;
                        }
                        stats.durability += 1;
                    }
                }
            }

            if top_off_stacks {
                let has_stack = ad.contains_key("StackableComponent_quantity");
                let target = stack_count_max_explicit(&tid)
                    .or_else(|| catalog_stack_capacity.get(&tid).copied())
                    .or_else(|| observed.get(&tid).copied());
                if let Some(target) = target {
                    if has_stack {
                        let cur = ad
                            .get("StackableComponent_quantity")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(1);
                        if cur < target {
                            ad.insert("StackableComponent_quantity".into(), ji(target));
                            changed = true;
                            stats.stacks += 1;
                        }
                    }
                }
            }

            if changed {
                stats.changed += 1;
            }
        }
    }
    stats
}

/// Set every stackable item (across all sources, including items nested in
/// containers - the source arrays are flat) to its maximum stack size. Max is
/// the explicit known cap, else the catalog `StackCapacity`, else the largest
/// quantity observed for that template in this save. Returns the count raised.
pub fn top_up_stacks(data: &mut Value, cat: &model::Catalog) -> usize {
    let observed = build_observed_stack_max(data);
    let mut changed = 0;
    for source in model::SOURCES {
        let Some(items) = model::items_list_mut(data, source) else {
            continue;
        };
        for item in items.iter_mut() {
            let tid = item.get("TemplateId").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let has_stack = model::additional_data(item)
                .map(|m| m.contains_key("StackableComponent_quantity"))
                .unwrap_or(false);
            if !has_stack {
                continue;
            }
            let target = stack_count_max_explicit(&tid)
                .or_else(|| cat.stack_capacity.get(&tid).copied())
                .or_else(|| observed.get(&tid).copied());
            let Some(target) = target else { continue };
            let ad = ad_data_mut(item);
            let cur = ad.get("StackableComponent_quantity").and_then(|v| v.as_i64()).unwrap_or(1);
            if cur < target {
                ad.insert("StackableComponent_quantity".into(), ji(target));
                changed += 1;
            }
        }
    }
    changed
}

// ---------- split ----------

#[derive(Debug, serde::Serialize)]
pub struct SplitResult {
    pub new_id: String,
    pub position: (i64, i64),
    pub new_quantity: i64,
    pub original_quantity: i64,
}

/// Split a stackable item; `split_quantity` moves into a clone placed in the
/// first free slot of the same parent. Port of `split_stack_item`.
pub fn split_stack(
    data: &mut Value,
    source: &str,
    item_id: &str,
    split_quantity: i64,
    cat: &model::Catalog,
    grid_width: Option<i64>,
) -> Result<SplitResult, String> {
    let (parent_id, tid, current, new_item, pos) = {
        let items = model::items_list(data, source).ok_or("unknown source")?;
        let item = items
            .iter()
            .find(|it| model::item_id(it) == Some(item_id))
            .ok_or_else(|| format!("item {item_id} not found"))?;
        let ad = model::additional_data(item);
        let current = ad
            .and_then(|m| m.get("StackableComponent_quantity"))
            .and_then(|v| v.as_i64())
            .ok_or("item is not stackable")?;
        if current < 2 {
            return Err("stack must have at least 2 to split".into());
        }
        if split_quantity < 1 || split_quantity >= current {
            return Err(format!("split must be between 1 and {}", current - 1));
        }
        let parent_id = model::parent_id(item).ok_or("item has no parent")?.to_string();
        let tid = model::template_id(item).to_string();
        let (w, h) = model::item_size(&tid, &cat.dims);
        // Default to the container's real grid width (like add/paste/move), not
        // a hard-coded 10, so a split clone never lands off an 8-wide page.
        let gw = grid_width.unwrap_or_else(|| {
            let declared = model::declared_width(items, &parent_id);
            model::effective_grid_width(items, &parent_id, declared, &cat.dims)
        });
        let occ = model::compute_occupancy(items, &parent_id, cat);
        let (pi, pj) = model::find_free_slot(&occ, w, h, gw, 256)
            .ok_or("no free slot for split stack")?;

        let mut clone = item.clone();
        let obj = clone.as_object_mut().unwrap();
        obj.insert("Id".into(), Value::String(Uuid::new_v4().to_string()));
        obj.insert("ParentId".into(), Value::String(parent_id.clone()));
        let mut p = Map::new();
        p.insert("I".into(), ji(pi));
        p.insert("J".into(), ji(pj));
        obj.insert("Position".into(), Value::Object(p));
        (parent_id, tid, current, clone, (pi, pj))
    };
    let _ = (parent_id, tid);

    // Set the clone's quantity and reduce the original.
    let new_id = new_item.get("Id").and_then(|v| v.as_str()).unwrap().to_string();
    {
        let items = model::items_list_mut(data, source).unwrap();
        // reduce original
        if let Some(orig) = items.iter_mut().find(|it| model::item_id(it) == Some(item_id)) {
            let ad = ad_data_mut(orig);
            ad.insert("StackableComponent_quantity".into(), ji(current - split_quantity));
        }
        let mut clone = new_item;
        ad_data_mut(&mut clone).insert("StackableComponent_quantity".into(), ji(split_quantity));
        items.push(clone);
    }
    Ok(SplitResult {
        new_id,
        position: pos,
        new_quantity: split_quantity,
        original_quantity: current - split_quantity,
    })
}

// ---------- add items ----------

fn build_new_item(
    parent_id: &str,
    template_id: &str,
    i: i64,
    j: i64,
    qty: Option<i64>,
    condition: Option<f64>,
    durability: Option<f64>,
) -> Value {
    let mut obj = Map::new();
    obj.insert("Id".into(), Value::String(Uuid::new_v4().to_string()));
    obj.insert("ParentId".into(), Value::String(parent_id.to_string()));
    obj.insert("TemplateId".into(), Value::String(template_id.to_string()));
    let mut pos = Map::new();
    pos.insert("I".into(), ji(i));
    pos.insert("J".into(), ji(j));
    obj.insert("Position".into(), Value::Object(pos));
    obj.insert("IsInspected".into(), Value::Bool(true));
    let mut extra = Map::new();
    if let Some(q) = qty {
        extra.insert("StackableComponent_quantity".into(), ji(q));
    }
    if let Some(c) = condition {
        extra.insert("Condition_d".into(), jf(c));
        extra.insert("Condition_mt".into(), jf(c));
    }
    if let Some(d) = durability {
        extra.insert("DurabilityComponent_durability".into(), jf(d));
        extra.insert("DurabilityComponent_md".into(), jf(d));
    }
    if !extra.is_empty() {
        let mut ad = Map::new();
        ad.insert("_data".into(), Value::Object(extra));
        obj.insert("AdditionalData".into(), Value::Object(ad));
    }
    Value::Object(obj)
}

/// Insert `count` copies of a template into a container. Port of
/// `add_items_to_data`. The grid width is derived from the destination
/// container itself (declared width, else inferred from contents) so items land
/// in a real free slot - never on top of an existing item. Returns the new ids.
pub fn add_items(
    data: &mut Value,
    template_id: &str,
    source: &str,
    owner_id: &str,
    qty: Option<i64>,
    count: i64,
    condition: Option<f64>,
    durability: Option<f64>,
    cat: &model::Catalog,
) -> Result<Vec<String>, String> {
    let (w, h) = model::item_size(template_id, &cat.dims);
    let (grid_width, mut occ) = {
        let items = model::items_list(data, source).ok_or("unknown source")?;
        let declared = items
            .iter()
            .find(|it| model::item_id(*it) == Some(owner_id))
            .and_then(|owner| model::base_component_wh(owner).0);
        let gw = model::effective_grid_width(items, owner_id, declared, &cat.dims);
        (gw, model::compute_occupancy(items, owner_id, cat))
    };
    let mut new_items = Vec::new();
    let mut new_ids = Vec::new();
    for _ in 0..count.max(1) {
        let (pi, pj) = model::find_free_slot(&occ, w, h, grid_width, 256)
            .ok_or("no free slot for new item")?;
        for di in 0..w {
            for dj in 0..h {
                occ.insert((pi + di, pj + dj));
            }
        }
        let it = build_new_item(owner_id, template_id, pi, pj, qty, condition, durability);
        new_ids.push(it.get("Id").and_then(|v| v.as_str()).unwrap().to_string());
        new_items.push(it);
    }
    let items = model::items_list_mut(data, source).unwrap();
    items.extend(new_items);
    Ok(new_ids)
}

// ---------- copy / paste subtree ----------

/// Deep-clone an item and ALL its descendants from a source (for copy). The
/// root is included; ordering is source order.
pub fn collect_subtree(data: &Value, source: &str, root_id: &str) -> Vec<Value> {
    let Some(items) = model::items_list(data, source) else {
        return Vec::new();
    };
    let mut ids: HashSet<String> = HashSet::from([root_id.to_string()]);
    loop {
        let mut added = false;
        for it in items {
            let id = it.get("Id").and_then(|v| v.as_str()).unwrap_or("");
            let pid = it.get("ParentId").and_then(|v| v.as_str()).unwrap_or("");
            if !pid.is_empty() && ids.contains(pid) && !ids.contains(id) {
                ids.insert(id.to_string());
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    items
        .iter()
        .filter(|it| it.get("Id").and_then(|v| v.as_str()).is_some_and(|i| ids.contains(i)))
        .cloned()
        .collect()
}

/// Paste a copied subtree into `dest_owner_id` with fresh UUIDs throughout. The
/// root is re-parented to the destination and placed in the first free slot;
/// descendants keep their (now-remapped) parent chain and relative positions.
/// Returns the new root id.
pub fn paste_subtree(
    data: &mut Value,
    clipboard: &[Value],
    dest_source: &str,
    dest_owner_id: &str,
    cat: &model::Catalog,
) -> Result<String, String> {
    if clipboard.is_empty() {
        return Err("nothing to paste".into());
    }
    let ids: HashSet<&str> = clipboard
        .iter()
        .filter_map(|it| it.get("Id").and_then(|v| v.as_str()))
        .collect();
    // The root is the only item whose parent isn't also in the subtree.
    let root = clipboard
        .iter()
        .find(|it| {
            let pid = it.get("ParentId").and_then(|v| v.as_str()).unwrap_or("");
            !ids.contains(pid)
        })
        .ok_or("clipboard has no root")?;
    let root_old = root.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let root_tid = model::template_id(root).to_string();

    // Fresh UUID for every item in the subtree.
    let remap: HashMap<String, String> = clipboard
        .iter()
        .filter_map(|it| it.get("Id").and_then(|v| v.as_str()))
        .map(|id| (id.to_string(), Uuid::new_v4().to_string()))
        .collect();

    // Free slot in the destination, reserving the root's max footprint.
    let (pi, pj) = {
        let items = model::items_list(data, dest_source).ok_or("unknown source")?;
        let base = model::item_size(&root_tid, &cat.dims);
        let (mw, mh) = cat.max_dims.get(&root_tid).copied().unwrap_or(base);
        let (w, h) = (base.0.max(mw), base.1.max(mh));
        let declared = items
            .iter()
            .find(|it| model::item_id(*it) == Some(dest_owner_id))
            .and_then(|o| model::base_component_wh(o).0);
        let gw = model::effective_grid_width(items, dest_owner_id, declared, &cat.dims);
        let occ = model::compute_occupancy(items, dest_owner_id, cat);
        model::find_free_slot(&occ, w, h, gw, 256).ok_or("no free slot to paste into")?
    };

    let mut clones: Vec<Value> = Vec::with_capacity(clipboard.len());
    for it in clipboard {
        let mut c = it.clone();
        let old = it.get("Id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let obj = c.as_object_mut().ok_or("bad item")?;
        obj.insert("Id".into(), Value::String(remap[&old].clone()));
        if old == root_old {
            obj.insert("ParentId".into(), Value::String(dest_owner_id.to_string()));
            let mut pos = Map::new();
            pos.insert("I".into(), ji(pi));
            pos.insert("J".into(), ji(pj));
            obj.insert("Position".into(), Value::Object(pos));
        } else if let Some(old_pid) = it.get("ParentId").and_then(|v| v.as_str()) {
            if let Some(new_pid) = remap.get(old_pid) {
                obj.insert("ParentId".into(), Value::String(new_pid.clone()));
            }
        }
        clones.push(c);
    }
    model::items_list_mut(data, dest_source).unwrap().extend(clones);
    Ok(remap[&root_old].clone())
}

/// Move an existing item into a different container in the same source (e.g.
/// another inventory page), placing it in the first free slot. Its descendants
/// follow automatically, since they are parented to the item.
pub fn move_item_to_container(
    data: &mut Value,
    source: &str,
    item_id: &str,
    dest_owner_id: &str,
    cat: &model::Catalog,
) -> Result<(), String> {
    let (pi, pj) = {
        let items = model::items_list(data, source).ok_or("unknown source")?;
        let item = items
            .iter()
            .find(|it| model::item_id(it) == Some(item_id))
            .ok_or("item not found")?;
        let tid = model::template_id(item).to_string();
        let base = model::item_size(&tid, &cat.dims);
        let (mw, mh) = cat.max_dims.get(&tid).copied().unwrap_or(base);
        let (w, h) = (base.0.max(mw), base.1.max(mh));
        let declared = items
            .iter()
            .find(|it| model::item_id(*it) == Some(dest_owner_id))
            .and_then(|o| model::base_component_wh(o).0);
        let gw = model::effective_grid_width(items, dest_owner_id, declared, &cat.dims);
        let occ = model::compute_occupancy(items, dest_owner_id, cat);
        model::find_free_slot(&occ, w, h, gw, 256).ok_or("no free slot in destination")?
    };
    let items = model::items_list_mut(data, source).ok_or("unknown source")?;
    for item in items.iter_mut() {
        if model::item_id(item) == Some(item_id) {
            let obj = item.as_object_mut().ok_or("bad item")?;
            obj.insert("ParentId".into(), Value::String(dest_owner_id.to_string()));
            let mut pos = Map::new();
            pos.insert("I".into(), ji(pi));
            pos.insert("J".into(), ji(pj));
            obj.insert("Position".into(), Value::Object(pos));
            return Ok(());
        }
    }
    Err("item not found".into())
}

// ---------- account ----------

pub fn set_experience(data: &mut Value, level: Option<i64>, xp: Option<i64>, next_goal: Option<i64>) {
    let Some(exp) = data
        .get_mut("AccountDto")
        .and_then(|a| a.get_mut("ExperienceDto"))
        .and_then(|e| e.as_object_mut())
    else {
        return;
    };
    if let Some(l) = level {
        exp.insert("Level".into(), ji(l));
    }
    if let Some(x) = xp {
        exp.insert("ExperiencePoints".into(), ji(x));
    }
    if let Some(g) = next_goal {
        exp.insert("NextLevelExperienceGoal".into(), ji(g));
    }
}

pub fn set_skill_points(data: &mut Value, count: i64) {
    if let Some(sk) = data
        .get_mut("AccountDto")
        .and_then(|a| a.get_mut("SkillsDto"))
        .and_then(|s| s.as_object_mut())
    {
        sk.insert("SkillPointsCount".into(), ji(count));
    }
}

/// Per-skill level / next-goal updates by skill Id. Returns count updated.
pub fn set_skill_levels(data: &mut Value, changes: &HashMap<i64, (Option<i64>, Option<i64>)>) -> usize {
    let Some(list) = data
        .get_mut("AccountDto")
        .and_then(|a| a.get_mut("SkillsDto"))
        .and_then(|s| s.get_mut("Skills"))
        .and_then(|v| v.as_array_mut())
    else {
        return 0;
    };
    let mut updated = 0;
    for entry in list.iter_mut() {
        let Some(sid) = entry.get("Id").and_then(|v| v.as_i64()) else {
            continue;
        };
        let Some((level, next)) = changes.get(&sid) else {
            continue;
        };
        let obj = entry.as_object_mut().unwrap();
        if let Some(l) = level {
            obj.insert("Level".into(), ji(*l));
        }
        if let Some(n) = next {
            obj.insert("NextLevelExperienceGoal".into(), ji(*n));
        }
        updated += 1;
    }
    updated
}

pub fn set_nickname(data: &mut Value, nickname: &str) {
    if let Some(acc) = data.get_mut("AccountDto").and_then(|a| a.as_object_mut()) {
        acc.insert("Nickname".into(), Value::String(nickname.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_literal_matches_python_repr() {
        // arbitrary_precision + ryu must yield "4.0", not "4".
        let v = jf(4.0);
        assert_eq!(serde_json::to_string(&v).unwrap(), "4.0");
        assert_eq!(serde_json::to_string(&ji(60)).unwrap(), "60");
    }

    #[test]
    fn set_fields_writes_expected() {
        let mut data: Value = serde_json::json!({
            "InventoryDto": { "ItemsContainerDto": { "OwnerItemId": "root", "Items": [
                {"Id": "a", "ParentId": "bp", "TemplateId": "t", "Position": {"I":0,"J":0}}
            ]}}
        });
        set_item_fields(&mut data, "inventory", "a", Some(60), Some(4.0), None).unwrap();
        let s = serde_json::to_string(&data).unwrap();
        assert!(s.contains("\"StackableComponent_quantity\":60"));
        assert!(s.contains("\"Condition_d\":4.0"));
        assert!(s.contains("\"Condition_mt\":4.0"));
    }

    #[test]
    fn set_fields_coerces_null_additional_data() {
        // A present-but-null AdditionalData (hand-edited save) must not panic;
        // it is coerced to an object and the field is written.
        let mut data: Value = serde_json::json!({
            "InventoryDto": { "ItemsContainerDto": { "OwnerItemId": "root", "Items": [
                {"Id": "a", "ParentId": "bp", "TemplateId": "t", "Position": {"I":0,"J":0},
                 "AdditionalData": null}
            ]}}
        });
        set_item_fields(&mut data, "inventory", "a", Some(42), None, None).unwrap();
        let s = serde_json::to_string(&data).unwrap();
        assert!(s.contains("\"StackableComponent_quantity\":42"));
    }
}
