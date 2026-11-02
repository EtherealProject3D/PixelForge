use std::io::{Cursor, Read, Write};
use std::path::Path;

const MAGIC_V1: &[u8; 4] = b"PXF1";
const MAGIC_V2: &[u8; 4] = b"PXF2";
const MAGIC_V3: &[u8; 4] = b"PXF3";
const MAGIC_V4: &[u8; 4] = b"PXF4";
const MAGIC_V5: &[u8; 4] = b"PXF5";
const MAGIC_V6: &[u8; 4] = b"PXF6";
const MAGIC_V7: &[u8; 4] = b"PXF7";
const MAGIC_V8: &[u8; 4] = b"PXF8";
const MAGIC_V9: &[u8; 4] = b"PXF9";
const MAGIC_V10: &[u8; 4] = b"PX10";
const MAX_SELECTIONS: usize = 128;
pub const MAX_VERSIONS: usize = 128;
const MAX_SELECTION_NAME: usize = 256;
const MAX_BYTES: u64 = 1_073_741_824;
const MAX_NAME: usize = 4096;

// Payload tag 0 represents an adjustment checkpoint; other tags are
// reserved for future raster AI-edit versions.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectVersion {
    pub global: [f32; 12],
    pub local: [f32; 12],
    pub mask: Option<Vec<u8>>,
    pub raster: Option<image::RgbaImage>,
}

pub struct Project {
    pub pixels: image::RgbaImage,
    pub image_name: String,
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
    pub comparison_enabled: bool,
    pub comparison_position: f32,
    pub saved_selections: Vec<(String, Vec<u8>)>,
    pub selection_adjustments: [f32; 12],
    pub active_mask: Option<Vec<u8>>,
    pub selection_target: bool,
    pub edit_versions: Vec<ProjectVersion>,
}

fn read<const N: usize>(reader: &mut impl Read) -> Result<[u8; N], String> {
    let mut value = [0u8; N];
    reader.read_exact(&mut value).map_err(|e| e.to_string())?;
    Ok(value)
}

fn valid_adjustments(values: &[f32; 12]) -> bool {
    values.iter().enumerate().all(|(index, value)| {
        value.is_finite() && (if index == 2 {
            (-3.0..=3.0).contains(value)
        } else {
            (-100.0..=100.0).contains(value)
        })
    })
}

