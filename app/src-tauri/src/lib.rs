//! Tauri command layer. All save-format logic lives in the pure `ch_engine`
//! crate; this file exposes it to the SolidJS frontend and holds the in-memory
//! editing session: edits mutate the working copy, nothing touches disk until
//! an explicit, validated `save_game`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use ch_engine as engine;
use engine::model::Catalog;
use engine::snapshot::Snapshot;
use serde::Serialize;
use serde_json::Value;

// Catalog embedded so it works identically in dev and the bundled exe.
const CATALOG_CSV: &str = include_str!("../../../all_items_detailed.csv");
const QUESTS_CSV: &str = include_str!("../../../quests_catalog.json");

struct AppState {
    catalog: Catalog,
    quest_catalog: engine::quests::QuestCatalog,
    save_path: Option<PathBuf>,
    data: Option<Value>,
    dirty: bool,
    /// Copied item subtree (item + descendants) + its display name, for paste.
    clipboard: Option<Vec<Value>>,
    clipboard_label: Option<String>,
}

impl AppState {
    fn new() -> Self {
        Self {
            catalog: engine::model::load_catalog_str(CATALOG_CSV),
            quest_catalog: engine::quests::load_quest_catalog_str(QUESTS_CSV),
            save_path: None,
            data: None,
            dirty: false,
            clipboard: None,
            clipboard_label: None,
        }
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        let data = self.data.as_ref().ok_or("no save loaded")?;
        let path = self.save_path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let mut snap = engine::snapshot::build_snapshot(data, &path, &self.catalog);
        snap.dirty = self.dirty;
        snap.clipboard = self.clipboard_label.clone();
        Ok(snap)
    }

    fn data_mut(&mut self) -> Result<&mut Value, String> {
        self.data.as_mut().ok_or_else(|| "no save loaded".to_string())
    }
}

type Shared = Mutex<AppState>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveResult {
    ok: bool,
    message: String,
    backup: Option<String>,
}

// ---- helpers ----
fn lock<'a>(
    state: &'a tauri::State<Shared>,
) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    // Recover from a poisoned mutex (a prior command panicked while holding the
    // lock) instead of bricking every later command - the worst case is the one
    // failed op; the session stays usable. The engine never leaves `data` in a
    // half-written state (mutations build new values before swapping them in).
    Ok(state.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Run a mutation that needs catalog dims, then return the fresh snapshot.
fn after_mut(st: &mut AppState) -> Result<Snapshot, String> {
    st.dirty = true;
    st.snapshot()
}

// ---- read ----
#[tauri::command]
fn default_save_path() -> Option<String> {
    engine::default_save_path().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn load_state(state: tauri::State<Shared>, path: Option<String>) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let path = match path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => engine::default_save_path().ok_or("could not resolve default save path")?,
    };
    let data = engine::load_save(&path).map_err(|e| e.to_string())?;
    st.save_path = Some(path);
    st.data = Some(data);
    st.dirty = false;
    st.snapshot()
}

#[tauri::command]
fn current_snapshot(state: tauri::State<Shared>) -> Result<Snapshot, String> {
    lock(&state)?.snapshot()
}

#[tauri::command]
fn list_catalog(state: tauri::State<Shared>) -> Result<Vec<engine::snapshot::CatalogEntry>, String> {
    let st = lock(&state)?;
    Ok(engine::snapshot::catalog_entries(&st.catalog))
}

/// Decipher the in-progress (and tally other) missions from the loaded save.
#[tauri::command]
fn list_missions(state: tauri::State<Shared>) -> Result<engine::quests::MissionsView, String> {
    let st = lock(&state)?;
    let data = st.data.as_ref().ok_or("no save loaded")?;
    Ok(engine::quests::build_missions(data, &st.quest_catalog))
}

