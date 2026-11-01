/// Reusable grayscale selection mask.
/// 0 = protected, 255 = fully selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionMask {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushMode {
    Paint,
    Erase,
}

impl SelectionMask {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let count = (width as usize)
            .checked_mul(height as usize)
            .ok_or("Mask dimensions overflow")?;

        if count > isize::MAX as usize {
            return Err("Mask is too large".into());
        }

        Ok(Self {
            width,
            height,
            pixels: vec![0; count],
        })
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn from_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or("Mask dimensions overflow")?;

        if width == 0 || height == 0 || pixels.len() != expected {
            return Err("Invalid saved mask dimensions".into());
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn get(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(self.pixels[y as usize * self.width as usize + x as usize])
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    pub fn invert(&mut self) {
        for pixel in &mut self.pixels {
            *pixel = 255 - *pixel;
        }
    }

    /// Paint a circular brush dab in image-pixel coordinates.
    /// Hardness and opacity are both in the 0.0..=1.0 range.
    pub fn dab(
        &mut self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        hardness: f32,
        opacity: f32,
        mode: BrushMode,
    ) {
        if !center_x.is_finite()
            || !center_y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || self.width == 0
            || self.height == 0
        {
            return;
        }

        let hardness = if hardness.is_finite() {
            hardness.clamp(0.0, 1.0)
        } else {
            1.0
        };

        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if opacity == 0.0 {
            return;
        }

        let left = ((center_x - radius).floor().max(0.0) as u32).min(self.width - 1);
        let top = ((center_y - radius).floor().max(0.0) as u32).min(self.height - 1);
        let right = ((center_x + radius).ceil().max(0.0) as u32).min(self.width - 1);
        let bottom = ((center_y + radius).ceil().max(0.0) as u32).min(self.height - 1);

        for y in top..=bottom {
            for x in left..=right {
                let dx = x as f32 + 0.5 - center_x;
                let dy = y as f32 + 0.5 - center_y;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance > radius {
                    continue;
                }

                let normalized = distance / radius;
                let strength = if normalized <= hardness {
                    1.0
                } else if hardness >= 1.0 {
                    1.0
                } else {
                    ((1.0 - normalized) / (1.0 - hardness)).clamp(0.0, 1.0)
                };

                let alpha = strength * opacity;
                let index = y as usize * self.width as usize + x as usize;
                let old = self.pixels[index] as f32;

                let new = match mode {
                    BrushMode::Paint => old + (255.0 - old) * alpha,
                    BrushMode::Erase => old * (1.0 - alpha),
                };

                self.pixels[index] = new.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Detect near-white or near-black pixels in an RGBA image.
    /// This is a candidate clipping mask, not proof of lost detail.
    pub fn detect_clipping(
        image: &image::RgbaImage,
        highlight_threshold: u8,
        shadow_threshold: u8,
        highlights: bool,
        shadows: bool,
    ) -> Result<Self, String> {
        let mut mask = Self::new(image.width(), image.height())?;

        for (destination, pixel) in mask.pixels.iter_mut().zip(image.pixels()) {
            let [r, g, b, a] = pixel.0;

            if a == 0 {
                continue;
            }

            let highlight = highlights
                && r >= highlight_threshold
                && g >= highlight_threshold
                && b >= highlight_threshold;

            let shadow =
                shadows && r <= shadow_threshold && g <= shadow_threshold && b <= shadow_threshold;

            if highlight || shadow {
                *destination = 255;
            }
        }

        Ok(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_starts_empty() {
        let mask = SelectionMask::new(8, 6).unwrap();
        assert_eq!(mask.dimensions(), (8, 6));
        assert!(mask.pixels().iter().all(|&v| v == 0));
        assert_eq!(mask.get(8, 0), None);
    }

    #[test]
    fn painting_and_erasing() {
        let mut mask = SelectionMask::new(16, 16).unwrap();
        mask.dab(8.5, 8.5, 4.0, 1.0, 1.0, BrushMode::Paint);

        assert_eq!(mask.get(8, 8), Some(255));
        assert_eq!(mask.get(0, 0), Some(0));

        mask.dab(8.5, 8.5, 4.0, 1.0, 1.0, BrushMode::Erase);
        assert_eq!(mask.get(8, 8), Some(0));
    }

    #[test]
    fn soft_brush_has_partial_values() {
        let mut mask = SelectionMask::new(16, 16).unwrap();
        mask.dab(8.5, 8.5, 6.0, 0.0, 1.0, BrushMode::Paint);

        assert!(mask.pixels().iter().any(|&v| v > 0 && v < 255));
    }

    #[test]
    fn inversion_and_clear() {
        let mut mask = SelectionMask::new(2, 2).unwrap();
        mask.invert();
        assert!(mask.pixels().iter().all(|&v| v == 255));

        mask.clear();
        assert!(mask.pixels().iter().all(|&v| v == 0));
    }

    #[test]
    fn detects_bright_and_dark_pixels() {
        let mut image = image::RgbaImage::new(3, 1);
        image.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(1, 0, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(2, 0, image::Rgba([120, 130, 140, 255]));

        let mask = SelectionMask::detect_clipping(&image, 250, 5, true, true).unwrap();

        assert_eq!(mask.pixels(), &[255, 255, 0]);
    }
}