pub fn save(path: &Path, project: &Project) -> Result<(), String> {
    let mut png = Cursor::new(Vec::new());

    image::DynamicImage::ImageRgba8(project.pixels.clone())
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    let png = png.into_inner();
    let name = project.image_name.as_bytes();

    if name.len() > MAX_NAME {
        return Err("Image filename is too long".into());
    }

    let mut data = Vec::new();
    data.extend_from_slice(MAGIC_V10);
    data.extend_from_slice(&project.brightness.to_le_bytes());
    data.extend_from_slice(&project.contrast.to_le_bytes());
    data.extend_from_slice(&project.exposure.to_le_bytes());
    data.extend_from_slice(&project.saturation.to_le_bytes());
    data.extend_from_slice(&project.temperature.to_le_bytes());
    data.extend_from_slice(&project.tint.to_le_bytes());
    data.extend_from_slice(&project.highlights.to_le_bytes());
    data.extend_from_slice(&project.shadows.to_le_bytes());
    data.extend_from_slice(&project.whites.to_le_bytes());
    data.extend_from_slice(&project.blacks.to_le_bytes());
    data.extend_from_slice(&project.vibrance.to_le_bytes());
    data.extend_from_slice(&project.gamma.to_le_bytes());
    data.push(u8::from(project.comparison_enabled));
    data.extend_from_slice(&project.comparison_position.to_le_bytes());
    data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    data.extend_from_slice(name);
    data.extend_from_slice(&(png.len() as u64).to_le_bytes());
    data.extend_from_slice(&png);

    if project.saved_selections.len() > MAX_SELECTIONS {
        return Err("Too many saved selections".into());
    }

    data.extend_from_slice(&(project.saved_selections.len() as u32).to_le_bytes());

    let expected = (project.pixels.width() as usize)
        .checked_mul(project.pixels.height() as usize)
        .ok_or("Image dimensions overflow")?;

    for (selection_name, mask_pixels) in &project.saved_selections {
        let name_bytes = selection_name.as_bytes();

        if selection_name.trim().is_empty()
            || name_bytes.len() > MAX_SELECTION_NAME
            || mask_pixels.len() != expected
        {
            return Err("Invalid saved selection".into());
        }

        let gray = image::GrayImage::from_raw(
            project.pixels.width(),
            project.pixels.height(),
            mask_pixels.clone(),
        )
        .ok_or("Invalid selection mask")?;

        let mut output = Cursor::new(Vec::new());

        image::DynamicImage::ImageLuma8(gray)
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;

        let encoded = output.into_inner();

        data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(name_bytes);
        data.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
        data.extend_from_slice(&encoded);
    }

    // PXF8 appends selection-targeted adjustments and the current working mask.
    for value in project.selection_adjustments {
        if !value.is_finite() { return Err("Invalid selection adjustment".into()); }
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.push(u8::from(project.selection_target));
    if let Some(mask) = &project.active_mask {
        if mask.len() != expected { return Err("Invalid active selection mask".into()); }
        let gray = image::GrayImage::from_raw(project.pixels.width(), project.pixels.height(), mask.clone())
            .ok_or("Invalid active selection mask")?;
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(gray).write_to(&mut output, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let encoded = output.into_inner();
        data.push(1);
        data.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
        data.extend_from_slice(&encoded);
    } else {
        data.push(0);
    }

    // PXF9: counted, tagged edit checkpoints appended to PXF8 data.
    if project.edit_versions.len() > MAX_VERSIONS {
        return Err("Too many edit versions".into());
    }
    data.extend_from_slice(&(project.edit_versions.len() as u32).to_le_bytes());
    for version in &project.edit_versions {
        if !valid_adjustments(&version.global) || !valid_adjustments(&version.local) {
            return Err("Invalid edit version adjustments".into());
        }
        data.push(if version.raster.is_some() { 1 } else { 0 });
        for value in version.global {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for value in version.local {
            data.extend_from_slice(&value.to_le_bytes());
        }
        if let Some(mask) = &version.mask {
            if mask.len() != expected {
                return Err("Invalid edit version mask".into());
            }
            let gray = image::GrayImage::from_raw(
                project.pixels.width(), project.pixels.height(), mask.clone()
            ).ok_or("Invalid edit version mask")?;
            let mut output = Cursor::new(Vec::new());
            image::DynamicImage::ImageLuma8(gray)
                .write_to(&mut output, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            let encoded = output.into_inner();
            data.push(1);
            data.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
            data.extend_from_slice(&encoded);
        } else {
            data.push(0);
        }
        if let Some(raster) = &version.raster {
            if raster.dimensions() != project.pixels.dimensions() {
                return Err("Image version dimensions mismatch".into());
            }
            let mut output = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(raster.clone())
                .write_to(&mut output, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            let encoded = output.into_inner();
            data.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
            data.extend_from_slice(&encoded);
        }
    }

    if data.len() as u64 > MAX_BYTES {
        return Err("Project exceeds the 1 GiB limit".into());
    }

    // Write a complete temporary project before replacing
    // an existing one.
    let mut temporary = None;

    for number in 0..1000 {
        let candidate = path.with_extension(format!("pxf.tmp.{number:03}"));

        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.to_string()),
        }
    }

    let (temp_path, mut file) = temporary.ok_or("Could not allocate a temporary project file")?;

    let written = file.write_all(&data).and_then(|_| file.sync_all());

    drop(file);

    if let Err(err) = written {
        let _ = std::fs::remove_file(&temp_path);
        return Err(err.to_string());
    }

    if path.exists() {
        let mut backup = None;

        for number in 0..1000 {
            let candidate = path.with_extension(format!("pxf.bak.{number:03}"));
            if !candidate.exists() {
                backup = Some(candidate);
                break;
            }
        }

        let backup = backup.ok_or("No backup filename available")?;

        if let Err(err) = std::fs::rename(path, &backup) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(err.to_string());
        }

        if let Err(err) = std::fs::rename(&temp_path, path) {
            let restored = std::fs::rename(&backup, path);
            let _ = std::fs::remove_file(&temp_path);

            return match restored {
                Ok(()) => Err(err.to_string()),
                Err(recovery) => Err(format!(
                    "Save failed: {err}. Recovery file: {}. Restore error: {recovery}",
                    backup.display()
                )),
            };
        }

        let _ = std::fs::remove_file(backup);
    } else if let Err(err) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(err.to_string());
    }

    Ok(())
}

pub fn load(path: &Path) -> Result<Project, String> {
    let length = std::fs::metadata(path).map_err(|e| e.to_string())?.len();

    if length > MAX_BYTES {
        return Err("Project exceeds the 1 GiB limit".into());
    }

    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut reader = Cursor::new(bytes.as_slice());

    let version = read::<4>(&mut reader)?;

    if &version != MAGIC_V1
        && &version != MAGIC_V2
        && &version != MAGIC_V3
        && &version != MAGIC_V4
        && &version != MAGIC_V5
        && &version != MAGIC_V6
        && &version != MAGIC_V7
        && &version != MAGIC_V8
        && &version != MAGIC_V9
        && &version != MAGIC_V10
    {
        return Err("Unsupported PixelForge project format".into());
    }

    let brightness = f32::from_le_bytes(read::<4>(&mut reader)?);
    let contrast = f32::from_le_bytes(read::<4>(&mut reader)?);

    let (exposure, saturation) = if matches!(
        &version,
        MAGIC_V2 | MAGIC_V3 | MAGIC_V4 | MAGIC_V5 | MAGIC_V6 | MAGIC_V7 | MAGIC_V8 | MAGIC_V9 | MAGIC_V10
    ) {
        (
            f32::from_le_bytes(read::<4>(&mut reader)?),
            f32::from_le_bytes(read::<4>(&mut reader)?),
        )
    } else {
        (0.0, 0.0)
    };

    let (temperature, tint) = if matches!(
        &version,
        MAGIC_V3 | MAGIC_V4 | MAGIC_V5 | MAGIC_V6 | MAGIC_V7 | MAGIC_V8 | MAGIC_V9 | MAGIC_V10
    ) {
        (
            f32::from_le_bytes(read::<4>(&mut reader)?),
            f32::from_le_bytes(read::<4>(&mut reader)?),
        )
    } else {
        (0.0, 0.0)
    };

    let (highlights, shadows) = if matches!(&version, MAGIC_V4 | MAGIC_V5 | MAGIC_V6 | MAGIC_V7 | MAGIC_V8 | MAGIC_V9 | MAGIC_V10) {
        (
            f32::from_le_bytes(read::<4>(&mut reader)?),
            f32::from_le_bytes(read::<4>(&mut reader)?),
        )
    } else {
        (0.0, 0.0)
    };

    let (whites, blacks) = if matches!(&version, MAGIC_V5 | MAGIC_V6 | MAGIC_V7 | MAGIC_V8 | MAGIC_V9 | MAGIC_V10) {
        (
            f32::from_le_bytes(read::<4>(&mut reader)?),
            f32::from_le_bytes(read::<4>(&mut reader)?),
        )
    } else {
        (0.0, 0.0)
    };

    let (vibrance, gamma) = if matches!(&version, MAGIC_V6 | MAGIC_V7 | MAGIC_V8 | MAGIC_V9 | MAGIC_V10) {
        (
            f32::from_le_bytes(read::<4>(&mut reader)?),
            f32::from_le_bytes(read::<4>(&mut reader)?),
        )
    } else {
        (0.0, 0.0)
    };

    let enabled = read::<1>(&mut reader)?[0];
    let comparison_position = f32::from_le_bytes(read::<4>(&mut reader)?);

    if !brightness.is_finite()
        || !contrast.is_finite()
        || !exposure.is_finite()
        || !saturation.is_finite()
        || !temperature.is_finite()
        || !tint.is_finite()
        || !highlights.is_finite()
        || !shadows.is_finite()
        || !whites.is_finite()
        || !blacks.is_finite()
        || !vibrance.is_finite()
        || !gamma.is_finite()
        || !(-100.0..=100.0).contains(&vibrance)
        || !(-100.0..=100.0).contains(&gamma)
        || !(-100.0..=100.0).contains(&whites)
        || !(-100.0..=100.0).contains(&blacks)
        || !(-100.0..=100.0).contains(&highlights)
        || !(-100.0..=100.0).contains(&shadows)
        || !(-100.0..=100.0).contains(&temperature)
        || !(-100.0..=100.0).contains(&tint)
        || !(-3.0..=3.0).contains(&exposure)
        || !(-100.0..=100.0).contains(&saturation)
        || !comparison_position.is_finite()
        || !(-100.0..=100.0).contains(&brightness)
        || !(-100.0..=100.0).contains(&contrast)
        || !(0.0..=1.0).contains(&comparison_position)
        || enabled > 1
    {
        return Err("Invalid project adjustment settings".into());
    }

    let name_length = u32::from_le_bytes(read::<4>(&mut reader)?) as usize;

    if name_length > MAX_NAME {
        return Err("Invalid filename length".into());
    }

    let mut name = vec![0u8; name_length];
    reader.read_exact(&mut name).map_err(|e| e.to_string())?;

    let image_name = String::from_utf8(name).map_err(|e| e.to_string())?;
    let image_length = u64::from_le_bytes(read::<8>(&mut reader)?);

    if image_length == 0
        || image_length > MAX_BYTES
        || image_length > bytes.len() as u64 - reader.position()
        || (&version != MAGIC_V7 && &version != MAGIC_V8 && &version != MAGIC_V9 && &version != MAGIC_V10 && image_length != bytes.len() as u64 - reader.position())
    {
        return Err("Invalid or incomplete embedded image".into());
    }

    let start = reader.position() as usize;

    let image_end = start
        .checked_add(image_length as usize)
        .ok_or("Invalid embedded image length")?;

    let pixels =
        image::load_from_memory_with_format(&bytes[start..image_end], image::ImageFormat::Png)
            .map_err(|e| e.to_string())?
            .to_rgba8();

    let mut saved_selections = Vec::new();

    if matches!(&version, MAGIC_V7 | MAGIC_V8 | MAGIC_V9 | MAGIC_V10) {
        reader.set_position(image_end as u64);

        let count = u32::from_le_bytes(read::<4>(&mut reader)?) as usize;

        if count > MAX_SELECTIONS {
            return Err("Too many saved selections".into());
        }

        for _ in 0..count {
            let name_length = u32::from_le_bytes(read::<4>(&mut reader)?) as usize;

            if name_length == 0 || name_length > MAX_SELECTION_NAME {
                return Err("Invalid saved selection name length".into());
            }

            let mut name_bytes = vec![0u8; name_length];

            reader
                .read_exact(&mut name_bytes)
                .map_err(|e| e.to_string())?;

            let name = String::from_utf8(name_bytes).map_err(|e| e.to_string())?;

            if name.trim().is_empty() {
                return Err("Invalid saved selection name".into());
            }

            let mask_length = u64::from_le_bytes(read::<8>(&mut reader)?);

            if mask_length == 0 || mask_length > bytes.len() as u64 - reader.position() {
                return Err("Invalid saved selection length".into());
            }

            let begin = reader.position() as usize;

            let end = begin
                .checked_add(mask_length as usize)
                .ok_or("Invalid saved selection length")?;

            let gray =
                image::load_from_memory_with_format(&bytes[begin..end], image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?
                    .to_luma8();

            if gray.dimensions() != pixels.dimensions() {
                return Err("Saved selection dimensions mismatch".into());
            }

            saved_selections.push((name, gray.into_raw()));
            reader.set_position(end as u64);
        }

        if &version == MAGIC_V7 && reader.position() != bytes.len() as u64 {
            return Err("Unexpected trailing project data".into());
        }
    }

    let mut selection_adjustments = [0.0_f32; 12];
    let mut active_mask = None;
    let mut selection_target = false;
    if matches!(&version, MAGIC_V8 | MAGIC_V9 | MAGIC_V10) {
        for value in &mut selection_adjustments {
            *value = f32::from_le_bytes(read::<4>(&mut reader)?);
            if !value.is_finite() { return Err("Invalid selection adjustment".into()); }
        }
        selection_target = match read::<1>(&mut reader)?[0] {
            0 => false, 1 => true, _ => return Err("Invalid selection target".into()),
        };
        match read::<1>(&mut reader)?[0] {
            0 => {},
            1 => {
                let length = u64::from_le_bytes(read::<8>(&mut reader)?);
                if length == 0 || length > bytes.len() as u64 - reader.position() {
                    return Err("Invalid active selection length".into());
                }
                let begin = reader.position() as usize;
                let end = begin.checked_add(length as usize).ok_or("Invalid active selection length")?;
                let gray = image::load_from_memory_with_format(&bytes[begin..end], image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?.to_luma8();
                if gray.dimensions() != pixels.dimensions() { return Err("Active selection dimensions mismatch".into()); }
                active_mask = Some(gray.into_raw());
                reader.set_position(end as u64);
            },
            _ => return Err("Invalid active selection flag".into()),
        }
        if &version == MAGIC_V8 && reader.position() != bytes.len() as u64 {
            return Err("Unexpected trailing project data".into());
        }
    }

    let mut edit_versions = Vec::new();
    if &version == MAGIC_V9 || &version == MAGIC_V10 {
        let count = u32::from_le_bytes(read::<4>(&mut reader)?) as usize;
        if count > MAX_VERSIONS {
            return Err("Too many edit versions".into());
        }
        for _ in 0..count {
            let tag = read::<1>(&mut reader)?[0];
            if tag > 1 || (tag == 1 && &version != MAGIC_V10) {
                return Err("Unsupported edit version payload".into());
            }
            let mut global = [0.0_f32; 12];
            let mut local = [0.0_f32; 12];
            for value in &mut global {
                *value = f32::from_le_bytes(read::<4>(&mut reader)?);
            }
            for value in &mut local {
                *value = f32::from_le_bytes(read::<4>(&mut reader)?);
            }
            if !valid_adjustments(&global) || !valid_adjustments(&local) {
                return Err("Invalid edit version adjustments".into());
            }
            let mask = match read::<1>(&mut reader)?[0] {
                0 => None,
                1 => {
                    let length = u64::from_le_bytes(read::<8>(&mut reader)?);
                    if length == 0 || length > bytes.len() as u64 - reader.position() {
                        return Err("Invalid edit version mask length".into());
                    }
                    let begin = reader.position() as usize;
                    let end = begin.checked_add(length as usize)
                        .ok_or("Invalid edit version mask length")?;
                    let gray = image::load_from_memory_with_format(
                        &bytes[begin..end], image::ImageFormat::Png,
                    ).map_err(|e| e.to_string())?.to_luma8();
                    if gray.dimensions() != pixels.dimensions() {
                        return Err("Edit version mask dimensions mismatch".into());
                    }
                    reader.set_position(end as u64);
                    Some(gray.into_raw())
                }
                _ => return Err("Invalid edit version mask flag".into()),
            };
            let raster = if tag == 1 {
                let length = u64::from_le_bytes(read::<8>(&mut reader)?);
                if length == 0 || length > bytes.len() as u64 - reader.position() {
                    return Err("Invalid image version length".into());
                }
                let begin = reader.position() as usize;
                let end = begin.checked_add(length as usize).ok_or("Invalid image version length")?;
                let rgba = image::load_from_memory_with_format(&bytes[begin..end], image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?.to_rgba8();
                if rgba.dimensions() != pixels.dimensions() {
                    return Err("Image version dimensions mismatch".into());
                }
                reader.set_position(end as u64);
                Some(rgba)
            } else { None };
            edit_versions.push(ProjectVersion { global, local, mask, raster });
        }
        if reader.position() != bytes.len() as u64 {
            return Err("Unexpected trailing project data".into());
        }
    }

    Ok(Project {
        pixels,
        image_name,
        brightness,
        contrast,
        exposure,
        saturation,
        temperature,
        tint,
        highlights,
        shadows,
        whites,
        blacks,
        vibrance,
        gamma,
        comparison_enabled: enabled == 1,
        comparison_position,
        saved_selections,
        selection_adjustments,
        active_mask,
        selection_target,
        edit_versions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_round_trip() {
        // Windows-safe unique filename: no thread-name colons.
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path =
            std::env::temp_dir().join(format!("pixelforge_{}_{}.pxf", std::process::id(), unique));

        let original = Project {
            pixels: image::RgbaImage::from_pixel(2, 2, image::Rgba([12, 34, 56, 78])),
            image_name: "test.png".into(),
            brightness: 25.0,
            contrast: -10.0,
            exposure: 1.2,
            saturation: 35.0,
            temperature: 42.0,
            tint: -18.0,
            highlights: -35.0,
            shadows: 48.0,
            whites: 32.0,
            blacks: -27.0,
            vibrance: 38.0,
            gamma: -22.0,
            comparison_enabled: true,
            comparison_position: 0.3,
            saved_selections: vec![("Subject".into(), vec![0, 64, 128, 255])],
            selection_adjustments: [1.0; 12],
            active_mask: Some(vec![0, 64, 128, 255]),
            selection_target: true,
            edit_versions: vec![
                ProjectVersion {
                    global: [12.0, -10.0, 1.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0],
                    local: [0.0; 12],
                    mask: Some(vec![0, 64, 128, 255]),
                    raster: None,
                },
                ProjectVersion {
                    global: [0.0; 12],
                    local: [0.0, 8.0, -0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    mask: None,
                    raster: Some(image::RgbaImage::from_pixel(2, 2, image::Rgba([90, 80, 70, 255]))),
                },
            ],
        };

        save(&path, &original).expect("PXF save failed");
        let loaded = load(&path).expect("PXF load failed");
        std::fs::remove_file(&path).unwrap();

        assert_eq!(loaded.pixels, original.pixels);
        assert_eq!(loaded.saved_selections, original.saved_selections);
        assert_eq!(loaded.selection_adjustments, original.selection_adjustments);
        assert_eq!(loaded.active_mask, original.active_mask);
        assert_eq!(loaded.selection_target, original.selection_target);
        assert_eq!(loaded.edit_versions, original.edit_versions);
        assert_eq!(loaded.image_name, original.image_name);
        assert_eq!(loaded.brightness, original.brightness);
        assert_eq!(loaded.contrast, original.contrast);
        assert_eq!(loaded.exposure, original.exposure);
        assert_eq!(loaded.saturation, original.saturation);
        assert_eq!(loaded.temperature, original.temperature);
        assert_eq!(loaded.tint, original.tint);
        assert_eq!(loaded.highlights, original.highlights);
        assert_eq!(loaded.shadows, original.shadows);
        assert_eq!(loaded.whites, original.whites);
        assert_eq!(loaded.blacks, original.blacks);
        assert_eq!(loaded.vibrance, original.vibrance);
        assert_eq!(loaded.gamma, original.gamma);
        assert_eq!(loaded.comparison_enabled, original.comparison_enabled);
        assert_eq!(loaded.comparison_position, original.comparison_position);
    }

    #[test]
    fn pxf8_loads_without_version_history() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pixelforge_legacy_{}_{}.pxf", std::process::id(), unique
        ));
        let project = Project {
            pixels: image::RgbaImage::from_pixel(2, 2, image::Rgba([7, 8, 9, 255])),
            image_name: "legacy.png".into(),
            brightness: 1.0, contrast: 0.0, exposure: 0.0, saturation: 0.0,
            temperature: 0.0, tint: 0.0, highlights: 0.0, shadows: 0.0,
            whites: 0.0, blacks: 0.0, vibrance: 0.0, gamma: 0.0,
            comparison_enabled: false, comparison_position: 0.5,
            saved_selections: vec![], selection_adjustments: [0.0; 12],
            active_mask: None, selection_target: false, edit_versions: vec![],
        };
        save(&path, &project).unwrap();
        // PXF9 with zero versions is PXF8 plus a four-byte version count.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0..4].copy_from_slice(MAGIC_V8);
        bytes.truncate(bytes.len() - 4);
        std::fs::write(&path, &bytes).unwrap();
        let loaded = load(&path).expect("PXF8 should load");
        std::fs::remove_file(&path).unwrap();
        assert!(loaded.edit_versions.is_empty());
        assert_eq!(loaded.pixels, project.pixels);
    }

    #[test]
    fn rejects_excessive_version_count() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pixelforge_bad_versions_{}_{}.pxf", std::process::id(), unique
        ));
        let project = Project {
            pixels: image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255])),
            image_name: "test.png".into(),
            brightness: 0.0, contrast: 0.0, exposure: 0.0, saturation: 0.0,
            temperature: 0.0, tint: 0.0, highlights: 0.0, shadows: 0.0,
            whites: 0.0, blacks: 0.0, vibrance: 0.0, gamma: 0.0,
            comparison_enabled: false, comparison_position: 0.5,
            saved_selections: vec![], selection_adjustments: [0.0; 12],
            active_mask: None, selection_target: false, edit_versions: vec![],
        };
        save(&path, &project).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let offset = bytes.len() - 4;
        bytes[offset..].copy_from_slice(&((MAX_VERSIONS as u32) + 1).to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn rejects_invalid_header() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "pixelforge_invalid_{}_{}.pxf",
            std::process::id(),
            unique
        ));

        std::fs::write(&path, b"NOT_A_PROJECT").unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
