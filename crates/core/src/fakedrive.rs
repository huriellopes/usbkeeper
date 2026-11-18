//! Pendrive falsificado emulado em memória, para testar os algoritmos sem
//! hardware. Guarda só os setores escritos, então dá para simular um
//! "128 GB" que na verdade tem 8 GB sem alocar nada disso.

use std::collections::{HashMap, HashSet};
use std::io;

use crate::blockdev::{BlockDevice, SECTOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeKind {
    /// Capacidade real = anunciada.
    Genuine,
    /// Endereços acima da capacidade real "dão a volta" (endereço % real).
    Wrap,
    /// Escritas acima da capacidade real são descartadas; leituras dão zero.
    Discard,
}

pub struct FakeDrive {
    advertised: u64,
    real: u64,
    kind: FakeKind,
    cells: HashMap<u64, Box<[u8; SECTOR]>>,
    rotten: HashSet<u64>,
}

impl FakeDrive {
    pub fn genuine(size: u64) -> Self {
        Self::new(size, size, FakeKind::Genuine)
    }

    pub fn wrapping(advertised: u64, real: u64) -> Self {
        Self::new(advertised, real, FakeKind::Wrap)
    }

    pub fn discarding(advertised: u64, real: u64) -> Self {
        Self::new(advertised, real, FakeKind::Discard)
    }

    fn new(advertised: u64, real: u64, kind: FakeKind) -> Self {
        assert!(advertised.is_multiple_of(SECTOR as u64) && real.is_multiple_of(SECTOR as u64));
        Self {
            advertised,
            real,
            kind,
            cells: HashMap::new(),
            rotten: HashSet::new(),
        }
    }

    /// Marca um setor físico como defeituoso (devolve lixo na leitura).
    pub fn with_rotten_sector(mut self, offset: u64) -> Self {
        self.rotten.insert(offset / SECTOR as u64);
        self
    }

    fn physical(&self, sector: u64) -> Option<u64> {
        let real_sectors = self.real / SECTOR as u64;
        match self.kind {
            FakeKind::Genuine => Some(sector),
            FakeKind::Wrap => Some(sector % real_sectors),
            FakeKind::Discard => (sector < real_sectors).then_some(sector),
        }
    }

    fn check(&self, offset: u64, len: usize) -> io::Result<()> {
        if !offset.is_multiple_of(SECTOR as u64) || !len.is_multiple_of(SECTOR) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "desalinhado"));
        }
        if offset + len as u64 > self.advertised {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "além do fim"));
        }
        Ok(())
    }

    /// Conteúdo físico atual, para conferir restaurações nos testes.
    pub fn physical_snapshot(&self) -> Vec<(u64, [u8; SECTOR])> {
        let mut v: Vec<_> = self.cells.iter().map(|(k, v)| (*k, **v)).collect();
        v.sort_by_key(|(k, _)| *k);
        v
    }

    /// Preenche setores físicos com conteúdo "do usuário" conhecido.
    pub fn seed_user_data(&mut self, offsets: &[u64]) {
        for &off in offsets {
            let mut cell = [0u8; SECTOR];
            for (i, b) in cell.iter_mut().enumerate() {
                *b = (off as usize + i) as u8 ^ 0x5A;
            }
            if let Some(p) = self.physical(off / SECTOR as u64) {
                self.cells.insert(p, Box::new(cell));
            }
        }
    }
}

impl BlockDevice for FakeDrive {
    fn size(&self) -> u64 {
        self.advertised
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.check(offset, buf.len())?;
        for (i, out) in buf.as_chunks_mut::<SECTOR>().0.iter_mut().enumerate() {
            let sector = offset / SECTOR as u64 + i as u64;
            match self.physical(sector) {
                Some(p) if self.rotten.contains(&p) => out.fill(0xA5),
                Some(p) => match self.cells.get(&p) {
                    Some(cell) => out.copy_from_slice(&cell[..]),
                    None => out.fill(0),
                },
                None => out.fill(0),
            }
        }
        Ok(())
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        self.check(offset, buf.len())?;
        for (i, data) in buf.as_chunks::<SECTOR>().0.iter().enumerate() {
            let sector = offset / SECTOR as u64 + i as u64;
            if let Some(p) = self.physical(sector) {
                let mut cell = Box::new([0u8; SECTOR]);
                cell.copy_from_slice(data);
                self.cells.insert(p, cell);
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_aliases_addresses() {
        let mut d = FakeDrive::wrapping(4096, 1024);
        d.write_at(2048, &[7u8; SECTOR]).unwrap();
        let mut out = [0u8; SECTOR];
        d.read_at(0, &mut out).unwrap();
        assert_eq!(out, [7u8; SECTOR]);
    }

    #[test]
    fn discard_drops_writes_beyond_real() {
        let mut d = FakeDrive::discarding(4096, 1024);
        d.write_at(2048, &[7u8; SECTOR]).unwrap();
        let mut out = [1u8; SECTOR];
        d.read_at(2048, &mut out).unwrap();
        assert_eq!(out, [0u8; SECTOR]);
    }
}
