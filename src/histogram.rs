use eframe::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HistogramMode {
    Rgb,
    Luminance,
}

#[derive(Clone)]
pub struct Histogram {
    pub red: [u32; 256],
    pub green: [u32; 256],
    pub blue: [u32; 256],
    pub luminance: [u32; 256],
    pub samples: u64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
            luminance: [0; 256],
            samples: 0,
        }
    }
}

impl Histogram {
    pub fn from_image(image: &image::RgbaImage) -> Self {
        let mut result = Self::default();

        for pixel in image.pixels() {
            let [r, g, b, a] = pixel.0;

            if a == 0 {
                continue;
            }

            let y = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32)
                .round()
                .clamp(0.0, 255.0) as usize;

            result.red[r as usize] += 1;
            result.green[g as usize] += 1;
            result.blue[b as usize] += 1;
            result.luminance[y] += 1;
            result.samples += 1;
        }

        result
    }

    fn scale(&self, mode: HistogramMode) -> f32 {
        let mut peaks = Vec::with_capacity(256);

        for i in 0..256 {
            let value = match mode {
                HistogramMode::Rgb => self.red[i].max(self.green[i]).max(self.blue[i]),
                HistogramMode::Luminance => self.luminance[i],
            };

            peaks.push(value);
        }

        peaks.sort_unstable();

        // Ignore a few extreme spikes when scaling the graph.
        let index = (peaks.len() * 99 / 100).min(255);
        peaks[index].max(1) as f32
    }

    fn draw_channel(
        painter: &egui::Painter,
        rect: egui::Rect,
        bins: &[u32; 256],
        peak: f32,
        color: Color32,
    ) {
        let width = rect.width();
        let height = rect.height();

        let mut points = Vec::with_capacity(256);

        for (i, count) in bins.iter().enumerate() {
            let x = rect.left() + (i as f32 / 255.0) * width;
            let value = (*count as f32 / peak).clamp(0.0, 1.0);
            let y = rect.bottom() - value * height;

            points.push(Pos2::new(x, y));
        }

        painter.add(egui::Shape::line(points, Stroke::new(1.2, color)));
    }

    pub fn show(&self, ui: &mut egui::Ui, mode: HistogramMode) {
        let width = ui.available_width().max(40.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 140.0), Sense::hover());

        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 4.0, Color32::from_rgb(19, 22, 29));

        for n in 1..4 {
            let x = rect.left() + rect.width() * n as f32 / 4.0;
            let y = rect.top() + rect.height() * n as f32 / 4.0;

            let grid = Stroke::new(0.5, Color32::from_gray(58));

            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                grid,
            );

            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                grid,
            );
        }

        if self.samples == 0 {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No image data",
                egui::FontId::proportional(12.0),
                Color32::GRAY,
            );
            return;
        }

        let peak = self.scale(mode);

        match mode {
            HistogramMode::Rgb => {
                Self::draw_channel(
                    &painter,
                    rect,
                    &self.red,
                    peak,
                    Color32::from_rgb(255, 95, 95),
                );
                Self::draw_channel(
                    &painter,
                    rect,
                    &self.green,
                    peak,
                    Color32::from_rgb(95, 225, 120),
                );
                Self::draw_channel(
                    &painter,
                    rect,
                    &self.blue,
                    peak,
                    Color32::from_rgb(95, 150, 255),
                );
            }
            HistogramMode::Luminance => {
                Self::draw_channel(
                    &painter,
                    rect,
                    &self.luminance,
                    peak,
                    Color32::from_rgb(225, 225, 230),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn histogram_counts_pixels() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 255, 0, 255]));

        let hist = Histogram::from_image(&img);

        assert_eq!(hist.samples, 2);
        assert_eq!(hist.red[255], 1);
        assert_eq!(hist.green[255], 1);
        assert_eq!(hist.blue[0], 2);
    }

    #[test]
    fn transparent_pixels_are_ignored() {
        let img = RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 0]));

        let hist = Histogram::from_image(&img);

        assert_eq!(hist.samples, 0);
    }

    #[test]
    fn black_and_white_luminance() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([255, 255, 255, 255]));

        let hist = Histogram::from_image(&img);

        assert_eq!(hist.luminance[0], 1);
        assert_eq!(hist.luminance[255], 1);
    }
}
