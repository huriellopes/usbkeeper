//! `imagem criar/verificar/restaurar` (backup do disco inteiro, zstd +
//! SHA-256) e `gravar-iso` (com conferência de hash antes e depois).

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use usbkeeper_core::blockdev::{BlockDevice, Phase, Progress};
use usbkeeper_core::image::{CopyReport, device_to_writer, hash_device, reader_to_device};
use usbkeeper_core::iso::{HEAD_LEN, inspect};
use usbkeeper_core::units::human_bytes;
use usbkeeper_core::{CoreError, Device};
use usbkeeper_history::kind;

use crate::backend::Backend;
use crate::checks::{cancel_flag, open_checked};
use crate::destructive::{Guard, confirm_and_take, precheck};
use crate::machine::Reporter;
use crate::store;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
const ZSTD_LEVEL: i32 = 3;

/// Gravado ao lado da imagem como `<imagem>.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageMeta {
    pub format: u8,
    pub model: String,
    pub serial: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub compressed: bool,
    pub created_at: String,
    pub usbkeeper_version: String,
}

pub fn meta_path(image: &Path) -> PathBuf {
    let mut p = image.as_os_str().to_owned();
    p.push(".json");
    PathBuf::from(p)
}

pub fn read_meta(image: &Path) -> Option<ImageMeta> {
    let text = std::fs::read_to_string(meta_path(image)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Compressão detectada pelo número mágico do zstd, não pela extensão.
pub fn is_zstd(path: &Path) -> Result<bool> {
    let mut magic = [0u8; 4];
    let mut f = File::open(path).with_context(|| format!("abrindo {}", path.display()))?;
    Ok(f.read(&mut magic)? == 4 && magic == ZSTD_MAGIC)
}

/// Fluxo com os bytes originais da imagem (descomprimindo se preciso).
pub fn open_image(path: &Path) -> Result<Box<dyn Read>> {
    let file =
        BufReader::new(File::open(path).with_context(|| format!("abrindo {}", path.display()))?);
    if is_zstd(path)? {
        Ok(Box::new(zstd::Decoder::with_buffer(file)?))
    } else {
        Ok(Box::new(file))
    }
}

/// Copia `len` bytes do disco para `path` (zstd se `compress`).
pub fn create_from(
    dev: &mut dyn BlockDevice,
    len: u64,
    path: &Path,
    compress: bool,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<CopyReport> {
    let file =
        BufWriter::new(File::create(path).with_context(|| format!("criando {}", path.display()))?);
    let report = if compress {
        let mut enc = zstd::Encoder::new(file, ZSTD_LEVEL)?;
        let r = device_to_writer(dev, len, &mut enc, Phase::Image, progress, cancel)?;
        enc.finish()?.flush()?;
        r
    } else {
        let mut file = file;
        let r = device_to_writer(dev, len, &mut file, Phase::Image, progress, cancel)?;
        file.flush()?;
        r
    };
    Ok(report)
}

/// SHA-256 e tamanho dos bytes originais de uma imagem (ou de uma ISO).
pub fn hash_file(
    path: &Path,
    progress: &mut dyn Progress,
    total_hint: u64,
) -> Result<(String, u64)> {
    let mut reader = open_image(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
        progress.report(Phase::Verify, total, total_hint.max(total));
    }
    Ok((
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        total,
    ))
}

pub fn restore_into(
    dev: &mut dyn BlockDevice,
    path: &Path,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
    total_hint: u64,
) -> Result<CopyReport> {
    let mut reader = open_image(path)?;
    Ok(reader_to_device(
        &mut reader,
        dev,
        progress,
        cancel,
        total_hint,
    )?)
}

/// Hash esperado de uma ISO: `--sha256`, `<iso>.sha256` ou `SHA256SUMS`
/// (linha com o nome do arquivo) na mesma pasta.
pub fn expected_sha256(iso: &Path, given: Option<&str>) -> Result<Option<String>> {
    let normalize = |h: &str| -> Result<String> {
        let h = h.trim().to_ascii_lowercase();
        if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("hash SHA-256 inválido: {h:?} (deve ter 64 caracteres hexadecimais)");
        }
        Ok(h)
    };
    if let Some(h) = given {
        return normalize(h).map(Some);
    }
    let mut side = iso.as_os_str().to_owned();
    side.push(".sha256");
    if let Ok(text) = std::fs::read_to_string(PathBuf::from(side))
        && let Some(first) = text.split_whitespace().next()
    {
        return normalize(first).map(Some);
    }
    let name = iso.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let sums = iso.parent().unwrap_or(Path::new(".")).join("SHA256SUMS");
    if let Ok(text) = std::fs::read_to_string(sums) {
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(hash), Some(file)) = (parts.next(), parts.next())
                && file.trim_start_matches('*') == name
            {
                return normalize(hash).map(Some);
            }
        }
    }
    Ok(None)
}

