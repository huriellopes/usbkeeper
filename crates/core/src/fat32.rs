//! Formatador FAT32 próprio, sobre [`BlockDevice`].
//!
//! Existe porque o Windows (`Format-Volume`, `format.com`) recusa FAT32
//! acima de 32 GB, embora o formato vá até 2 TiB e seja o que TVs, carros e
//! videogames leem. Segue a especificação FAT da Microsoft (fatgen103).

use std::sync::atomic::AtomicBool;

use crate::CoreError;
use crate::blockdev::{AlignedBuf, BlockDevice, Phase, Progress, cancelled};

const RESERVED_MIN: u32 = 32;
const NUM_FATS: u32 = 2;
const ROOT_CLUSTER: u32 = 2;
const MIN_CLUSTERS: u64 = 65_525;
const MAX_CLUSTERS: u64 = 0x0FFF_FFF5;
const MEDIA: u8 = 0xF8;
const CHUNK: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fat32Layout {
    pub sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub fat_sectors: u32,
    pub total_sectors: u32,
    pub clusters: u32,
}

impl Fat32Layout {
    pub fn cluster_bytes(&self) -> u32 {
        self.sector * self.sectors_per_cluster
    }

    /// Primeiro setor da área de dados (cluster 2), relativo ao volume.
    pub fn data_start(&self) -> u32 {
        self.reserved_sectors + NUM_FATS * self.fat_sectors
    }

    /// Tamanho do cluster pela tabela padrão do Windows para FAT32.
    fn cluster_size(len: u64) -> u64 {
        const MIB: u64 = 1 << 20;
        const GIB: u64 = 1 << 30;
        match len {
            l if l <= 64 * MIB => 512,
            l if l <= 128 * MIB => 1024,
            l if l <= 256 * MIB => 2048,
            l if l <= 8 * GIB => 4096,
            l if l <= 16 * GIB => 8192,
            l if l <= 32 * GIB => 16384,
            _ => 32768,
        }
    }

