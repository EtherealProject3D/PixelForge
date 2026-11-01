#[derive(Clone, Copy, Debug, Default)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub exposure: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub vibrance: f32,
    pub gamma: f32,
}

impl Adjustments {
    pub fn apply_pixel(self, pixel: &mut image::Rgba<u8>) {
        // Exposure in photographic stops.
        let exposure_factor = 2.0_f32.powf(self.exposure);
        let contrast_factor = 1.0 + self.contrast / 100.0;
        let brightness_offset = self.brightness * 2.55;

        let mut rgb = [0.0_f32; 3];

        for (index, channel) in rgb.iter_mut().enumerate() {
            let value = pixel[index] as f32 * exposure_factor;

            *channel = (value - 127.5) * contrast_factor + 127.5 + brightness_offset;
        }

        // Temperature: positive warms, negative cools.
        // Tint: positive magenta, negative green.
        // Approximate RGB color balance, not RAW Kelvin calibration.
        let warmth = self.temperature * 0.65;
        let tint = self.tint * 0.45;

        rgb[0] += warmth + tint;
        rgb[1] -= tint;
        rgb[2] += -warmth + tint;

        // Rec.709 luminance.
        let luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];

        let saturation_factor = 1.0 + self.saturation / 100.0;

        // Smooth tonal weighting:
        // shadows affect darker pixels most;
        // highlights affect brighter pixels most.
        let tone = (luminance / 255.0).clamp(0.0, 1.0);
        let shadow_weight = (1.0 - tone).powi(2);
        let highlight_weight = tone.powi(2);

        // Positive shadows brighten dark regions.
        // Negative highlights darken bright regions.
        let tonal_offset =
            self.shadows * 1.2 * shadow_weight + self.highlights * 1.2 * highlight_weight;

        // Endpoint controls target narrower tonal ranges.
        let white_weight = tone.powi(4);
        let black_weight = (1.0 - tone).powi(4);

        let endpoint_offset = self.whites * 1.25 * white_weight + self.blacks * 1.25 * black_weight;

        // Vibrance emphasizes less-saturated colors.
        let maximum = rgb[0].max(rgb[1]).max(rgb[2]);
        let minimum = rgb[0].min(rgb[1]).min(rgb[2]);
        let chroma = (maximum - minimum).clamp(0.0, 255.0) / 255.0;

        let strength = self.vibrance / 100.0;

        let vibrance_factor = if strength >= 0.0 {
            1.0 + strength * (1.0 - chroma)
        } else {
            1.0 + strength
        };

        // Positive gamma brightens midtones.
        // Negative gamma darkens midtones.
        let gamma_exponent = 2.0_f32.powf(-self.gamma / 100.0);

        for channel in 0..3 {
            let value = luminance
                + (rgb[channel] - luminance) * saturation_factor * vibrance_factor
                + tonal_offset
                + endpoint_offset;

            let normalized = (value / 255.0).clamp(0.0, 1.0);
            let corrected = normalized.powf(gamma_exponent) * 255.0;

            pixel[channel] = corrected.clamp(0.0, 255.0).round() as u8;
        }