fn finish_verify(
    dev: &mut dyn BlockDevice,
    len: u64,
    expected: &str,
    reporter: &mut Reporter,
    cancel: &AtomicBool,
) -> Result<bool> {
    let got = match hash_device(dev, len, reporter, cancel) {
        Err(CoreError::Cancelled) => bail!("verificação cancelada (a gravação foi concluída)"),
        other => other?,
    };
    Ok(got == expected)
}

// ------------------------------------------------------------ comandos

pub fn create(
    b: &Backend,
    device: &str,
    output: &Path,
    compress: bool,
    overwrite: bool,
    json: bool,
) -> Result<()> {
    let dev = b.enumerator.get(device)?;
    if output.exists() && !overwrite {
        bail!(
            "{} já existe; use --sobrescrever ou escolha outro nome",
            output.display()
        );
    }
    if usbkeeper_core::safety::system_mountpoint(&dev).is_some() {
        bail!(
            "{} é o disco do sistema: a imagem de um sistema em uso não seria consistente",
            dev.path
        );
    }
    if !json {
        eprintln!(
            "Criando imagem de {} {} ({}) em {}{}. Só lê o disco.",
            dev.path,
            dev.display_name(),
            human_bytes(dev.size_bytes),
            output.display(),
            if compress {
                " (comprimida com zstd)"
            } else {
                ""
            }
        );
    }
    let mut raw = open_checked(b, &dev, false)?;
    let cancel = cancel_flag();
    let mut reporter = Reporter::new(json);
    let result = create_from(
        raw.as_mut(),
        dev.size_bytes,
        output,
        compress,
        &mut reporter,
        &cancel,
    );
    reporter.finish();
    let report = match result {
        Err(e) if matches!(e.downcast_ref::<CoreError>(), Some(CoreError::Cancelled)) => {
            let _ = std::fs::remove_file(output);
            bail!("cancelado: a imagem incompleta foi apagada");
        }
        other => other?,
    };
    let meta = ImageMeta {
        format: 1,
        model: dev.display_name(),
        serial: dev.serial.clone(),
        size_bytes: report.bytes,
        sha256: report.sha256.clone(),
        compressed: compress,
        created_at: usbkeeper_history::format_utc(usbkeeper_history::now()),
        usbkeeper_version: env!("CARGO_PKG_VERSION").into(),
    };
    std::fs::write(meta_path(output), serde_json::to_string_pretty(&meta)?)?;
    store::give_back(output);
    store::give_back(&meta_path(output));
    let file_size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    store::record(
        &dev,
        kind::IMAGE_CREATE,
        Some("created"),
        &format!(
            "imagem criada: {} ({})",
            output.display(),
            human_bytes(file_size)
        ),
        serde_json::json!({ "path": output, "meta": meta, "file_bytes": file_size }),
    );
    if json {
        return crate::print_json(
            &serde_json::json!({ "device": dev.path, "path": output, "meta": meta, "file_bytes": file_size }),
        );
    }
    println!(
        "✔ Imagem criada: {} ({} no arquivo, {} do disco)\n  SHA-256: {}\n  Informações em {}",
        output.display(),
        human_bytes(file_size),
        human_bytes(report.bytes),
        report.sha256,
        meta_path(output).display()
    );
    Ok(())
}

pub fn verify(image: &Path, json: bool) -> Result<()> {
    let meta = read_meta(image);
    let mut reporter = Reporter::new(json);
    let (sha, bytes) = hash_file(
        image,
        &mut reporter,
        meta.as_ref().map_or(0, |m| m.size_bytes),
    )?;
    reporter.finish();
    let ok = meta
        .as_ref()
        .map(|m| m.sha256 == sha && m.size_bytes == bytes);
    if json {
        return crate::print_json(
            &serde_json::json!({ "path": image, "sha256": sha, "bytes": bytes, "matches_meta": ok }),
        );
    }
    match ok {
        Some(true) => println!(
            "✔ Imagem íntegra: {} ({}), SHA-256 confere.",
            image.display(),
            human_bytes(bytes)
        ),
        Some(false) => bail!(
            "a imagem NÃO confere com {}: o arquivo está corrompido ou foi alterado",
            meta_path(image).display()
        ),
        None => println!(
            "SHA-256: {sha} ({}). Sem {}, não há com o que comparar.",
            human_bytes(bytes),
            meta_path(image).display()
        ),
    }
    Ok(())
}