    pub fn compute(len_bytes: u64, sector: u32) -> Result<Self, String> {
        if !matches!(sector, 512 | 1024 | 2048 | 4096) {
            return Err(format!(
                "tamanho de setor não suportado pelo FAT32: {sector}"
            ));
        }
        let total = len_bytes / sector as u64;
        if total > u32::MAX as u64 {
            return Err("grande demais para FAT32 (limite de 2 TiB com setor de 512 bytes)".into());
        }
        let spc = (Self::cluster_size(len_bytes).max(sector as u64) / sector as u64) as u32;
        let entries_per_sector = (sector / 4) as u64;
        let mut reserved = RESERVED_MIN;
        let mut fat = 1u32;
        // Itera até a FAT caber os clusters e a área de dados (reservados +
        // FATs) ficar alinhada ao cluster; a FAT só cresce, então converge.
        for _ in 0..64 {
            let data = (total as i64) - (reserved + NUM_FATS * fat) as i64;
            if data <= 0 {
                return Err("pequeno demais para FAT32".into());
            }
            let clusters = data as u64 / spc as u64;
            let needed = ((clusters + 2).div_ceil(entries_per_sector) as u32).max(fat);
            let pad = (spc - (RESERVED_MIN + NUM_FATS * needed) % spc) % spc;
            if needed == fat && reserved == RESERVED_MIN + pad {
                if clusters < MIN_CLUSTERS {
                    return Err(format!(
                        "pequeno demais para FAT32 ({clusters} clusters; o mínimo é {MIN_CLUSTERS}): use exFAT"
                    ));
                }
                if clusters > MAX_CLUSTERS {
                    return Err("clusters demais para FAT32".into());
                }
                return Ok(Self {
                    sector,
                    sectors_per_cluster: spc,
                    reserved_sectors: reserved,
                    fat_sectors: fat,
                    total_sectors: total as u32,
                    clusters: clusters as u32,
                });
            }
            fat = needed;
            reserved = RESERVED_MIN + pad;
        }
        Err("não consegui calcular o layout do FAT32".into())
    }
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn label_bytes(label: Option<&str>) -> [u8; 11] {
    let mut out = *b"NO NAME    ";
    if let Some(l) = label.filter(|l| !l.is_empty()) {
        out = [b' '; 11];
        for (i, c) in l.bytes().take(11).enumerate() {
            out[i] = c;
        }
    }
    out
}

/// Setor de boot (BPB do FAT32) de um setor inteiro.
pub fn boot_sector(
    l: &Fat32Layout,
    hidden_sectors: u32,
    label: Option<&str>,
    volume_id: u32,
) -> Vec<u8> {
    let mut b = vec![0u8; l.sector as usize];
    b[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
    b[3..11].copy_from_slice(b"MSWIN4.1");
    put16(&mut b, 11, l.sector as u16);
    b[13] = l.sectors_per_cluster as u8;
    put16(&mut b, 14, l.reserved_sectors as u16);
    b[16] = NUM_FATS as u8;
    b[21] = MEDIA;
    put16(&mut b, 24, 63);
    put16(&mut b, 26, 255);
    put32(&mut b, 28, hidden_sectors);
    put32(&mut b, 32, l.total_sectors);
    put32(&mut b, 36, l.fat_sectors);
    put32(&mut b, 44, ROOT_CLUSTER);
    put16(&mut b, 48, 1); // FSInfo
    put16(&mut b, 50, 6); // cópia do setor de boot
    b[64] = 0x80;
    b[66] = 0x29;
    put32(&mut b, 67, volume_id);
    b[71..82].copy_from_slice(&label_bytes(label));
    b[82..90].copy_from_slice(b"FAT32   ");
    // Não inicializável: cli; hlt; jmp $-1
    b[90..94].copy_from_slice(&[0xFA, 0xF4, 0xEB, 0xFD]);
    b[510] = 0x55;
    b[511] = 0xAA;
    b
}

/// Setor FSInfo (contagem de clusters livres e próxima posição livre).
pub fn fsinfo_sector(l: &Fat32Layout) -> Vec<u8> {
    let mut b = vec![0u8; l.sector as usize];
    put32(&mut b, 0, 0x4161_5252);
    put32(&mut b, 484, 0x6141_7272);
    put32(&mut b, 488, l.clusters - 1); // a raiz ocupa um cluster
    put32(&mut b, 492, ROOT_CLUSTER + 1);
    b[510] = 0x55;
    b[511] = 0xAA;
    b
}

/// Formata `len` bytes a partir de `offset` (início da partição). O setor
/// de boot é gravado por último: um formato interrompido não parece válido.
pub fn format(
    dev: &mut dyn BlockDevice,
    offset: u64,
    len: u64,
    label: Option<&str>,
    volume_id: u32,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<Fat32Layout, CoreError> {
    let l = Fat32Layout::compute(len, 512).map_err(CoreError::Backend)?;
    let sec = l.sector as u64;
    if !offset.is_multiple_of(sec) || offset + len > dev.size() {
        return Err(CoreError::Backend(
            "partição fora do disco ou desalinhada".into(),
        ));
    }
    let hidden = u32::try_from(offset / sec).unwrap_or(u32::MAX);

    // Zera área reservada + FATs + cluster da raiz, em blocos.
    let zero_end = (l.data_start() as u64 + l.sectors_per_cluster as u64) * sec;
    let buf = AlignedBuf::new(CHUNK);
    let mut done = 0u64;
    while done < zero_end {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        let n = (zero_end - done).min(CHUNK as u64) as usize;
        dev.write_at(offset + done, &buf[..n])?;
        done += n as u64;
        progress.report(Phase::Write, done, zero_end);
    }

    // Início de cada FAT: mídia, marcador e fim de cadeia da raiz.
    let mut first = AlignedBuf::new(l.sector as usize);
    put32(&mut first, 0, 0x0FFF_FF00 | MEDIA as u32);
    put32(&mut first, 4, 0x0FFF_FFFF);
    put32(&mut first, 8, 0x0FFF_FFFF);
    for i in 0..NUM_FATS {
        let at = (l.reserved_sectors + i * l.fat_sectors) as u64 * sec;
        dev.write_at(offset + at, &first)?;
    }

    // Rótulo como entrada da raiz.
    if let Some(name) = label.filter(|s| !s.is_empty()) {
        let mut root = AlignedBuf::new(l.sector as usize);
        root[0..11].copy_from_slice(&label_bytes(Some(name)));
        root[11] = 0x08;
        dev.write_at(offset + l.data_start() as u64 * sec, &root)?;
    }

    // FSInfo e setor de boot, com as cópias nos setores 7 e 6; boot por último.
    let mut s = AlignedBuf::new(l.sector as usize);
    s.copy_from_slice(&fsinfo_sector(&l));
    dev.write_at(offset + sec, &s)?;
    dev.write_at(offset + 7 * sec, &s)?;
    s.copy_from_slice(&boot_sector(&l, hidden, label, volume_id));
    dev.write_at(offset + 6 * sec, &s)?;
    dev.write_at(offset, &s)?;
    dev.flush()?;
    Ok(l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::Silent;
    use crate::fakedrive::FakeDrive;

    const GIB: u64 = 1 << 30;

    #[test]
    fn layouts_follow_windows_cluster_table() {
        let l = Fat32Layout::compute(256 << 20, 512).unwrap();
        assert_eq!(l.cluster_bytes(), 2048);
        let l = Fat32Layout::compute(31 * GIB, 512).unwrap();
        assert_eq!(l.cluster_bytes(), 16384);
        let l = Fat32Layout::compute(64 * GIB, 512).unwrap();
        assert_eq!(l.cluster_bytes(), 32768);
        for len in [256 << 20, 4 * GIB, 31 * GIB, 64 * GIB, 1900 * GIB] {
            let l = Fat32Layout::compute(len, 512).unwrap();
            // A FAT cobre todos os clusters (+2 reservados) e a área de dados é alinhada.
            assert!(l.fat_sectors as u64 * 128 >= l.clusters as u64 + 2, "{len}");
            assert_eq!(l.data_start() % l.sectors_per_cluster, 0, "{len}");
            assert!(l.reserved_sectors >= 32);
            let used = l.data_start() as u64 + l.clusters as u64 * l.sectors_per_cluster as u64;
            assert!(used <= l.total_sectors as u64);
        }
    }

    #[test]
    fn refuses_too_small_and_too_big() {
        assert!(
            Fat32Layout::compute(16 << 20, 512)
                .unwrap_err()
                .contains("exFAT")
        );
        assert!(Fat32Layout::compute(3 << 40, 512).is_err());
        assert!(Fat32Layout::compute(GIB, 520).is_err());
    }

    #[test]
    fn boot_sector_fields() {
        let l = Fat32Layout::compute(64 * GIB, 512).unwrap();
        let b = boot_sector(&l, 2048, Some("FOTOS"), 0x1234_5678);
        assert_eq!(&b[510..], &[0x55, 0xAA]);
        assert_eq!(
            u32::from_le_bytes(b[28..32].try_into().unwrap()),
            2048,
            "hidden = offset/setor"
        );
        assert_eq!(
            u32::from_le_bytes(b[36..40].try_into().unwrap()),
            l.fat_sectors
        );
        assert_eq!(&b[71..82], b"FOTOS      ");
        assert_eq!(&b[82..90], b"FAT32   ");
        assert_eq!(&boot_sector(&l, 0, None, 1)[71..82], b"NO NAME    ");
    }

    #[test]
    fn format_writes_boot_last_and_respects_offset() {
        let mut d = FakeDrive::genuine(300 << 20);
        let l = format(
            &mut d,
            1 << 20,
            256 << 20,
            Some("PEN"),
            7,
            &mut Silent,
            &AtomicBool::new(false),
        )
        .unwrap();
        let mut s = AlignedBuf::new(512);
        d.read_at(1 << 20, &mut s).unwrap();
        assert_eq!(&s[82..90], b"FAT32   ");
        d.read_at((1 << 20) + 6 * 512, &mut s).unwrap();
        assert_eq!(&s[82..90], b"FAT32   ", "cópia no setor 6");
        d.read_at((1 << 20) + l.reserved_sectors as u64 * 512, &mut s)
            .unwrap();
        assert_eq!(u32::from_le_bytes(s[0..4].try_into().unwrap()), 0x0FFF_FFF8);
        let mut zero = AlignedBuf::new(512);
        d.read_at(0, &mut zero).unwrap();
        assert!(zero.iter().all(|&b| b == 0), "nada antes do offset");
    }

    #[test]
    fn cancel_leaves_no_boot_sector() {
        let mut d = FakeDrive::genuine(300 << 20);
        let err = format(
            &mut d,
            0,
            256 << 20,
            None,
            1,
            &mut Silent,
            &AtomicBool::new(true),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
        let mut s = AlignedBuf::new(512);
        d.read_at(0, &mut s).unwrap();
        assert_ne!(&s[510..], &[0x55, 0xAA]);
    }

    /// Imagem formatada passa no fsck.fat e o rótulo é lido pelo fatlabel.
    #[cfg(unix)]
    #[test]
    fn dosfstools_accept_the_result() {
        use std::os::unix::fs::FileExt;
        use std::process::Command;
        struct FileDev(std::fs::File, u64);
        impl BlockDevice for FileDev {
            fn size(&self) -> u64 {
                self.1
            }
            fn read_at(&mut self, o: u64, b: &mut [u8]) -> std::io::Result<()> {
                self.0.read_exact_at(b, o)
            }
            fn write_at(&mut self, o: u64, b: &[u8]) -> std::io::Result<()> {
                self.0.write_all_at(b, o)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.sync_all()
            }
        }
        let have = |t: &str| Command::new(t).arg("--help").output().is_ok();
        if !have("fsck.fat") || !have("fatlabel") {
            eprintln!("dosfstools ausente: pulando");
            return;
        }
        let dir =
            tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).unwrap();
        // 256 MiB e 40 GiB (esparso): o Windows recusaria este último.
        for (len, label) in [(256u64 << 20, "PEQUENO"), (40 * GIB, "GRANDE 40G")] {
            let path = dir.path().join(format!("fat-{len}.img"));
            let f = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            f.set_len(len).unwrap();
            let mut dev = FileDev(f, len);
            format(
                &mut dev,
                0,
                len,
                Some(label),
                0xCAFE_F00D,
                &mut Silent,
                &AtomicBool::new(false),
            )
            .unwrap();
            let fsck = Command::new("fsck.fat")
                .args(["-n", "-v"])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                fsck.status.success(),
                "fsck.fat {len}: {}{}",
                String::from_utf8_lossy(&fsck.stdout),
                String::from_utf8_lossy(&fsck.stderr)
            );
            let out = Command::new("fatlabel").arg(&path).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), label, "{len}");
        }
    }
}