/// Add a required mission material to the vault so the player can hand it in
/// in-game. Stackable templates are added as one stack of `count`; others as
/// `count` individual items. Lands on the first inventory page.
#[tauri::command]
fn grant_mission_item(state: tauri::State<Shared>, template_id: String, count: i64) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    {
        let s = &mut *st; // disjoint borrows: data (mut) + catalog (shared)
        let data = s.data.as_mut().ok_or("no save loaded")?;
        let page = engine::model::inventory_page_ids(data)
            .into_iter()
            .next()
            .ok_or("no inventory page to add to")?;
        let n = count.max(1);
        let stackable = s.catalog.stack_capacity.contains_key(&template_id);
        // Stackable -> one stack of n; non-stackable -> n separate items.
        let (qty, copies) = if stackable { (Some(n), 1) } else { (None, n) };
        engine::ops::add_items(data, &template_id, "inventory", &page, qty, copies, None, None, &s.catalog)?;
    }
    after_mut(&mut st)
}

// ---- item mutations ----
#[tauri::command]
fn apply_item(
    state: tauri::State<Shared>,
    source: String,
    item_id: String,
    quantity: Option<i64>,
    condition: Option<f64>,
    durability: Option<f64>,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    engine::ops::set_item_fields(st.data_mut()?, &source, &item_id, quantity, condition, durability)?;
    after_mut(&mut st)
}

#[tauri::command]
fn repair_items(state: tauri::State<Shared>, ids: Vec<String>) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let set: HashSet<String> = ids.into_iter().collect();
    // Match the shipped tool: explicit stack table + observed maxes (no catalog cap).
    engine::ops::repair_items(st.data_mut()?, &set, true, &Default::default());
    after_mut(&mut st)
}

#[tauri::command]
fn top_up_stacks(state: tauri::State<Shared>) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    {
        let s = &mut *st; // disjoint field borrows: data (mut) + catalog (shared)
        let data = s.data.as_mut().ok_or("no save loaded")?;
        engine::ops::top_up_stacks(data, &s.catalog);
    }
    after_mut(&mut st)
}

#[tauri::command]
fn delete_items(state: tauri::State<Shared>, ids: Vec<String>) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let set: HashSet<String> = ids.into_iter().collect();
    engine::ops::remove_items_by_ids(st.data_mut()?, &set);
    after_mut(&mut st)
}

/// Copy an item (and its descendants) to the in-memory clipboard. Not a save edit.
#[tauri::command]
fn copy_item(state: tauri::State<Shared>, source: String, item_id: String) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let sub = {
        let data = st.data.as_ref().ok_or("no save loaded")?;
        engine::ops::collect_subtree(data, &source, &item_id)
    };
    if sub.is_empty() {
        return Err("item not found".into());
    }
    let label = sub
        .iter()
        .find(|it| it.get("Id").and_then(|v| v.as_str()) == Some(item_id.as_str()))
        .and_then(|it| it.get("TemplateId").and_then(|v| v.as_str()))
        .map(|tid| st.catalog.names.get(tid).cloned().unwrap_or_else(|| tid.chars().take(8).collect()));
    st.clipboard = Some(sub);
    st.clipboard_label = label;
    st.snapshot()
}

/// Paste the clipboard subtree (fresh UUIDs) into a destination container.
#[tauri::command]
fn paste_item(state: tauri::State<Shared>, dest_source: String, dest_owner_id: String) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let clip = st.clipboard.clone().ok_or("nothing to paste")?;
    {
        let s = &mut *st; // disjoint field borrows: data (mut) + catalog (shared)
        let data = s.data.as_mut().ok_or("no save loaded")?;
        engine::ops::paste_subtree(data, &clip, &dest_source, &dest_owner_id, &s.catalog)?;
    }
    after_mut(&mut st)
}

#[tauri::command]
fn move_item(
    state: tauri::State<Shared>,
    source: String,
    item_id: String,
    i: i64,
    j: i64,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    engine::ops::move_item_position(st.data_mut()?, &source, &item_id, i, j)?;
    after_mut(&mut st)
}

/// Move an item into another container (e.g. a different inventory page).
#[tauri::command]
fn move_to_page(
    state: tauri::State<Shared>,
    source: String,
    item_id: String,
    dest_owner_id: String,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    {
        let s = &mut *st; // disjoint field borrows: data (mut) + catalog (shared)
        let data = s.data.as_mut().ok_or("no save loaded")?;
        engine::ops::move_item_to_container(data, &source, &item_id, &dest_owner_id, &s.catalog)?;
    }
    after_mut(&mut st)
}

