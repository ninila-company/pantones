use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use csv::StringRecord;

use crate::model::Pantone;

fn recent_file_path() -> PathBuf {
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|p| !p.is_empty());
    match base {
        Some(home) => PathBuf::from(home)
            .join(".config")
            .join("pantones")
            .join("recent"),
        _ => PathBuf::from(".pantones_recent"),
    }
}

pub fn load_recent_paths() -> Vec<String> {
    let Ok(text) = fs::read_to_string(recent_file_path()) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.to_string()))
        .map(str::to_string)
        .collect()
}

pub fn save_recent_paths(paths: &[String]) -> Result<(), String> {
    let file = recent_file_path();
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = paths.join("\n");
    fs::write(&file, contents).map_err(|e| e.to_string())
}

pub struct Storage {
    pub path: String,
}

impl Storage {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self) -> Result<Vec<Pantone>, String> {
        if !Path::new(&self.path).exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&self.path).map_err(|e| e.to_string())?;
        if file.metadata().map(|m| m.len() == 0).unwrap_or(false) {
            return Ok(Vec::new());
        }
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(BufReader::new(file));
        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(str::to_string)
            .collect();
        let records: Vec<csv::StringRecord> = reader
            .records()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("CSV parse error: {e}"))?;

        let migrated = !headers.iter().any(|h| h == "number");
        let pantones = if migrated {
            Self::migrate_legacy(&headers, &records)
        } else {
            Self::parse_current(&records)
        };
        if migrated {
            self.save(&pantones)?;
        }
        Ok(pantones)
    }

    fn parse_current(records: &[csv::StringRecord]) -> Vec<Pantone> {
        records
            .iter()
            .filter_map(|r| {
                let number = r.get(0).unwrap_or("").trim().to_string();
                if number.is_empty() {
                    return None;
                }
                let parse_u8 = |i: usize| r.get(i).and_then(|v| v.trim().parse::<u8>().ok());
                let weight = r
                    .get(4)
                    .and_then(|v| v.trim().replace(',', ".").parse::<f64>().ok())
                    .unwrap_or(0.0);
                Some(Pantone::new(
                    number,
                    parse_u8(1).unwrap_or(0),
                    parse_u8(2).unwrap_or(0),
                    parse_u8(3).unwrap_or(0),
                    weight,
                ))
            })
            .collect()
    }

    fn migrate_legacy(headers: &[String], records: &[csv::StringRecord]) -> Vec<Pantone> {
        let col_code = headers.iter().position(|h| h == "code");
        let col_w_kg = headers.iter().position(|h| h == "weight_kg");
        let col_w_g = headers.iter().position(|h| h == "weight_g");

        records
            .iter()
            .filter_map(|r| {
                let number = col_code
                    .and_then(|i| r.get(i))
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if number.is_empty() {
                    return None;
                }
                let mut weight = 0.0;
                if let Some(i) = col_w_kg {
                    if let Some(raw) = r.get(i) {
                        weight = raw.trim().replace(',', ".").parse::<f64>().unwrap_or(0.0);
                    }
                } else if let Some(i) = col_w_g {
                    if let Some(raw) = r.get(i) {
                        weight =
                            raw.trim().replace(',', ".").parse::<f64>().unwrap_or(0.0) / 1000.0;
                    }
                }
                Some(Pantone::new(number, 0, 0, 0, weight))
            })
            .collect()
    }

    pub fn save(&self, pantones: &[Pantone]) -> Result<(), String> {
        if let Some(parent) = Path::new(&self.path).parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = File::create(&self.path).map_err(|e| e.to_string())?;
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(BufWriter::new(file));
        writer
            .write_record(&StringRecord::from_iter([
                "number",
                "rgb_r",
                "rgb_g",
                "rgb_b",
                "weight_kg",
            ]))
            .map_err(|e| e.to_string())?;
        for p in pantones {
            writer.serialize(p).map_err(|e| e.to_string())?;
        }
        writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pantone;

    #[test]
    fn round_trip_preserves_data() {
        let dir = std::env::temp_dir().join(format!("pantones_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stock.csv");
        let path_str = path.to_str().unwrap().to_string();

        let storage = Storage::new(&path_str);
        let items = vec![
            Pantone::new("PANTONE 7622".into(), 157, 48, 43, 20.0),
            Pantone::new("PANTONE 19-4052".into(), 15, 76, 129, 0.5),
        ];
        storage.save(&items).unwrap();
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].number, "PANTONE 7622");
        assert_eq!(loaded[0].rgb_r, 157);
        assert_eq!(loaded[0].rgb_g, 48);
        assert_eq!(loaded[0].rgb_b, 43);
        assert_eq!(loaded[0].weight_kg, 20.0);
        assert_eq!(loaded[0].html(), "#9D302B");
        assert_eq!(loaded[1].number, "PANTONE 19-4052");
        assert_eq!(loaded[1].weight_kg, 0.5);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let storage = Storage::new("/nonexistent/path/stock.csv");
        assert_eq!(storage.load().unwrap().len(), 0);
    }

    #[test]
    fn migrates_legacy_kg_schema() {
        let dir = std::env::temp_dir().join(format!("pantones_mig_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy.csv");
        let path_str = path.to_str().unwrap().to_string();

        std::fs::write(&path, "code,name,weight_kg\n7623c,ninila,20.0\n").unwrap();
        let storage = Storage::new(&path_str);
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].number, "7623c");
        assert_eq!(loaded[0].weight_kg, 20.0);
        assert_eq!(
            (loaded[0].rgb_r, loaded[0].rgb_g, loaded[0].rgb_b),
            (0, 0, 0)
        );

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with("number,rgb_r,rgb_g,rgb_b,weight_kg"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migrates_legacy_grams_schema() {
        let dir = std::env::temp_dir().join(format!("pantones_mig_g_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy.csv");
        let path_str = path.to_str().unwrap().to_string();

        std::fs::write(&path, "code,name,weight_g\n7623c,ninila,20000\n").unwrap();
        let loaded = Storage::new(&path_str).load().unwrap();
        assert_eq!(loaded[0].weight_kg, 20.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recent_paths_round_trip() {
        let dir = std::env::temp_dir().join(format!("pantones_rec_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("HOME", &dir) };

        let cur = recent_file_path();
        assert!(!cur.exists());
        let paths = vec!["/tmp/a.csv".to_string(), "/tmp/b.csv".to_string()];
        save_recent_paths(&paths).unwrap();
        assert_eq!(load_recent_paths(), paths);

        let _ = std::fs::remove_file(&cur);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
