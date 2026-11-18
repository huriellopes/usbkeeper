use thiserror::Error;

use crate::platform::Holder;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("dispositivo não encontrado: {0}")]
    NotFound(String),
    #[error("falha ao consultar o sistema: {0}")]
    Backend(String),
    #[error("saída inesperada do sistema: {0}")]
    Parse(String),
    #[error("ferramenta ausente: {tool} (instale o pacote {package})")]
    MissingTool { tool: String, package: String },
    #[error("{mountpoint} está em uso{}", holders_suffix(.holders))]
    Busy {
        mountpoint: String,
        holders: Vec<Holder>,
    },
    #[error("o comando falhou: {command}\n{stderr}")]
    CommandFailed { command: String, stderr: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn holders_suffix(holders: &[Holder]) -> String {
    if holders.is_empty() {
        return " (nenhum processo visível; talvez de outro usuário, tente com sudo)".into();
    }
    let list = holders
        .iter()
        .map(|h| format!("  - PID {} {} ({})", h.pid, h.command, h.path))
        .collect::<Vec<_>>()
        .join("\n");
    format!(" pelos processos:\n{list}")
}
