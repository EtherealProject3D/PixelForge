use crate::histogram::Histogram;

#[derive(Debug, Clone)]
pub struct Analysis {
    pub mean_luminance: f32,
    pub shadow_clipping: f32,
    pub highlight_clipping: f32,
    pub shadows: f32,
    pub midtones: f32,
    pub highlights: f32,
    pub red_mean: f32,
    pub green_mean: f32,
    pub blue_mean: f32,
    pub exposure: &'static str,
    pub color_balance: &'static str,
    pub recommendations: Vec<String>,
}

impl Analysis {
    pub fn from_histogram(hist: &Histogram) -> Option<Self> {
        if hist.samples == 0 {
            return None;
        }

        let total = hist.samples as f64;
        let mut sum_y = 0.0_f64;
        let mut sum_r = 0.0_f64;
        let mut sum_g = 0.0_f64;
        let mut sum_b = 0.0_f64;

        let mut shadow_clip = 0_u64;
        let mut highlight_clip = 0_u64;
        let mut shadow_count = 0_u64;
        let mut midtone_count = 0_u64;
        let mut highlight_count = 0_u64;

        for i in 0..256 {
            let y = hist.luminance[i] as u64;
            sum_y += i as f64 * y as f64;

            sum_r += i as f64 * hist.red[i] as f64;
            sum_g += i as f64 * hist.green[i] as f64;
            sum_b += i as f64 * hist.blue[i] as f64;

            if i <= 5 {
                shadow_clip += y;
            }

            if i >= 250 {
                highlight_clip += y;
            }

            if i < 85 {
                shadow_count += y;
            } else if i < 171 {
                midtone_count += y;
            } else {
                highlight_count += y;
            }
        }

        let mean = (sum_y / total) as f32;
        let red = (sum_r / total) as f32;
        let green = (sum_g / total) as f32;
        let blue = (sum_b / total) as f32;

        let shadow_pct = (shadow_clip as f64 * 100.0 / total) as f32;
        let highlight_pct = (highlight_clip as f64 * 100.0 / total) as f32;

        let exposure = if mean < 55.0 {
            "Very dark"
        } else if mean < 95.0 {
            "Dark"
        } else if mean <= 170.0 {
            "Balanced"
        } else if mean <= 210.0 {
            "Bright"
        } else {
            "Very bright"
        };

        let max_color = red.max(green).max(blue);
        let min_color = red.min(green).min(blue);
        let spread = max_color - min_color;

        let color_balance = if spread < 12.0 {
            "No strong channel bias"
        } else if red > green + 12.0 && red > blue + 12.0 {
            "Red / warm bias"
        } else if blue > red + 12.0 && blue > green + 12.0 {
            "Blue / cool bias"
        } else if green > red + 12.0 && green > blue + 12.0 {
            "Green bias"
        } else {
            "Mixed channel imbalance"
        };

        let mut recommendations = Vec::new();

        if mean < 95.0 {
            recommendations.push(
                "Image is relatively dark. Consider increasing exposure or lifting shadows."
                    .to_string(),
            );
        } else if mean > 170.0 {
            recommendations.push(
                "Image is relatively bright. Consider reducing exposure if the scene should be darker."
                    .to_string(),
            );
        }

        if shadow_pct >= 2.0 {
            recommendations.push(format!(
                "{shadow_pct:.1}% of pixels are near black. Check whether shadow detail is being lost."
            ));
        }

        if highlight_pct >= 2.0 {
            recommendations.push(format!(
                "{highlight_pct:.1}% of pixels are near white. Check whether highlight detail is being lost."
            ));
        }

        if spread >= 20.0 {
            recommendations.push(
                "RGB channel averages differ noticeably. Check white balance against a neutral area before changing temperature or tint."
                    .to_string(),
            );
        }

        if recommendations.is_empty() {
            recommendations.push(
                "No major tonal warnings detected. Review the image visually before making adjustments."
                    .to_string(),
            );
        }

        Some(Self {
            mean_luminance: mean,
            shadow_clipping: shadow_pct,
            highlight_clipping: highlight_pct,
            shadows: (shadow_count as f64 * 100.0 / total) as f32,
            midtones: (midtone_count as f64 * 100.0 / total) as f32,
            highlights: (highlight_count as f64 * 100.0 / total) as f32,
            red_mean: red,
            green_mean: green,
            blue_mean: blue,
            exposure,
            color_balance,
            recommendations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid_histogram(rgb: [u8; 3]) -> Histogram {
        let img = RgbaImage::from_pixel(10, 10, Rgba([rgb[0], rgb[1], rgb[2], 255]));

        Histogram::from_image(&img)
    }

    #[test]
    fn empty_histogram_has_no_analysis() {
        assert!(Analysis::from_histogram(&Histogram::default()).is_none());
    }

    #[test]
    fn black_image_is_dark_and_clipped() {
        let hist = solid_histogram([0, 0, 0]);
        let analysis = Analysis::from_histogram(&hist).unwrap();

        assert_eq!(analysis.exposure, "Very dark");
        assert_eq!(analysis.shadow_clipping, 100.0);
        assert_eq!(analysis.highlight_clipping, 0.0);
    }

    #[test]
    fn white_image_is_bright_and_clipped() {
        let hist = solid_histogram([255, 255, 255]);
        let analysis = Analysis::from_histogram(&hist).unwrap();

        assert_eq!(analysis.exposure, "Very bright");
        assert_eq!(analysis.highlight_clipping, 100.0);
        assert_eq!(analysis.shadow_clipping, 0.0);
    }

    #[test]
    fn neutral_gray_is_balanced() {
        let hist = solid_histogram([128, 128, 128]);
        let analysis = Analysis::from_histogram(&hist).unwrap();

        assert_eq!(analysis.exposure, "Balanced");
        assert_eq!(analysis.color_balance, "No strong channel bias");
        assert_eq!(analysis.midtones, 100.0);
    }

    #[test]
    fn blue_dominance_is_detected() {
        let hist = solid_histogram([80, 90, 180]);
        let analysis = Analysis::from_histogram(&hist).unwrap();

        assert_eq!(analysis.color_balance, "Blue / cool bias");
    }

    #[test]
    fn tonal_percentages_sum_to_100() {
        let mut img = RgbaImage::new(3, 1);
        img.put_pixel(0, 0, Rgba([20, 20, 20, 255]));
        img.put_pixel(1, 0, Rgba([128, 128, 128, 255]));
        img.put_pixel(2, 0, Rgba([230, 230, 230, 255]));

        let hist = Histogram::from_image(&img);
        let analysis = Analysis::from_histogram(&hist).unwrap();

        let sum = analysis.shadows + analysis.midtones + analysis.highlights;

        assert!((sum - 100.0).abs() < 0.01);
    }
}