pub fn restore(b: &Backend, image: &Path, device: &str, guard: Guard, json: bool) -> Result<()> {
    let dev = b.enumerator.get(device)?;
    precheck(&dev, &guard)?;
    let meta = read_meta(image);
    if let Some(m) = &meta
        && m.size_bytes > dev.size_bytes
    {
        bail!(
            "a imagem tem {} e o disco só {}: restaure num disco do mesmo tamanho ou maior",
            human_bytes(m.size_bytes),
            human_bytes(dev.size_bytes)
        );
    }
    if !json {
        eprintln!(
            "RESTAURAR {} em {} {}: tudo o que está no disco será substituído.",
            image.display(),
            dev.path,
            dev.display_name()
        );
        if let Some(m) = &meta {
            eprintln!(
                "Imagem de: {} ({}), criada em {}",
                m.model,
                human_bytes(m.size_bytes),
                m.created_at
            );
        }
    }
    let current = confirm_and_take(b, &dev, &guard)?;
    let mut raw = open_checked(b, &current, true)?;
    let cancel = cancel_flag();
    let mut reporter = Reporter::new(json);
    let total = meta.as_ref().map_or(current.size_bytes, |m| m.size_bytes);
    let written = match restore_into(raw.as_mut(), image, &mut reporter, &cancel, total) {
        Err(e) if matches!(e.downcast_ref::<CoreError>(), Some(CoreError::Cancelled)) => {
            reporter.finish();
            bail!(
                "cancelado: o disco ficou parcialmente restaurado; restaure de novo antes de usar"
            );
        }
        other => other?,
    };
    let expected = meta
        .as_ref()
        .map_or(written.sha256.clone(), |m| m.sha256.clone());
    let ok = finish_verify(
        raw.as_mut(),
        written.bytes,
        &expected,
        &mut reporter,
        &cancel,
    )?;
    reporter.finish();
    let verdict = if ok { "restored" } else { "verify_failed" };
    record_and_report(
        &current,
        kind::IMAGE_RESTORE,
        verdict,
        &format!("imagem restaurada de {}", image.display()),
        &written,
        ok,
        json,
    )
}

pub fn burn_iso(
    b: &Backend,
    iso: &Path,
    device: &str,
    given_sha: Option<&str>,
    force: bool,
    guard: Guard,
    json: bool,
) -> Result<()> {
    let mut head = vec![0u8; HEAD_LEN];
    let n = File::open(iso)
        .with_context(|| format!("abrindo {}", iso.display()))?
        .read(&mut head)?;
    let info = inspect(&head[..n]);
    if !info.is_iso && !info.hybrid {
        bail!("{} não parece uma imagem de disco (ISO)", iso.display());
    }
    if info.windows && !force {
        bail!(
            "{} é uma ISO de instalação do Windows ({}). Gravada byte a byte ela NÃO dá boot: \
             o Windows precisa de um preparo especial (partição FAT32 + install.wim dividido), \
             que o usbkeeper ainda não faz. Use uma ferramenta própria para isso; --forcar grava mesmo assim.",
            iso.display(),
            info.volume_id.as_deref().unwrap_or("?")
        );
    }
    let size = std::fs::metadata(iso)?.len();
    let dev = b.enumerator.get(device)?;
    precheck(&dev, &guard)?;
    if size > dev.size_bytes {
        bail!(
            "a ISO ({}) não cabe em {} ({})",
            human_bytes(size),
            dev.path,
            human_bytes(dev.size_bytes)
        );
    }
    if !info.hybrid && !json {
        eprintln!(
            "aviso: a ISO não tem setor de boot para pendrive (não é híbrida); pode não dar boot assim."
        );
    }
    let expected = expected_sha256(iso, given_sha)?;
    if let Some(exp) = &expected {
        if !json {
            eprintln!("Conferindo o SHA-256 da ISO antes de gravar…");
        }
        let mut reporter = Reporter::new(json);
        let (got, _) = hash_file(iso, &mut reporter, size)?;
        reporter.finish();
        if &got != exp {
            bail!(
                "o SHA-256 da ISO não confere (esperado {exp}, calculado {got}): download corrompido ou arquivo errado; nada foi gravado"
            );
        }
    } else if !json {
        eprintln!(
            "aviso: sem hash para conferir a ISO (use --sha256 ou deixe o .sha256/SHA256SUMS na mesma pasta)."
        );
    }
    if !json {
        eprintln!(
            "GRAVAR {} ({}) em {} {}: tudo o que está no disco será apagado.",
            info.volume_id.as_deref().unwrap_or("ISO"),
            human_bytes(size),
            dev.path,
            dev.display_name()
        );
    }
    let current = confirm_and_take(b, &dev, &guard)?;
    let mut raw = open_checked(b, &current, true)?;
    let cancel = cancel_flag();
    let mut reporter = Reporter::new(json);
    let mut file = BufReader::new(File::open(iso)?);
    let written = match reader_to_device(&mut file, raw.as_mut(), &mut reporter, &cancel, size) {
        Err(CoreError::Cancelled) => {
            reporter.finish();
            bail!("cancelado: o pendrive ficou com uma gravação incompleta");
        }
        other => other?,
    };
    let expected = expected.unwrap_or_else(|| written.sha256.clone());
    let ok = finish_verify(
        raw.as_mut(),
        written.bytes,
        &expected,
        &mut reporter,
        &cancel,
    )?;
    reporter.finish();
    let verdict = if ok { "written" } else { "verify_failed" };
    record_and_report(
        &current,
        kind::ISO_WRITE,
        verdict,
        &format!("ISO gravada: {}", iso.display()),
        &written,
        ok,
        json,
    )
}

