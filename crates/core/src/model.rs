use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Default)]
pub struct Cmyk {
    pub c: u8,
    pub m: u8,
    pub y: u8,
    pub k: u8,
}

impl fmt::Display for Cmyk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}, {}, {}", self.c, self.m, self.y, self.k)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pantone {
    pub number: String,
    pub rgb_r: u8,
    pub rgb_g: u8,
    pub rgb_b: u8,
    pub weight_kg: f64,
}

impl Pantone {
    pub fn new(number: String, rgb_r: u8, rgb_g: u8, rgb_b: u8, weight_kg: f64) -> Self {
        Self {
            number,
            rgb_r,
            rgb_g,
            rgb_b,
            weight_kg,
        }
    }

    pub fn html(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.rgb_r, self.rgb_g, self.rgb_b)
    }

    pub fn rgb_string(&self) -> String {
        format!("{},{},{}", self.rgb_r, self.rgb_g, self.rgb_b)
    }

    pub fn cmyk(&self) -> Cmyk {
        cmyk_from_rgb(self.rgb_r, self.rgb_g, self.rgb_b)
    }
}

pub fn cmyk_from_rgb(r: u8, g: u8, b: u8) -> Cmyk {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let k = 1.0 - rf.max(gf).max(bf);
    let denom = 1.0 - k;
    let (c, m, y) = if denom <= 0.0 {
        (0.0, 0.0, 0.0)
    } else {
        (
            (1.0 - rf - k) / denom,
            (1.0 - gf - k) / denom,
            (1.0 - bf - k) / denom,
        )
    };
    Cmyk {
        c: (c * 100.0).round() as u8,
        m: (m * 100.0).round() as u8,
        y: (y * 100.0).round() as u8,
        k: (k * 100.0).round() as u8,
    }
}

pub fn format_weight(kg: f64) -> String {
    format!("{kg} кг")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_derived_from_rgb() {
        let p = Pantone::new("PANTONE 7622".into(), 157, 48, 43, 20.0);
        assert_eq!(p.html(), "#9D302B");
        assert_eq!(p.rgb_string(), "157,48,43");
    }

    #[test]
    fn cmyk_derived_from_rgb() {
        let cmyk = cmyk_from_rgb(255, 0, 0);
        assert_eq!(cmyk.c, 0);
        assert_eq!(cmyk.m, 100);
        assert_eq!(cmyk.y, 100);
        assert_eq!(cmyk.k, 0);

        let cmyk = cmyk_from_rgb(0, 0, 0);
        assert_eq!((cmyk.c, cmyk.m, cmyk.y, cmyk.k), (0, 0, 0, 100));

        let cmyk = cmyk_from_rgb(255, 255, 255);
        assert_eq!((cmyk.c, cmyk.m, cmyk.y, cmyk.k), (0, 0, 0, 0));
    }

    #[test]
    fn weight_formatte() {
        assert_eq!(format_weight(20.0), "20 кг");
        assert_eq!(format_weight(0.5), "0.5 кг");
    }
}
