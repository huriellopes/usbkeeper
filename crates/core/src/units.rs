/// Formata bytes em unidades decimais (as mesmas impressas na embalagem do
/// pendrive), o que facilita comparar com a capacidade anunciada.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Lê um tamanho como "7800MiB", "7.8G", "7800M", "8GB" ou "8000000000".
/// Sufixos decimais (K/M/G/T, com ou sem B) seguem a embalagem do pendrive;
/// KiB/MiB/GiB/TiB são binários.
pub fn parse_size(text: &str) -> Result<u64, String> {
    let t = text.trim();
    let split = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(t.len());
    let (number, unit) = t.split_at(split);
    let value: f64 = number
        .parse()
        .map_err(|_| format!("tamanho inválido: {text:?} (ex.: 7800MiB, 7.8G)"))?;
    let factor: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kb" => 1e3,
        "m" | "mb" => 1e6,
        "g" | "gb" => 1e9,
        "t" | "tb" => 1e12,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        other => {
            return Err(format!(
                "unidade desconhecida: {other:?} (use M, G, MiB, GiB...)"
            ));
        }
    };
    let bytes = value * factor;
    if !bytes.is_finite() || bytes < 1.0 || bytes > u64::MAX as f64 {
        return Err(format!("tamanho fora do intervalo: {text:?}"));
    }
    Ok(bytes as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("7800MiB"), Ok(7800 << 20));
        assert_eq!(parse_size("7.8G"), Ok(7_800_000_000));
        assert_eq!(parse_size("8gb"), Ok(8_000_000_000));
        assert_eq!(parse_size(" 512 "), Ok(512));
        assert_eq!(parse_size("1.5KiB"), Ok(1536));
        assert!(parse_size("muito").is_err());
        assert!(parse_size("8 XB").is_err());
        assert!(parse_size("0").is_err());
    }

    #[test]
    fn formats_decimal_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1_500), "1.5 KB");
        assert_eq!(human_bytes(15_376_000_000), "15.4 GB");
        assert_eq!(human_bytes(1_000_204_886_016), "1.0 TB");
    }
}