fn record_and_report(
    dev: &Device,
    k: &str,
    verdict: &str,
    summary: &str,
    r: &CopyReport,
    ok: bool,
    json: bool,
) -> Result<()> {
    store::record(
        dev,
        k,
        Some(verdict),
        summary,
        serde_json::json!({ "report": r, "verified": ok }),
    );
    if json {
        return crate::print_json(
            &serde_json::json!({ "device": dev.path, "report": r, "verified": ok }),
        );
    }
    if ok {
        println!(
            "✔ Gravado e verificado ({}): SHA-256 {}",
            human_bytes(r.bytes),
            r.sha256
        );
        Ok(())
    } else {
        bail!(
            "a verificação falhou: o que foi lido do disco não confere com o que foi gravado (disco com defeito ou falsificado?)"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usbkeeper_core::blockdev::Silent;
    use usbkeeper_core::fakedrive::FakeDrive;

    fn never() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn create_verify_restore_roundtrip_compressed_and_raw() {
        for compress in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("pen.img");
            let mut src = FakeDrive::genuine(4 << 20);
            src.seed_user_data(&[0, 512, 3 << 20]);
            let made =
                create_from(&mut src, 4 << 20, &path, compress, &mut Silent, &never()).unwrap();
            assert_eq!(is_zstd(&path).unwrap(), compress);
            if compress {
                assert!(
                    std::fs::metadata(&path).unwrap().len() < 1 << 20,
                    "zeros comprimem"
                );
            }
            let (sha, bytes) = hash_file(&path, &mut Silent, 0).unwrap();
            assert_eq!((sha.as_str(), bytes), (made.sha256.as_str(), 4 << 20));

            let mut dst = FakeDrive::genuine(8 << 20);
            let back = restore_into(&mut dst, &path, &mut Silent, &never(), 0).unwrap();
            assert_eq!(back.sha256, made.sha256);
            assert_eq!(
                hash_device(&mut dst, 4 << 20, &mut Silent, &never()).unwrap(),
                made.sha256
            );
        }
    }

    #[test]
    fn expected_hash_sources() {
        let dir = tempfile::tempdir().unwrap();
        let iso = dir.path().join("distro.iso");
        std::fs::write(&iso, b"x").unwrap();
        let h = "a".repeat(64);
        assert_eq!(expected_sha256(&iso, None).unwrap(), None);
        std::fs::write(
            dir.path().join("SHA256SUMS"),
            format!("{} *outra.iso\n{h}  distro.iso\n", "b".repeat(64)),
        )
        .unwrap();
        assert_eq!(
            expected_sha256(&iso, None).unwrap().as_deref(),
            Some(h.as_str())
        );
        let side = "C".repeat(64);
        std::fs::write(
            dir.path().join("distro.iso.sha256"),
            format!("{side}  distro.iso\n"),
        )
        .unwrap();
        assert_eq!(
            expected_sha256(&iso, None).unwrap(),
            Some(side.to_lowercase())
        );
        assert_eq!(
            expected_sha256(&iso, Some(&"d".repeat(64))).unwrap(),
            Some("d".repeat(64))
        );
        assert!(expected_sha256(&iso, Some("123")).is_err());
    }

    #[test]
    fn meta_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("x.img.zst");
        let meta = ImageMeta {
            format: 1,
            model: "Pen".into(),
            serial: Some("ABC".into()),
            size_bytes: 10,
            sha256: "0".repeat(64),
            compressed: true,
            created_at: "2026-11-19 10:00 UTC".into(),
            usbkeeper_version: "0.8.0".into(),
        };
        std::fs::write(meta_path(&img), serde_json::to_string(&meta).unwrap()).unwrap();
        assert_eq!(read_meta(&img), Some(meta));
        assert!(meta_path(&img).ends_with("x.img.zst.json"));
    }
}
