//! The Cargo Hunters save engine, ported from the validated Python
//! (`save_io.py` / `add_item.py` / `editor_session.py`).
//!
//! Pure Rust - no Tauri dependency - so it builds fast and is exercised by the
//! oracle harness: identical operations on real saves must produce
//! byte-identical output to the Python engine.

pub mod io;
pub mod model;
pub mod ops;
pub mod skills;
pub mod snapshot;

pub use io::{
    atomic_write_text, default_save_path, list_save_backups, load_save, make_timestamped_backup,
    prune_old_backups, serialize_pretty, write_save, IoError, Save,
};
