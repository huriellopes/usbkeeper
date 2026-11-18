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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_decimal_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1_500), "1.5 KB");
        assert_eq!(human_bytes(15_376_000_000), "15.4 GB");
        assert_eq!(human_bytes(1_000_204_886_016), "1.0 TB");
    }
}
