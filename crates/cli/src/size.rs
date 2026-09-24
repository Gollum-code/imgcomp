pub fn parse_size(s: &str) -> Option<u64> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let (num_str, mult) = split_number_suffix(&t);
    let val: f64 = num_str.trim().parse().ok()?;
    if val < 0.0 {
        return None;
    }
    let bytes = val * mult;
    Some(bytes as u64)
}

fn split_number_suffix(t: &str) -> (&str, f64) {
    let suffix_pos = t
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit() && *c != '.' && *c != ',' && *c != ' ')
        .map(|(i, _)| i)
        .unwrap_or(t.len());
    let (num, suffix) = t.split_at(suffix_pos);
    let suffix = suffix.trim().to_ascii_lowercase();
    let base = match suffix.as_str() {
        "k" | "kb" | "kib" | "kilobyte" | "kilobytes" => 1024.0,
        "m" | "mb" | "mib" | "megabyte" | "megabytes" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" | "gigabyte" | "gigabytes" => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    (num, base)
}

pub fn human_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.2} MB", n as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("500kb"), Some(500 * 1024));
        assert_eq!(parse_size("1.5MB"), Some((1.5 * 1024.0 * 1024.0) as u64));
        assert_eq!(parse_size("2 mib"), Some(2 * 1024 * 1024));
        assert_eq!(parse_size("100"), Some(100));
        assert_eq!(parse_size("300KB"), Some(300 * 1024));
        assert_eq!(parse_size("1gb"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size(""), None);
    }
}
