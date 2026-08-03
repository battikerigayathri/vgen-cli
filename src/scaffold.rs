pub use crate::kit::{ensure_live_dirs, kit_is_present, workspace_has_live_artifacts};

pub fn known_recipes() -> &'static [&'static str] {
    &["oracle-pr", "minimal", "form-wizard"]
}
