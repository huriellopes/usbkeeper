//! Verificar/reparar sistemas de arquivos: tipos e interpretação dos
//! códigos de saída (que diferem entre as ferramentas).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Vfat,
    Exfat,
    Ntfs,
    Ext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus {
    Clean,
    Fixed,
    ErrorsRemain,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PartitionRepair {
    pub path: String,
    pub fstype: Option<String>,
    pub status: RepairStatus,
    /// Últimas linhas da saída das ferramentas.
    pub log: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RepairReport {
    pub check_only: bool,
    pub partitions: Vec<PartitionRepair>,
    pub table_warning: Option<String>,
}

impl RepairReport {
    /// "errors" se alguma partição ficou com erros (ou a ferramenta falhou),
    /// "fixed" se algo foi corrigido, senão "clean".
    pub fn verdict(&self) -> &'static str {
        let any = |s: &[RepairStatus]| self.partitions.iter().any(|p| s.contains(&p.status));
        if any(&[RepairStatus::ErrorsRemain, RepairStatus::Failed]) {
            "errors"
        } else if any(&[RepairStatus::Fixed]) {
            "fixed"
        } else {
            "clean"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Clean,
    Changed,
    Errors,
    Failed,
}

/// e2fsck e fsck.exfat usam bits (1/2 = alterou, 4 = erros restantes,
/// 8+ = falha); fsck.vfat usa 1 = problemas/alterou e 2 = erro de uso;
/// ntfsfix só distingue 0 de não-zero.
fn outcome(tool: Tool, code: i32) -> Outcome {
    match tool {
        _ if code == 0 => Outcome::Clean,
        Tool::Ntfs => Outcome::Errors,
        Tool::Vfat if code == 1 => Outcome::Changed,
        Tool::Vfat => Outcome::Failed,
        Tool::Ext | Tool::Exfat if !(0..8).contains(&code) => Outcome::Failed,
        Tool::Ext | Tool::Exfat if code & 4 != 0 => Outcome::Errors,
        Tool::Ext | Tool::Exfat => Outcome::Changed,
    }
}

/// Status de uma verificação (que nunca altera nada).
pub fn check_status(tool: Tool, code: i32) -> RepairStatus {
    match outcome(tool, code) {
        Outcome::Clean => RepairStatus::Clean,
        Outcome::Changed | Outcome::Errors => RepairStatus::ErrorsRemain,
        Outcome::Failed => RepairStatus::Failed,
    }
}

/// O status final do reparo vem da nova verificação feita logo depois.
pub fn repair_status(tool: Tool, repair_code: i32, recheck_code: i32) -> RepairStatus {
    let repaired = outcome(tool, repair_code);
    if repaired == Outcome::Failed {
        return RepairStatus::Failed;
    }
    match outcome(tool, recheck_code) {
        Outcome::Clean if repaired == Outcome::Changed => RepairStatus::Fixed,
        Outcome::Clean => RepairStatus::Clean,
        Outcome::Failed => RepairStatus::Failed,
        _ => RepairStatus::ErrorsRemain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_codes_per_tool() {
        assert_eq!(check_status(Tool::Ext, 0), RepairStatus::Clean);
        assert_eq!(check_status(Tool::Ext, 4), RepairStatus::ErrorsRemain);
        assert_eq!(check_status(Tool::Ext, 1), RepairStatus::ErrorsRemain);
        assert_eq!(check_status(Tool::Ext, 8), RepairStatus::Failed);
        assert_eq!(check_status(Tool::Vfat, 1), RepairStatus::ErrorsRemain);
        assert_eq!(check_status(Tool::Vfat, 2), RepairStatus::Failed); // erro de uso
        assert_eq!(check_status(Tool::Ntfs, 1), RepairStatus::ErrorsRemain);
    }

    #[test]
    fn repair_result_comes_from_recheck() {
        assert_eq!(repair_status(Tool::Ext, 1, 0), RepairStatus::Fixed);
        assert_eq!(repair_status(Tool::Ext, 0, 0), RepairStatus::Clean);
        assert_eq!(repair_status(Tool::Ext, 1, 4), RepairStatus::ErrorsRemain);
        assert_eq!(repair_status(Tool::Vfat, 2, 0), RepairStatus::Failed);
        assert_eq!(repair_status(Tool::Exfat, 1, 8), RepairStatus::Failed);
    }

    #[test]
    fn report_verdict() {
        let p = |status| PartitionRepair {
            path: "/dev/sdb1".into(),
            fstype: None,
            status,
            log: vec![],
            note: None,
        };
        let r = |v: Vec<PartitionRepair>| RepairReport {
            check_only: false,
            partitions: v,
            table_warning: None,
        };
        assert_eq!(r(vec![p(RepairStatus::Clean)]).verdict(), "clean");
        assert_eq!(
            r(vec![p(RepairStatus::Clean), p(RepairStatus::Fixed)]).verdict(),
            "fixed"
        );
        assert_eq!(
            r(vec![p(RepairStatus::Fixed), p(RepairStatus::ErrorsRemain)]).verdict(),
            "errors"
        );
        assert_eq!(r(vec![p(RepairStatus::Unsupported)]).verdict(), "clean");
    }
}
