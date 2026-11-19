//! Acesso bruto a um dispositivo de blocos, independente de plataforma.

use std::alloc::{self, Layout};
use std::io;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

pub const SECTOR: usize = 512;
/// Alinhamento exigido por O_DIRECT / FILE_FLAG_NO_BUFFERING na prática.
pub const ALIGN: usize = 4096;

/// Leitura/escrita em offsets absolutos. Offsets e tamanhos são múltiplos de
/// [`SECTOR`]; quem implementa pode exigir buffers alinhados (use [`AlignedBuf`]).
pub trait BlockDevice {
    fn size(&self) -> u64;
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()>;
    /// Garante que o que foi escrito chegou ao dispositivo e que as próximas
    /// leituras não virão de cache do sistema.
    fn flush(&mut self) -> io::Result<()>;
}

/// Buffer com alinhamento de [`ALIGN`] bytes, zerado.
pub struct AlignedBuf {
    ptr: *mut u8,
    len: usize,
}

impl AlignedBuf {
    pub fn new(len: usize) -> Self {
        assert!(
            len > 0 && len.is_multiple_of(SECTOR),
            "tamanho deve ser múltiplo de 512"
        );
        let layout = Layout::from_size_align(len, ALIGN).expect("layout válido");
        // SAFETY: layout tem tamanho > 0.
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }
        Self { ptr, len }
    }
}

impl Deref for AlignedBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        // SAFETY: ptr aponta para `len` bytes alocados e inicializados.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for AlignedBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: idem, e temos &mut self.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.len, ALIGN).expect("layout válido");
        // SAFETY: alocado em `new` com o mesmo layout.
        unsafe { alloc::dealloc(self.ptr, layout) }
    }
}

// SAFETY: o buffer é dono exclusivo da memória.
unsafe impl Send for AlignedBuf {}

/// Etapas reportadas pelos testes longos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Backup,
    Probe,
    Write,
    Verify,
    Refine,
    Restore,
    SeqRead,
    RandRead,
    SeqWrite,
    RandWrite,
    Scan,
}

impl Phase {
    /// Identificador estável (snake_case) usado no protocolo `--progress-json`.
    pub fn id(self) -> &'static str {
        match self {
            Phase::Backup => "backup",
            Phase::Probe => "probe",
            Phase::Write => "write",
            Phase::Verify => "verify",
            Phase::Refine => "refine",
            Phase::Restore => "restore",
            Phase::SeqRead => "seq_read",
            Phase::RandRead => "rand_read",
            Phase::SeqWrite => "seq_write",
            Phase::RandWrite => "rand_write",
            Phase::Scan => "scan",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Backup => "salvando blocos originais",
            Phase::Probe => "gravando blocos de teste",
            Phase::Write => "gravando",
            Phase::Verify => "verificando",
            Phase::Refine => "refinando limite",
            Phase::Restore => "restaurando blocos originais",
            Phase::SeqRead => "leitura sequencial",
            Phase::RandRead => "leitura aleatória 4K",
            Phase::SeqWrite => "escrita sequencial",
            Phase::RandWrite => "escrita aleatória 4K",
            Phase::Scan => "lendo a superfície",
        }
    }
}

impl Phase {
    /// Etapas medidas em bytes (as outras contam blocos ou operações).
    pub fn counts_bytes(self) -> bool {
        matches!(
            self,
            Phase::Write | Phase::Verify | Phase::SeqRead | Phase::SeqWrite | Phase::Scan
        )
    }
}

pub trait Progress {
    /// `done`/`total` na unidade da etapa (bytes ou operações).
    fn report(&mut self, phase: Phase, done: u64, total: u64);
}

/// Progresso que não mostra nada (testes, saída JSON).
pub struct Silent;

impl Progress for Silent {
    fn report(&mut self, _: Phase, _: u64, _: u64) {}
}

pub fn cancelled(flag: &AtomicBool) -> bool {
    flag.load(Ordering::Relaxed)
}

/// Gerador xorshift64: rápido, determinístico, sem dependências.
#[derive(Debug, Clone)]
pub struct XorShift(u64);

impl XorShift {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Valor em `0..bound` (bound > 0).
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

/// Semente diferente a cada execução, para que carimbos de uma rodada
/// anterior nunca sejam aceitos como válidos.
pub fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (u64::from(std::process::id()) << 32) ^ 0x9E37_79B9_7F4A_7C15
}

pub fn align_down(value: u64, align: u64) -> u64 {
    value - value % align
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_buf_is_aligned_and_zeroed() {
        let b = AlignedBuf::new(8192);
        assert_eq!(b.as_ptr() as usize % ALIGN, 0);
        assert!(b.iter().all(|&x| x == 0));
    }

    #[test]
    fn phase_ids_are_stable_snake_case() {
        assert_eq!(Phase::Probe.id(), "probe");
        assert_eq!(Phase::SeqRead.id(), "seq_read");
        assert_eq!(Phase::Scan.id(), "scan");
    }

    #[test]
    fn xorshift_is_deterministic() {
        let mut a = XorShift::new(42);
        let mut b = XorShift::new(42);
        assert_eq!(a.next_u64(), b.next_u64());
        assert!(a.below(10) < 10);
    }
}
