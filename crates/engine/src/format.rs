use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    Jpeg,
    Png,
    WebP,
    Avif,
}

impl Format {
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" => Some(Format::Jpeg),
            "png" => Some(Format::Png),
            "webp" => Some(Format::WebP),
            "avif" => Some(Format::Avif),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::WebP => "webp",
            Format::Avif => "avif",
        }
    }

    pub fn is_lossless(self) -> bool {
        matches!(self, Format::Png)
    }

    pub fn supports_alpha(self) -> bool {
        matches!(self, Format::Png | Format::WebP | Format::Avif)
    }
}

impl FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Ok(Format::Jpeg),
            "png" => Ok(Format::Png),
            "webp" => Ok(Format::WebP),
            "avif" => Ok(Format::Avif),
            other => Err(format!("unsupported format: {other}")),
        }
    }
}
