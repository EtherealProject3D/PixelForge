use std::io::{Cursor, Read, Write};
use std::path::Path;

const MAGIC: &[u8; 4] = b"PXF1";
const MAX_BYTES: u64 = 1_073_741_824;
const MAX_NAME: usize = 4096;

pub struct Project {
    pub pixels: image::RgbaImage,
    pub image_name: String,
    pub brightness: f32,
    pub contrast: f32,
    pub comparison_enabled: bool,
    pub comparison_position: f32,
}

fn read<const N: usize>(reader: &mut impl Read) -> Result<[u8; N], String> {
    let mut value = [0u8; N];
    reader.read_exact(&mut value).map_err(|e| e.to_string())?;
    Ok(value)
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
    data.extend_from_slice(MAGIC);
    data.extend_from_slice(&project.brightness.to_le_bytes());
    data.extend_from_slice(&project.contrast.to_le_bytes());
    data.push(u8::from(project.comparison_enabled));
    data.extend_from_slice(&project.comparison_position.to_le_bytes());
    data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    data.extend_from_slice(name);
    data.extend_from_slice(&(png.len() as u64).to_le_bytes());
    data.extend_from_slice(&png);

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

    if &read::<4>(&mut reader)? != MAGIC {
        return Err("Unsupported PixelForge project format".into());
    }

    let brightness = f32::from_le_bytes(read::<4>(&mut reader)?);
    let contrast = f32::from_le_bytes(read::<4>(&mut reader)?);
    let enabled = read::<1>(&mut reader)?[0];
    let comparison_position = f32::from_le_bytes(read::<4>(&mut reader)?);

    if !brightness.is_finite()
        || !contrast.is_finite()
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
        || image_length != bytes.len() as u64 - reader.position()
    {
        return Err("Invalid or incomplete embedded image".into());
    }

    let start = reader.position() as usize;

    let pixels = image::load_from_memory_with_format(&bytes[start..], image::ImageFormat::Png)
        .map_err(|e| e.to_string())?
        .to_rgba8();

    Ok(Project {
        pixels,
        image_name,
        brightness,
        contrast,
        comparison_enabled: enabled == 1,
        comparison_position,
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
            comparison_enabled: true,
            comparison_position: 0.3,
        };

        save(&path, &original).expect("PXF save failed");
        let loaded = load(&path).expect("PXF load failed");
        std::fs::remove_file(&path).unwrap();

        assert_eq!(loaded.pixels, original.pixels);
        assert_eq!(loaded.image_name, original.image_name);
        assert_eq!(loaded.brightness, original.brightness);
        assert_eq!(loaded.contrast, original.contrast);
        assert_eq!(loaded.comparison_enabled, original.comparison_enabled);
        assert_eq!(loaded.comparison_position, original.comparison_position);
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