#[tauri::command]
fn split_stack(
    state: tauri::State<Shared>,
    source: String,
    item_id: String,
    split_quantity: i64,
    grid_width: Option<i64>,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    {
        let s = &mut *st; // disjoint field borrows: data (mut) + catalog (shared)
        let data = s.data.as_mut().ok_or("no save loaded")?;
        engine::ops::split_stack(data, &source, &item_id, split_quantity, &s.catalog, grid_width)?;
    }
    after_mut(&mut st)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn add_items(
    state: tauri::State<Shared>,
    template_id: String,
    source: String,
    owner_id: String,
    quantity: Option<i64>,
    count: i64,
    condition: Option<f64>,
    durability: Option<f64>,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    {
        let s = &mut *st; // disjoint field borrows: data (mut) + catalog (shared)
        let data = s.data.as_mut().ok_or("no save loaded")?;
        engine::ops::add_items(
            data, &template_id, &source, &owner_id, quantity, count,
            condition, durability, &s.catalog,
        )?;
    }
    after_mut(&mut st)
}

// ---- account ----
#[tauri::command]
fn set_account(
    state: tauri::State<Shared>,
    nickname: Option<String>,
    level: Option<i64>,
    xp: Option<i64>,
    next_goal: Option<i64>,
    skill_points: Option<i64>,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let data = st.data_mut()?;
    if level.is_some() || xp.is_some() || next_goal.is_some() {
        engine::ops::set_experience(data, level, xp, next_goal);
    }
    if let Some(sp) = skill_points {
        engine::ops::set_skill_points(data, sp);
    }
    if let Some(nick) = nickname {
        engine::ops::set_nickname(data, &nick);
    }
    after_mut(&mut st)
}

#[tauri::command]
fn set_skill(
    state: tauri::State<Shared>,
    skill_id: i64,
    level: Option<i64>,
    next_goal: Option<i64>,
) -> Result<Snapshot, String> {
    let mut st = lock(&state)?;
    let mut changes = std::collections::HashMap::new();
    changes.insert(skill_id, (level, next_goal));
    engine::ops::set_skill_levels(st.data_mut()?, &changes);
    after_mut(&mut st)
}

// ---- save (write + validate) ----
#[tauri::command]
fn save_game(state: tauri::State<Shared>) -> Result<SaveResult, String> {
    let mut st = lock(&state)?;
    if !st.dirty {
        return Ok(SaveResult { ok: true, message: "No staged changes.".into(), backup: None });
    }
    let path = st.save_path.clone().ok_or("no save loaded")?;
    let data = st.data.clone().ok_or("no save loaded")?;
    let backup = engine::write_save(&path, &data, true, Some(20)).map_err(|e| e.to_string())?;
    // Validate: re-read from disk and confirm it matches the working copy exactly.
    let on_disk = engine::load_save(&path).map_err(|e| e.to_string())?;
    let ok = on_disk == data;
    if ok {
        st.dirty = false;
    }
    Ok(SaveResult {
        ok,
        message: if ok {
            "Saved & verified - on-disk file matches staged edits.".into()
        } else {
            "WARNING: on-disk file does not match staged edits (round-trip mismatch).".into()
        },
        backup: backup.map(|p| p.to_string_lossy().into_owned()),
    })
}

#[tauri::command]
fn reload_from_disk(state: tauri::State<Shared>) -> Result<Snapshot, String> {
    let path = { lock(&state)?.save_path.clone() };
    load_state(state, path.map(|p| p.to_string_lossy().into_owned()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(AppState::new()))
        .invoke_handler(tauri::generate_handler![
            default_save_path,
            load_state,
            current_snapshot,
            list_catalog,
            list_missions,
            grant_mission_item,
            apply_item,
            repair_items,
            top_up_stacks,
            delete_items,
            copy_item,
            paste_item,
            move_to_page,
            move_item,
            split_stack,
            add_items,
            set_account,
            set_skill,
            save_game,
            reload_from_disk,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
