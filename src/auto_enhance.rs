use crate::analysis::Analysis;
use crate::histogram::Histogram;

#[derive(Debug, Clone, Copy)]
pub struct EnhancePlan {
    pub exposure: f32,
    pub shadows: f32,
    pub highlights: f32,
    pub temperature: f32,
    pub tint: f32,
    pub vibrance: f32,
}

impl EnhancePlan {
    pub fn from_histogram(hist: &Histogram) -> Option<Self> {
        let report = Analysis::from_histogram(hist)?;

        let mean = report.mean_luminance;

        // Conservative exposure correction, measured in EV.
        // Avoid trying to turn intentional night scenes into daylight.
        let exposure = if mean < 45.0 {
            0.65
        } else if mean < 75.0 {
            0.45
        } else if mean < 105.0 {
            0.20
        } else if mean > 215.0 {
            -0.40
        } else if mean > 185.0 {
            -0.20
        } else {
            0.0
        };

        // Lift dark tonal regions without applying excessive global
        // exposure. Reduce highlights only when there is evidence
        // of significant bright-area concentration.
        let shadows = if report.shadows > 65.0 {
            22.0
        } else if report.shadows > 45.0 {
            12.0
        } else {
            0.0
        };

        let highlights = if report.highlight_clipping > 8.0 {
            -30.0
        } else if report.highlight_clipping > 2.0 {
            -18.0
        } else if report.highlights > 65.0 {
            -10.0
        } else {
            0.0
        };

        // RGB channel averages are not a reliable white-balance
        // measurement for every scene. Only use small corrections
        // when the difference is pronounced.
        let red_blue_difference = report.red_mean - report.blue_mean;

        let temperature = if red_blue_difference > 35.0 {
            -8.0
        } else if red_blue_difference < -35.0 {
            8.0
        } else {
            0.0
        };

        let green_reference = (report.red_mean + report.blue_mean) * 0.5;

        let green_difference = report.green_mean - green_reference;

        let tint = if green_difference > 30.0 {
            6.0
        } else if green_difference < -30.0 {
            -6.0
        } else {
            0.0
        };

        let vibrance = if mean > 70.0
            && mean < 185.0
            && (report.red_mean - report.green_mean).abs() < 12.0
            && (report.green_mean - report.blue_mean).abs() < 12.0
        {
            6.0
        } else {
            0.0
        };

        Some(Self {
            exposure,
            shadows,
            highlights,
            temperature,
            tint,
            vibrance,
        })
    }

    pub fn has_changes(&self) -> bool {
        self.exposure != 0.0
            || self.shadows != 0.0
            || self.highlights != 0.0
            || self.temperature != 0.0
            || self.tint != 0.0
            || self.vibrance != 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(rgb: [u8; 3]) -> Histogram {
        let image = RgbaImage::from_pixel(16, 16, Rgba([rgb[0], rgb[1], rgb[2], 255]));

        Histogram::from_image(&image)
    }

    #[test]
    fn empty_image_produces_no_plan() {
        assert!(EnhancePlan::from_histogram(&Histogram::default()).is_none());
    }

    #[test]
    fn dark_image_receives_positive_exposure() {
        let plan = EnhancePlan::from_histogram(&solid([35, 35, 35])).unwrap();

        assert!(plan.exposure > 0.0);
        assert!(plan.shadows > 0.0);
    }

    #[test]
    fn bright_image_receives_negative_exposure() {
        let plan = EnhancePlan::from_histogram(&solid([235, 235, 235])).unwrap();

        assert!(plan.exposure < 0.0);
        assert!(plan.highlights < 0.0);
    }

    #[test]
    fn neutral_midgray_has_no_exposure_change() {
        let plan = EnhancePlan::from_histogram(&solid([128, 128, 128])).unwrap();

        assert_eq!(plan.exposure, 0.0);
        assert_eq!(plan.temperature, 0.0);
        assert_eq!(plan.tint, 0.0);
    }

    #[test]
    fn warm_bias_receives_cooling_correction() {
        let plan = EnhancePlan::from_histogram(&solid([200, 130, 100])).unwrap();

        assert!(plan.temperature < 0.0);
    }

    #[test]
    fn cool_bias_receives_warming_correction() {
        let plan = EnhancePlan::from_histogram(&solid([100, 130, 200])).unwrap();

        assert!(plan.temperature > 0.0);
    }

    #[test]
    fn values_stay_within_safe_bounds() {
        for rgb in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [128, 128, 128],
        ] {
            let plan = EnhancePlan::from_histogram(&solid(rgb)).unwrap();

            assert!(plan.exposure.abs() <= 0.65);
            assert!(plan.shadows.abs() <= 22.0);
            assert!(plan.highlights.abs() <= 30.0);
            assert!(plan.temperature.abs() <= 8.0);
            assert!(plan.tint.abs() <= 6.0);
            assert!(plan.vibrance.abs() <= 6.0);
        }
    }
}
