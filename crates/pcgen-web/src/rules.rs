//! Immutable runtime packs, invalidated by their actual source dependencies.
use pcgen_engine::Engine;
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    length: u64,
    modified: SystemTime,
}
fn stamp(path: &Path) -> Result<Stamp, String> {
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Stamp {
        length: metadata.len(),
        modified: metadata.modified().map_err(|error| error.to_string())?,
    })
}
struct Entry {
    engine: Arc<Engine>,
    dependencies: Vec<(PathBuf, Stamp)>,
}
#[derive(Default)]
pub(crate) struct RuleStore {
    entries: Mutex<HashMap<PathBuf, Entry>>,
}
impl RuleStore {
    pub fn load(&self, path: &Path) -> Result<Arc<Engine>, String> {
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "rule store lock poisoned".to_string())?;
        if let Some(entry) = entries.get(&path) {
            if entry
                .dependencies
                .iter()
                .all(|(path, previous)| stamp(path).as_ref() == Ok(previous))
            {
                return Ok(entry.engine.clone());
            }
        }
        let native = path
            .extension()
            .is_some_and(|extension| extension == "json");
        let base = if native {
            let db = pcgen_import::native::read_database(&path)?;
            pcgen_import::native::base_path(&path, &db)
        } else {
            path.clone()
        };
        let graph = pcgen_import::PccGraph::load(&base).map_err(|error| error.to_string())?;
        let mut paths = if native {
            pcgen_import::native::database_paths(&path)?
                .into_iter()
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::from([path.clone()])
        };
        for file in graph.files {
            paths.insert(file.path);
            paths.extend(file.data_files.into_iter().map(|file| file.resolved_path));
        }
        // Capture before compilation. A concurrent edit will invalidate the next load.
        let dependencies = paths
            .into_iter()
            .map(|path| stamp(&path).map(|stamp| (path, stamp)))
            .collect::<Result<Vec<_>, _>>()?;
        let pack = if native {
            pcgen_import::native::compile_database(&path)?
        } else {
            pcgen_import::compile_pcc(&path)
                .map_err(|error| error.to_string())?
                .0
        };
        let engine = Arc::new(Engine::new(pack));
        entries.insert(
            path,
            Entry {
                engine: engine.clone(),
                dependencies,
            },
        );
        Ok(engine)
    }
}
