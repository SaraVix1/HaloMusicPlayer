//! Settings → Appearance → Renderer: GPU (wgpu) or Software (tiny-skia).
//!
//! iced picks its renderer from `ICED_BACKEND` when the first window is
//! made, so the saved choice is read from the database file at the very
//! start of `main` and applies from the next launch. Measured on Linux
//! (HAZARDS): Software starts faster and uses about a third of the memory,
//! but costs ~35 % of a core while covers fade, growing with window size.

use std::sync::OnceLock;

pub const PREF: &str = "ui.renderer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    Gpu,
    Software,
}

impl Renderer {
    pub fn from_pref(v: Option<&str>) -> Self {
        match v {
            Some("software") => Renderer::Software,
            _ => Renderer::Gpu,
        }
    }

    pub fn as_pref(self) -> &'static str {
        match self {
            Renderer::Gpu => "gpu",
            Renderer::Software => "software",
        }
    }
}

static AT_LAUNCH: OnceLock<Renderer> = OnceLock::new();

/// Set `ICED_BACKEND` from the saved choice; an explicit `ICED_BACKEND`
/// wins (scripts/perf-measure.py). GPU leaves it unset, so iced still falls
/// back to software when there is no usable GPU. Must run before any thread
/// starts (it changes the environment).
pub fn apply_saved() {
    let chosen = match std::env::var("ICED_BACKEND") {
        Ok(v) => Renderer::from_pref((v == "tiny-skia").then_some("software")),
        Err(_) => {
            let r = Renderer::from_pref(halo_backend::commands::ui::read_pref_early(PREF).as_deref());
            if r == Renderer::Software {
                std::env::set_var("ICED_BACKEND", "tiny-skia");
            }
            r
        }
    };
    let _ = AT_LAUNCH.set(chosen);
}

/// The renderer this run started with.
pub fn at_launch() -> Renderer {
    AT_LAUNCH.get().copied().unwrap_or(Renderer::Gpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pref_round_trip() {
        for r in [Renderer::Gpu, Renderer::Software] {
            assert_eq!(Renderer::from_pref(Some(r.as_pref())), r);
        }
        assert_eq!(Renderer::from_pref(None), Renderer::Gpu);
        assert_eq!(Renderer::from_pref(Some("nonsense")), Renderer::Gpu);
    }
}
