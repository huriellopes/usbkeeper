//! Caminhos e estruturas do Windows interpretados sem chamar o Windows
//! (testados em todo SO).

/// `\\.\PhysicalDrive3` → 3.
pub fn disk_number(path: &str) -> Option<u32> {
    let lower = path.trim().to_ascii_lowercase();
    let n = lower.strip_prefix(r"\\.\physicaldrive")?;
    (!n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
}

/// Números de disco em `VOLUME_DISK_EXTENTS` (resposta de
/// `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`): `u32` com a quantidade, 4 bytes de
/// alinhamento e então `DISK_EXTENT`s de 24 bytes (`DiskNumber: u32`, 4 de
/// alinhamento, `StartingOffset: i64`, `ExtentLength: i64`).
pub fn extent_disks(buf: &[u8]) -> Vec<u32> {
    const HEADER: usize = 8;
    const EXTENT: usize = 24;
    let Some(count) = buf
        .get(..4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    else {
        return vec![];
    };
    (0..count as usize)
        .map_while(|i| {
            let at = HEADER + i * EXTENT;
            buf.get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        })
        .collect()
}

/// `\\?\Volume{…}\` → `\\?\Volume{…}` (o volume abre como dispositivo sem a barra final).
pub fn volume_device(volume_name: &str) -> &str {
    volume_name.trim_end_matches('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_numbers() {
        assert_eq!(disk_number(r"\\.\PhysicalDrive3"), Some(3));
        assert_eq!(disk_number(r"\\.\physicaldrive12"), Some(12));
        assert_eq!(disk_number(r"\\.\PhysicalDrive"), None);
        assert_eq!(disk_number(r"\\.\PhysicalDrive1x"), None);
        assert_eq!(disk_number("/dev/sdb"), None);
        assert_eq!(disk_number(r"C:\imagem.img"), None);
    }

    fn extents(disks: &[u32]) -> Vec<u8> {
        let mut b = (disks.len() as u32).to_le_bytes().to_vec();
        b.extend([0; 4]);
        for d in disks {
            b.extend(d.to_le_bytes());
            b.extend([0; 4]);
            b.extend(1_048_576i64.to_le_bytes());
            b.extend(268_435_456i64.to_le_bytes());
        }
        b
    }

    #[test]
    fn parses_disk_extents() {
        assert_eq!(extent_disks(&extents(&[2])), vec![2]);
        assert_eq!(
            extent_disks(&extents(&[0, 3])),
            vec![0, 3],
            "volume que ocupa dois discos"
        );
        assert!(extent_disks(&[]).is_empty());
        let mut cut = extents(&[1, 2]);
        cut.truncate(8 + 24 + 2);
        assert_eq!(extent_disks(&cut), vec![1], "buffer curto não passa do fim");
    }

    #[test]
    fn volume_device_path() {
        assert_eq!(volume_device(r"\\?\Volume{abc}\"), r"\\?\Volume{abc}");
        assert_eq!(volume_device(r"\\?\Volume{abc}"), r"\\?\Volume{abc}");
    }
}
