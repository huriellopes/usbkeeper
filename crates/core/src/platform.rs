use serde::Serialize;

use crate::format::FormatRequest;
use crate::{CoreError, Device};

/// Descobre os discos visíveis no sistema. Cada plataforma tem a sua.
pub trait Enumerator {
    fn list(&self) -> Result<Vec<Device>, CoreError>;

    /// Busca por caminho (`/dev/sdb`) ou nome (`sdb`).
    fn get(&self, query: &str) -> Result<Device, CoreError> {
        let wanted = query.trim_start_matches("/dev/");
        self.list()?
            .into_iter()
            .find(|d| d.name == wanted || d.path == query)
            .ok_or_else(|| CoreError::NotFound(query.to_string()))
    }
}

/// Um comando externo de um plano destrutivo. Planos são dados: podem ser
/// exibidos no `--dry-run` e testados sem executar nada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub description: String,
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
}

impl Step {
    pub fn new(description: impl Into<String>, program: &str, args: &[&str]) -> Self {
        Self {
            description: description.into(),
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            stdin: None,
        }
    }

    pub fn with_stdin(mut self, stdin: impl Into<String>) -> Self {
        self.stdin = Some(stdin.into());
        self
    }

    /// Linha de comando legível (para dry-run e logs).
    pub fn command_line(&self) -> String {
        let mut line = std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .map(|a| {
                if a.is_empty() || a.contains(' ') {
                    format!("'{a}'")
                } else {
                    a.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(input) = &self.stdin {
            line.push_str(&format!("  <<< {:?}", input));
        }
        line
    }
}

pub trait Formatter {
    /// Monta o plano (sem efeitos). Falha se faltar alguma ferramenta,
    /// antes de qualquer coisa ser apagada.
    fn plan(&self, device: &Device, request: &FormatRequest) -> Result<Vec<Step>, CoreError>;
    fn execute(&self, steps: &[Step]) -> Result<(), CoreError>;
}

/// Processo que mantém um ponto de montagem ocupado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Holder {
    pub pid: u32,
    pub command: String,
    pub path: String,
}

pub trait Mounts {
    /// Desmonta todas as partições. Se algo estiver em uso, retorna
    /// [`CoreError::Busy`] com os processos responsáveis.
    fn unmount_all(&self, device: &Device) -> Result<(), CoreError>;
    fn holders(&self, device: &Device) -> Vec<Holder>;
}

pub trait Ejector {
    /// Desliga o dispositivo para remoção segura (já desmontado).
    fn power_off(&self, device: &Device) -> Result<(), CoreError>;
}
