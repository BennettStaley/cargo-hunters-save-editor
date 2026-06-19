//! Oracle CLI - exercises the Rust engine so its output can be diffed against
//! the Python engine byte-for-byte. Used only by the test harness, never shipped.
//!
//! Usage:
//!   oracle roundtrip <in.save> <out.json>   load + serialize_pretty -> out
//!
//! More subcommands (repair/move/delete/...) are added as those ops are ported.

use std::path::Path;
use std::process::ExitCode;

use ch_engine as engine;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: oracle <command> [args...]");
        return ExitCode::FAILURE;
    }
    let cmd = args[1].as_str();
    let result = match cmd {
        "roundtrip" => roundtrip(&args[2..]),
        "snapshot" => snapshot(&args[2..]),
        "op" => op(&args[2..]),
        "catalog" => catalog(&args[2..]),
        other => Err(format!("unknown command: {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("oracle error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Load a save and write the byte-faithful serialization to `out`.
fn roundtrip(args: &[String]) -> Result<(), String> {
    let [inp, out] = args else {
        return Err("roundtrip needs <in.save> <out.json>".into());
    };
    let data = engine::load_save(Path::new(inp)).map_err(|e| e.to_string())?;
    let text = engine::serialize_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(Path::new(out), text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Build the frontend snapshot and print a human summary (+ full JSON to `out`
/// if given). Used to eyeball the read model against a real save.
fn snapshot(args: &[String]) -> Result<(), String> {
    let (save, csv, out) = match args {
        [s, c] => (s.clone(), c.clone(), None),
        [s, c, o] => (s.clone(), c.clone(), Some(o.clone())),
        _ => return Err("snapshot needs <in.save> <catalog.csv> [out.json]".into()),
    };
    let data = engine::load_save(Path::new(&save)).map_err(|e| e.to_string())?;
    let cat = engine::model::load_catalog(Path::new(&csv));
    let snap = engine::snapshot::build_snapshot(&data, &save, &cat);

    eprintln!("pages: {}", snap.pages.len());
    for pg in &snap.pages {
        eprintln!("  page {} id={} items={}", pg.index, &pg.id[..8.min(pg.id.len())], pg.item_count);
    }
    eprintln!("containers: {}", snap.containers.len());
    eprintln!("--- equipment top-level by slot ---");
    let mut by_slot: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for it in snap.equipment.iter().filter(|it| it.parent_id.is_none()) {
        by_slot.entry(it.slot.as_str()).or_default().push(it.name.as_str());
    }
    for (slot, names) in &by_slot {
        eprintln!("  {slot:20} {names:?}");
    }
    eprintln!(
        "account: nick={:?} level={:?} xp={:?} skills={}",
        snap.account.nickname, snap.account.level, snap.account.xp, snap.account.skills.len()
    );

    if let Some(out) = out {
        let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
        std::fs::write(Path::new(&out), json).map_err(|e| e.to_string())?;
        eprintln!("wrote full snapshot -> {out}");
    }
    Ok(())
}

/// Apply a single mutation and write the resulting save (byte-faithful) to out.
/// Mirrors what the Python op-harness does so the two can be diffed.
///   op repair  <save> <out> <id,id,...>
///   op set     <save> <out> <source> <id> <qty|-> <cond|-> <dur|->
///   op delete  <save> <out> <id,id,...>
///   op move    <save> <out> <source> <id> <i> <j>
///   op add     <save> <csv> <out> <source> <owner> <template> <count> <gridw>
///   op split   <save> <csv> <out> <source> <id> <splitqty> <gridw>
fn op(args: &[String]) -> Result<(), String> {
    use std::collections::HashSet;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    fn opt_i(s: &str) -> Option<i64> {
        if s == "-" { None } else { s.parse().ok() }
    }
    fn opt_f(s: &str) -> Option<f64> {
        if s == "-" { None } else { s.parse().ok() }
    }

    match sub {
        "repair" => {
            let [_, save, out, ids] = args else {
                return Err("op repair <save> <out> <ids>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let set: HashSet<String> = ids.split(',').map(|s| s.to_string()).collect();
            // Match the Python default: explicit table + observed only.
            engine::ops::repair_items(&mut data, &set, true, &Default::default());
            write_out(&data, out)
        }
        "set" => {
            let [_, save, out, source, id, q, c, d] = args else {
                return Err("op set <save> <out> <source> <id> <qty> <cond> <dur>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            engine::ops::set_item_fields(&mut data, source, id, opt_i(q), opt_f(c), opt_f(d))?;
            write_out(&data, out)
        }
        "delete" => {
            let [_, save, out, ids] = args else {
                return Err("op delete <save> <out> <ids>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let set: HashSet<String> = ids.split(',').map(|s| s.to_string()).collect();
            engine::ops::remove_items_by_ids(&mut data, &set);
            write_out(&data, out)
        }
        "move" => {
            let [_, save, out, source, id, i, j] = args else {
                return Err("op move <save> <out> <source> <id> <i> <j>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            engine::ops::move_item_position(&mut data, source, id, i.parse().unwrap(), j.parse().unwrap())?;
            write_out(&data, out)
        }
        "add" => {
            let [_, save, csv, out, source, owner, template, count] = args else {
                return Err("op add <save> <csv> <out> <source> <owner> <template> <count>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let cat = engine::model::load_catalog(Path::new(csv));
            engine::ops::add_items(&mut data, template, source, owner, None,
                count.parse().unwrap(), None, None, &cat)?;
            write_out(&data, out)
        }
        "split" => {
            let [_, save, csv, out, source, id, sq, gw] = args else {
                return Err("op split <save> <csv> <out> <source> <id> <splitqty> <gridw>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let cat = engine::model::load_catalog(Path::new(csv));
            engine::ops::split_stack(&mut data, source, id, sq.parse().unwrap(), &cat, Some(gw.parse().unwrap()))?;
            write_out(&data, out)
        }
        "topup" => {
            let [_, save, csv, out] = args else {
                return Err("op topup <save> <csv> <out>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let cat = engine::model::load_catalog(Path::new(csv));
            let n = engine::ops::top_up_stacks(&mut data, &cat);
            eprintln!("topped up {n} stacks");
            write_out(&data, out)
        }
        "copypaste" => {
            let [_, save, csv, out, source, copy_id, dest_owner] = args else {
                return Err("op copypaste <save> <csv> <out> <source> <copy_id> <dest_owner>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let cat = engine::model::load_catalog(Path::new(csv));
            let clip = engine::ops::collect_subtree(&data, source, copy_id);
            eprintln!("copied subtree of {} item(s)", clip.len());
            let new_root = engine::ops::paste_subtree(&mut data, &clip, source, dest_owner, &cat)?;
            eprintln!("pasted, new root id {new_root}");
            write_out(&data, out)
        }
        "movepage" => {
            let [_, save, csv, out, source, item_id, dest_owner] = args else {
                return Err("op movepage <save> <csv> <out> <source> <item_id> <dest_owner>".into());
            };
            let mut data = engine::load_save(Path::new(save)).map_err(|e| e.to_string())?;
            let cat = engine::model::load_catalog(Path::new(csv));
            engine::ops::move_item_to_container(&mut data, source, item_id, dest_owner, &cat)?;
            eprintln!("moved {item_id} -> {dest_owner}");
            write_out(&data, out)
        }
        other => Err(format!("unknown op: {other}")),
    }
}

/// Dump the full browsable catalog to JSON (for the frontend dev mock).
fn catalog(args: &[String]) -> Result<(), String> {
    let [csv, out] = args else {
        return Err("catalog <csv> <out.json>".into());
    };
    let cat = engine::model::load_catalog(Path::new(csv));
    let entries = engine::snapshot::catalog_entries(&cat);
    let json = serde_json::to_string(&entries).map_err(|e| e.to_string())?;
    std::fs::write(Path::new(out), json).map_err(|e| e.to_string())
}

fn write_out(data: &engine::Save, out: &str) -> Result<(), String> {
    let text = engine::serialize_pretty(data).map_err(|e| e.to_string())?;
    std::fs::write(Path::new(out), text.as_bytes()).map_err(|e| e.to_string())
}