        // Alpha remains unchanged.
    }

    pub fn apply(self, image: &mut image::RgbaImage) {
        for pixel in image.pixels_mut() {
            self.apply_pixel(pixel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_adjustments_preserve_pixels() {
        let mut pixel = image::Rgba([30, 100, 210, 127]);
        Adjustments::default().apply_pixel(&mut pixel);
        assert_eq!(pixel.0, [30, 100, 210, 127]);
    }

    #[test]
    fn exposure_one_stop_doubles_intensity() {
        let mut pixel = image::Rgba([40, 60, 80, 255]);

        Adjustments {
            exposure: 1.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert_eq!(pixel.0, [80, 120, 160, 255]);
    }

    #[test]
    fn temperature_warms_and_cools() {
        let original = image::Rgba([120, 120, 120, 255]);
        let mut warm = original;
        let mut cool = original;

        Adjustments {
            temperature: 50.0,
            ..Default::default()
        }
        .apply_pixel(&mut warm);

        Adjustments {
            temperature: -50.0,
            ..Default::default()
        }
        .apply_pixel(&mut cool);

        assert!(warm[0] > warm[2]);
        assert!(cool[2] > cool[0]);
    }

    #[test]
    fn tint_moves_green_and_magenta() {
        let original = image::Rgba([120, 120, 120, 255]);
        let mut magenta = original;
        let mut green = original;

        Adjustments {
            tint: 50.0,
            ..Default::default()
        }
        .apply_pixel(&mut magenta);

        Adjustments {
            tint: -50.0,
            ..Default::default()
        }
        .apply_pixel(&mut green);

        assert!(magenta[0] > magenta[1]);
        assert!(magenta[2] > magenta[1]);
        assert!(green[1] > green[0]);
        assert!(green[1] > green[2]);
    }

    #[test]
    fn positive_shadows_affect_dark_pixels_more() {
        let mut dark = image::Rgba([40, 40, 40, 255]);
        let mut bright = image::Rgba([210, 210, 210, 255]);

        Adjustments {
            shadows: 50.0,
            ..Default::default()
        }
        .apply_pixel(&mut dark);

        Adjustments {
            shadows: 50.0,
            ..Default::default()
        }
        .apply_pixel(&mut bright);

        assert!(dark[0] as i32 - 40 > bright[0] as i32 - 210);
    }

    #[test]
    fn negative_highlights_affect_bright_pixels_more() {
        let mut dark = image::Rgba([40, 40, 40, 255]);
        let mut bright = image::Rgba([210, 210, 210, 255]);

        Adjustments {
            highlights: -50.0,
            ..Default::default()
        }
        .apply_pixel(&mut dark);

        Adjustments {
            highlights: -50.0,
            ..Default::default()
        }
        .apply_pixel(&mut bright);

        assert!(210 - bright[0] as i32 > 40 - dark[0] as i32);
    }

    #[test]
    fn tonal_adjustments_preserve_alpha() {
        let mut pixel = image::Rgba([80, 120, 180, 77]);

        Adjustments {
            highlights: -35.0,
            shadows: 45.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert_eq!(pixel[3], 77);
    }

    #[test]
    fn zero_saturation_produces_grayscale() {
        let mut pixel = image::Rgba([220, 60, 30, 190]);

        Adjustments {
            saturation: -100.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
        assert_eq!(pixel[3], 190);
    }

    #[test]
    fn whites_affect_bright_pixels_more() {
        let mut dark = image::Rgba([40, 40, 40, 255]);
        let mut bright = image::Rgba([210, 210, 210, 255]);

        let settings = Adjustments {
            whites: 50.0,
            ..Default::default()
        };

        settings.apply_pixel(&mut dark);
        settings.apply_pixel(&mut bright);

        assert!(bright[0] as i32 - 210 > dark[0] as i32 - 40);
    }

    #[test]
    fn blacks_affect_dark_pixels_more() {
        let mut dark = image::Rgba([40, 40, 40, 255]);
        let mut bright = image::Rgba([210, 210, 210, 255]);

        let settings = Adjustments {
            blacks: -50.0,
            ..Default::default()
        };

        settings.apply_pixel(&mut dark);
        settings.apply_pixel(&mut bright);

        assert!(40 - dark[0] as i32 > 210 - bright[0] as i32);
    }

    #[test]
    fn whites_and_blacks_preserve_alpha() {
        let mut pixel = image::Rgba([100, 140, 200, 83]);

        Adjustments {
            whites: 35.0,
            blacks: -25.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert_eq!(pixel[3], 83);
    }

    #[test]
    fn positive_gamma_lifts_midtones() {
        let mut pixel = image::Rgba([80, 120, 180, 255]);

        Adjustments {
            gamma: 50.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert!(pixel[0] > 80);
        assert!(pixel[1] > 120);
        assert!(pixel[2] > 180);
    }

    #[test]
    fn negative_gamma_darkens_midtones() {
        let mut pixel = image::Rgba([80, 120, 180, 255]);

        Adjustments {
            gamma: -50.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert!(pixel[0] < 80);
        assert!(pixel[1] < 120);
        assert!(pixel[2] < 180);
    }

    #[test]
    fn gamma_preserves_endpoints() {
        for gamma in [-100.0, 0.0, 100.0] {
            let mut black = image::Rgba([0, 0, 0, 255]);
            let mut white = image::Rgba([255, 255, 255, 255]);

            let settings = Adjustments {
                gamma,
                ..Default::default()
            };

            settings.apply_pixel(&mut black);
            settings.apply_pixel(&mut white);

            assert_eq!(black.0, [0, 0, 0, 255]);
            assert_eq!(white.0, [255, 255, 255, 255]);
        }
    }

    #[test]
    fn vibrance_emphasizes_muted_colors() {
        let mut muted = image::Rgba([140, 120, 110, 255]);
        let mut vivid = image::Rgba([230, 50, 30, 255]);

        let settings = Adjustments {
            vibrance: 75.0,
            ..Default::default()
        };

        settings.apply_pixel(&mut muted);
        settings.apply_pixel(&mut vivid);

        let muted_gain = (muted[0] as i32 - muted[2] as i32) as f32 / 30.0;

        let vivid_gain = (vivid[0] as i32 - vivid[2] as i32) as f32 / 200.0;

        assert!(muted_gain > vivid_gain);
    }

    #[test]
    fn vibrance_and_gamma_preserve_alpha() {
        let mut pixel = image::Rgba([100, 140, 180, 73]);

        Adjustments {
            vibrance: 45.0,
            gamma: -25.0,
            ..Default::default()
        }
        .apply_pixel(&mut pixel);

        assert_eq!(pixel[3], 73);
    }
}
