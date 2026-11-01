#[derive(Clone, Copy, Debug, Default)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub exposure: f32,
    pub saturation: f32,
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

        // Rec.709 luminance.
        let luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];

        let saturation_factor = 1.0 + self.saturation / 100.0;

        for channel in 0..3 {
            let value = luminance + (rgb[channel] - luminance) * saturation_factor;

            pixel[channel] = value.clamp(0.0, 255.0).round() as u8;
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
}
