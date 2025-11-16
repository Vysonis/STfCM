use std::fs;
use std::path::Path;

use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum TleParseError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid TLE pair at line {line}")]
    InvalidPair { line: usize },
    #[error("sgp4 parse error: {0}")]
    Sgp4(#[from] sgp4::Error),
}

/// Parses a TLE file into a vector of `sgp4::Elements`.
/// Supports both 2-line and 3-line (with name) formats.
pub fn parse_tle_file_to_elements(path: &Path) -> Result<Vec<sgp4::Elements>, TleParseError> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let mut elements = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = &lines[i];
        if line.starts_with('1') {
            // Optional name on the previous line if it doesn't start with 1 or 2
            let name = if i >= 1 {
                let prev = &lines[i - 1];
                if !(prev.starts_with('1') || prev.starts_with('2')) {
                    Some(prev.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if i + 1 >= lines.len() || !lines[i + 1].starts_with('2') {
                warn!(line = i + 1, "Skipping invalid TLE pair: missing line 2");
                i += 1;
                continue;
            }

            let l1 = lines[i].clone();
            let l2 = lines[i + 1].clone();
            debug!("Parsing TLE at lines {}, {}", i + 1, i + 2);
            let elems = sgp4::Elements::from_tle(name, l1.as_bytes(), l2.as_bytes())?;
            elements.push(elems);
            i += 2;
        } else {
            // Skip non-TLE content or name lines
            i += 1;
        }
    }

    info!(count = elements.len(), "Parsed TLE elements");
    Ok(elements)
}

#[cfg(test)]
mod tests {
    use super::parse_tle_file_to_elements;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn parse_simple_tle() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "ISS (ZARYA)").unwrap();
        writeln!(
            file,
            "1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927"
        )
        .unwrap();
        writeln!(
            file,
            "2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537"
        )
        .unwrap();

        let elems = parse_tle_file_to_elements(file.path()).unwrap();
        assert_eq!(elems.len(), 1);
        assert_eq!(elems[0].norad_id, 25544);
    }
}