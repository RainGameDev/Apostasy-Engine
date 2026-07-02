use std::fs;
use std::path::PathBuf;

use hail::ImportResolver;

pub struct FileImportResolver {
    roots: Vec<PathBuf>,
}

impl FileImportResolver {
    pub fn new() -> Self {
        Self {
            roots: vec![
                PathBuf::from("res/scripts"),
                PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/res/scripts")),
            ],
        }
    }
}

impl ImportResolver for FileImportResolver {
    fn resolve_path(&self, _base: Option<&str>, path: &str) -> Result<String, String> {
        let key = path.trim_start_matches("./").trim_end_matches(".hail");
        Ok(key.to_string())
    }

    fn get_script(&self, path: &str) -> Result<String, String> {
        for root in &self.roots {
            let candidate = root.join(format!("{path}.hail"));
            if let Ok(source) = fs::read_to_string(&candidate) {
                return Ok(source);
            }
        }
        Err(format!(
            "import '{path}' not found under res/scripts (tried {} roots)",
            self.roots.len()
        ))
    }
}
