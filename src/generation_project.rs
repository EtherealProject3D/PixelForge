use std::{io::{Cursor, Read, Write}, path::Path};
use serde_json::{json, Value};
const MAGIC: &[u8; 4] = b"PXG1";
const MAX_GALLERY: usize = 512;
const MAX_FILE: u64 = 2_147_483_648;
pub struct GalleryImage { pub settings: Value, pub pixels: image::RgbaImage, pub seed: u64 }
pub struct GalleryProject { pub items: Vec<GalleryImage>, pub selected: usize, pub compare_index: usize, pub compare: bool, pub current: Value }
pub fn is_generation(path: &Path) -> Result<bool, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut magic = [0;4]; file.read_exact(&mut magic).map_err(|e|e.to_string())?;
    Ok(&magic == MAGIC)
}
pub fn save(path: &Path, project: &GalleryProject) -> Result<(), String> {
    if project.items.len() > MAX_GALLERY { return Err("Gallery exceeds 512 images".into()); }
    let meta = json!({"version":1,"selected":project.selected,"compare_index":project.compare_index,"compare":project.compare,"current":project.current,
        "items":project.items.iter().map(|item| json!({"settings":item.settings,"seed":item.seed})).collect::<Vec<_>>()});
    let metadata = serde_json::to_vec(&meta).map_err(|e|e.to_string())?;
    if metadata.len() > 8_000_000 { return Err("Project metadata is too large".into()); }
    let mut data = Vec::new(); data.extend_from_slice(MAGIC);
    data.extend_from_slice(&(metadata.len() as u64).to_le_bytes()); data.extend_from_slice(&metadata);
    for item in &project.items {
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(item.pixels.clone()).write_to(&mut encoded, image::ImageFormat::Png).map_err(|e|e.to_string())?;
        let png = encoded.into_inner();
        data.extend_from_slice(&(png.len() as u64).to_le_bytes()); data.extend_from_slice(&png);
        if data.len() as u64 > MAX_FILE { return Err("Project exceeds 2 GiB limit".into()); }
    }
    let mut temp = path.to_path_buf(); temp.set_extension("pxf.tmp");
    { let mut file = std::fs::File::create(&temp).map_err(|e|e.to_string())?;
      file.write_all(&data).map_err(|e|e.to_string())?; file.sync_all().map_err(|e|e.to_string())?; }
    if !path.exists() { return std::fs::rename(&temp,path).map_err(|e|e.to_string()); }
    let mut backup=path.to_path_buf(); backup.set_extension("pxf.backup");
    if backup.exists() { return Err("A previous project backup exists; move it before overwriting".into()); }
    std::fs::rename(path,&backup).map_err(|e|e.to_string())?;
    match std::fs::rename(&temp,path) {
        Ok(()) => {let _=std::fs::remove_file(&backup);Ok(())}
        Err(e) => {let _=std::fs::rename(&backup,path);Err(format!("Project replacement failed: {e}"))}
    }
}
pub fn load(path: &Path) -> Result<GalleryProject, String> {
    let file = std::fs::File::open(path).map_err(|e|e.to_string())?;
    if file.metadata().map_err(|e|e.to_string())?.len() > MAX_FILE { return Err("Project exceeds size limit".into()); }
    let mut reader = std::io::BufReader::new(file);
    let mut magic=[0;4]; reader.read_exact(&mut magic).map_err(|e|e.to_string())?;
    if &magic != MAGIC { return Err("Not a Generation Studio project".into()); }
    let mut bytes=[0;8]; reader.read_exact(&mut bytes).map_err(|e|e.to_string())?;
    let len=u64::from_le_bytes(bytes);
    if len > 8_000_000 { return Err("Invalid project metadata size".into()); }
    let mut buf=vec![0;len as usize]; reader.read_exact(&mut buf).map_err(|e|e.to_string())?;
    let meta:Value=serde_json::from_slice(&buf).map_err(|e|e.to_string())?;
    if meta["version"].as_u64()!=Some(1) { return Err("Unsupported generation project version".into()); }
    let entries=meta["items"].as_array().ok_or("Missing gallery entries")?;
    if entries.len()>MAX_GALLERY { return Err("Too many gallery entries".into()); }
    let mut items=Vec::with_capacity(entries.len());
    for entry in entries {
        reader.read_exact(&mut bytes).map_err(|e|e.to_string())?;
        let len=u64::from_le_bytes(bytes);
        if len > 150_000_000 { return Err("Invalid image size".into()); }
        let mut png=vec![0;len as usize]; reader.read_exact(&mut png).map_err(|e|e.to_string())?;
        let pixels=image::load_from_memory_with_format(&png,image::ImageFormat::Png).map_err(|e|e.to_string())?.to_rgba8();
        items.push(GalleryImage {settings:entry["settings"].clone(), seed:entry["seed"].as_u64().unwrap_or(0), pixels});
    }
    Ok(GalleryProject {items,selected:meta["selected"].as_u64().unwrap_or(0) as usize,compare_index:meta["compare_index"].as_u64().unwrap_or(0) as usize,compare:meta["compare"].as_bool().unwrap_or(false),current:meta["current"].clone()})
}
