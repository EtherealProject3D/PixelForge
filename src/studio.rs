use crate::{adjustments, masking};
use serde_json::{json, Value};

#[derive(Clone, Default)]
pub struct EditorState {
    pub name: String,
    pub global: adjustments::Adjustments,
    pub local: adjustments::Adjustments,
    pub mask: Option<masking::SelectionMask>,
    pub saved_selections: Vec<(String, masking::SelectionMask)>,
}

impl PartialEq for EditorState {
    fn eq(&self, other: &Self) -> bool {
        self.name==other.name && crate::adjustment_values(self.global)==crate::adjustment_values(other.global)
            && crate::adjustment_values(self.local)==crate::adjustment_values(other.local)
            && self.mask==other.mask && self.saved_selections==other.saved_selections
    }
}

impl EditorState {
    pub fn render(&self, source: &image::RgbaImage) -> image::RgbaImage {
        let mut pixels = source.clone();
        let (width, height) = pixels.dimensions();
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            self.global.apply_pixel(pixel);
            if let Some(mask) = &self.mask {
                let (mw, mh) = mask.dimensions();
                let alpha = mask.get((x as u64 * mw as u64 / width as u64) as u32,
                    (y as u64 * mh as u64 / height as u64) as u32).unwrap_or(0);
                if alpha > 0 {
                    let before = *pixel;
                    self.local.apply_pixel(pixel);
                    for c in 0..3 { pixel[c] = ((before[c] as u32 * (255-alpha) as u32 + pixel[c] as u32 * alpha as u32 + 127)/255) as u8; }
                }
            }
        }
        pixels
    }

    pub fn to_json(&self) -> Value {
        json!({"name":self.name,"global":crate::adjustment_values(self.global),"local":crate::adjustment_values(self.local),
            "mask":self.mask.as_ref().map(encode_mask),
            "saved":self.saved_selections.iter().map(|(name,mask)|json!({"name":name,"mask":encode_mask(mask)})).collect::<Vec<_>>()})
    }

    pub fn from_json(value: &Value, width: u32, height: u32) -> Result<Self, String> {
        if value.is_null() { return Ok(Self::default()); }
        fn adjustment(value: &Value) -> Result<adjustments::Adjustments, String> {
            let values: [f32;12] = serde_json::from_value(value.clone()).map_err(|e|format!("Invalid Studio adjustments: {e}"))?;
            if !values.iter().all(|v|v.is_finite()) {return Err("Invalid Studio adjustments".into());}
            Ok(crate::adjustments_from_values(values))
        }
        let mut saved_selections = Vec::new();
        if let Some(saved) = value["saved"].as_array() {
            if saved.len()>256 {return Err("Too many Studio selections".into());}
            for entry in saved {
                saved_selections.push((entry["name"].as_str().unwrap_or("Selection").to_owned(), decode_mask(&entry["mask"],width,height)?));
            }
        }
        Ok(Self {name:value["name"].as_str().unwrap_or("").into(),global:adjustment(&value["global"] )?,local:adjustment(&value["local"] )?,
            mask:if value["mask"].is_null(){None}else{Some(decode_mask(&value["mask"],width,height)?)}, saved_selections})
    }
}

fn encode_mask(mask: &masking::SelectionMask) -> Value {
    let mut runs: Vec<(u64,u8)> = Vec::new();
    for &pixel in mask.pixels() {
        if let Some(last) = runs.last_mut().filter(|last|last.1==pixel) {last.0+=1;}
        else {runs.push((1,pixel));}
    }
    json!(runs)
}
fn decode_mask(value: &Value, width: u32, height: u32) -> Result<masking::SelectionMask,String> {
    let count = (width as usize).checked_mul(height as usize).ok_or("Studio mask size overflow")?;
    let runs: Vec<(u64,u8)> = serde_json::from_value(value.clone()).map_err(|e|format!("Invalid Studio mask: {e}"))?;
    let mut pixels = Vec::new();
    for (length, pixel) in runs {
        let length = usize::try_from(length).map_err(|_|"Studio mask size overflow")?;
        let end = pixels.len().checked_add(length).ok_or("Studio mask size overflow")?;
        if length==0 || end>count {return Err("Studio mask exceeds image dimensions".into());}
        pixels.resize(end,pixel);
    }
    masking::SelectionMask::from_pixels(width,height,pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn studio_state_roundtrip_retains_local_edits_and_named_selections() {
        let mut state = EditorState::default();
        state.name="Restored photo".into();
        state.global.exposure=1.0;
        state.local.brightness=20.0;
        let mask=masking::SelectionMask::from_pixels(2,1,vec![255,0]).unwrap();
        state.mask=Some(mask.clone());state.saved_selections.push(("Subject".into(),mask));
        let loaded=EditorState::from_json(&state.to_json(),2,1).unwrap();
        assert_eq!(loaded.mask,state.mask);
        assert_eq!(loaded.saved_selections[0].0,"Subject");
        let image=image::RgbaImage::from_pixel(2,1,image::Rgba([30,30,30,255]));
        assert_eq!(loaded.render(&image),state.render(&image));
        assert!(loaded.render(&image).get_pixel(0,0)[0]>loaded.render(&image).get_pixel(1,0)[0]);
    }
    #[test]
    fn invalid_mask_runs_are_rejected_and_legacy_images_load() {
        assert!(decode_mask(&json!([[3,255]]),2,1).is_err());
        assert!(decode_mask(&json!([[0,255]]),2,1).is_err());
        assert!(EditorState::from_json(&Value::Null,2,1).is_ok());
    }
}
