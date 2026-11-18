use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("dispositivo não encontrado: {0}")]
    NotFound(String),
    #[error("falha ao consultar o sistema: {0}")]
    Backend(String),
    #[error("saída inesperada do sistema: {0}")]
    Parse(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
