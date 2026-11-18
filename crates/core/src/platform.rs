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
