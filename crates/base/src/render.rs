//! Shared render primitives.
//!
//! Domain-agnostic building blocks for the "render to a temp dir, then atomic
//! swap onto the canonical location" pattern, shared by install rendering and
//! (from T-013) adapter regeneration. Pairs with [`crate::platform::atomic_swap`].
//!
//! OE-01 fence (architect): only genuinely shared, install-agnostic primitives
//! live here. Install-domain logic (source scan, canonical-root render, provider
//! manifest generation) stays in its owning crate. Template-string substitution
//! helpers are added here when the second consumer (adapters, T-013) needs them.

use std::io;
use std::path::{Path, PathBuf};

/// Create a uniquely-named staging directory (`.gal-render-<uuid>`) under
/// `parent`, creating `parent` first if needed.
///
/// The caller validates the parent (e.g. derives it from a canonical root) and
/// maps the returned `io::Error` to its own domain error type. After rendering
/// into the returned directory, finalize with [`crate::platform::atomic_swap`].
pub fn create_temp_render_dir(parent: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(parent)?;

    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let temp_name = format!(".gal-render-{}", uuid);
    let temp_dir = parent.join(temp_name);

    std::fs::create_dir(&temp_dir)?;

    Ok(temp_dir)
}
