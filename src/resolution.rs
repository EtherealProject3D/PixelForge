//! Advisory pixel budgets, not VRAM guarantees. Never clamp a user's override.
pub struct Guidance {
    pub width: u32,
    pub height: u32,
    pub explanation: String,
    pub caution: bool,
}
pub fn recommend(model: &str, vram: Option<f64>, width: u32, height: u32) -> Guidance {
    let model = model.to_ascii_lowercase();
    let heavy = ["flux", "qwen", "z-image", "z_image"]
        .iter()
        .any(|n| model.contains(n));
    let xl = ["xl", "pony", "juggernaut", "realvis"]
        .iter()
        .any(|n| model.contains(n));
    let sd15 = ["sd1.5", "sd15", "v1-5", "sd_1.5"]
        .iter()
        .any(|n| model.contains(n));
    let family = if heavy {
        "large diffusion model"
    } else if xl {
        "SDXL-family checkpoint"
    } else if sd15 {
        "SD 1.5 checkpoint"
    } else {
        "unrecognized checkpoint"
    };
    let base = if sd15 {
        512.0_f64
    } else if heavy {
        match vram {
            Some(gb) if gb >= 16.0 => 1024.0,
            Some(gb) if gb >= 12.0 => 768.0,
            _ => 512.0,
        }
    } else if xl {
        match vram {
            Some(gb) if gb >= 8.0 => 1024.0,
            Some(gb) if gb >= 6.0 => 768.0,
            _ => 512.0,
        }
    } else {
        512.0
    };
    let ratio = (width.max(1) as f64 / height.max(1) as f64).clamp(0.125, 8.0);
    // Round down to the existing 64-pixel grid and respect the app's dimension limits.
    let snap = |n: f64| ((n / 64.0).floor() as u32 * 64).clamp(256, 4096);
    let w = snap(base * ratio.sqrt());
    let h = snap(base / ratio.sqrt());
    let memory = vram
        .map(|gb| format!("{gb:.1} GiB VRAM snapshot"))
        .unwrap_or("VRAM unknown; refresh GPU diagnostics in Dependencies".into());
    let exceeds = width as u64 * height as u64 > w as u64 * h as u64;
    Guidance {
        width: w,
        height: h,
        caution: exceeds || (heavy && vram.is_none_or(|gb| gb < 12.0)),
        explanation: format!(
            "Suggested starting size: {w} × {h} ({family}; {memory}). Heuristic only: precision, offloading, other GPU apps and model weights affect memory.{} You can keep any custom resolution.",
            if heavy {
                " Large models may fail to fit even at small sizes; legacy FP32 needs more memory."
            } else {
                ""
            }
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advice_tracks_model_and_memory_without_changing_the_request() {
        let low = recommend("CyberRealisticXL", Some(4.0), 1024, 1024);
        let high = recommend("CyberRealisticXL", Some(16.0), 1024, 1024);
        assert_eq!((low.width, high.width), (512, 1024));
        assert!(low.caution);
        assert!(!high.caution);
        assert_eq!(recommend("sd1.5", Some(16.0), 1024, 1024).width, 512);
        assert!(recommend("Qwen", Some(8.0), 512, 512).caution);
        assert_eq!(recommend("custom", None, 4096, 4096).width, 512);
    }
    #[test]
    fn aspect_ratio_stays_on_valid_grid() {
        let r = recommend("SDXL", Some(12.0), 1536, 768);
        assert_eq!(r.width % 64, 0);
        assert_eq!(r.height % 64, 0);
        assert!((r.width as f32 / r.height as f32 - 2.0).abs() < 0.15);
        assert!(recommend("Z-Image", None, 1024, 1024).caution);
    }
}

/// Public preset IDs are stable so old projects keep Standard / Large / Extra Large.
pub fn preset_dimensions(rw: u32, rh: u32, preset: usize, step: u32) -> (u32, u32) {
    let long = match preset {
        0 => 768.0,
        1 => 1344.0,
        2 => 2048.0,
        3 => 4096.0,
        4 => 1024.0,
        _ => 768.0,
    };
    let divisor = rw.max(rh).max(1) as f32;
    (
        snap(long * rw as f32 / divisor, step),
        snap(long * rh as f32 / divisor, step),
    )
}
pub fn valid_step(step: u32) -> u32 {
    if [16, 32, 64].contains(&step) {
        step
    } else {
        32
    }
}
pub fn snap(value: f32, step: u32) -> u32 {
    let step = valid_step(step);
    ((value / step as f32).round() as u32 * step).clamp(256, 4096)
}
#[cfg(test)]
mod preset_tests {
    use super::*;
    #[test]
    fn medium_uses_1024_long_side_and_requested_rounding() {
        assert_eq!(preset_dimensions(1, 1, 4, 32), (1024, 1024));
        assert_eq!(preset_dimensions(16, 9, 4, 32), (1024, 576));
        assert_eq!(preset_dimensions(9, 16, 4, 16), (576, 1024));
        assert_eq!(preset_dimensions(3, 2, 4, 32), (1024, 672));
        assert_eq!(preset_dimensions(32, 9, 4, 32), (1024, 288));
        assert_eq!(preset_dimensions(32, 9, 4, 64), (1024, 320));
        for step in [16, 32, 64] {
            for (rw, rh) in [(1, 1), (4, 3), (3, 4), (21, 9), (9, 32)] {
                let (w, h) = preset_dimensions(rw, rh, 4, step);
                assert_eq!(w.max(h), 1024);
                assert_eq!(w % step, 0);
                assert_eq!(h % step, 0);
            }
        }
    }
    #[test]
    fn original_preset_ids_are_unchanged() {
        for (id, side) in [(0, 768), (1, 1344), (2, 2048), (3, 4096)] {
            assert_eq!(preset_dimensions(1, 1, id, 64), (side, side));
        }
        assert_eq!(valid_step(48), 32);
    }
}
