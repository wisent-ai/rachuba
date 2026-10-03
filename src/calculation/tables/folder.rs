//! One tax year's tables are a folder of TOML section files.
//!
//! A transcribed publication runs to hundreds of lines of figures and the
//! citations that justify them, so each year is a folder and each section of
//! the publication is a file in it (sub-folders group a method's sections).
//! Every file is a fragment of the same document: the fragments are merged
//! table by table into one, and the loader of each jurisdiction deserializes
//! that. Two files defining the same key is a transcription error and is
//! refused by name, never resolved by file order.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use toml::{Table, Value};

const TABLE_EXTENSION: &str = "toml";

/// Every `.toml` file under `dir`, merged into one document.
pub fn read_folder(dir: &Path, describe: &dyn Fn() -> String) -> Result<Table> {
    if !dir.is_dir() {
        bail!("{}", describe());
    }
    let files = table_files(dir)?;
    if files.is_empty() {
        bail!(
            "{} holds no .toml file, so there is no table to load. {}",
            dir.display(),
            describe()
        );
    }
    let mut merged = Table::new();
    let mut origins: Vec<(String, PathBuf)> = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("reading {}", file.display()))?;
        let fragment: Table =
            toml::from_str(&text).with_context(|| format!("parsing {}", file.display()))?;
        merge(&mut merged, fragment, String::new(), &file, &mut origins)?;
    }
    Ok(merged)
}

/// The table files under `dir`, recursively, in a stable order. Anything else
/// in the folder is refused: a stray file next to tax figures is either a
/// section nobody loads or a note that belongs in a comment.
fn table_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("listing {}", dir.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<_>>()
        .with_context(|| format!("listing {}", dir.display()))?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files.extend(table_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some(TABLE_EXTENSION) {
            files.push(path);
        } else {
            bail!(
                "{} is not a .toml section file. A table folder holds only the transcribed \
                 sections of one year's publications; move anything else out of it.",
                path.display()
            );
        }
    }
    Ok(files)
}

/// Merge `fragment` into `into`, descending through tables and refusing any
/// key that is already defined, naming both files.
fn merge(
    into: &mut Table,
    fragment: Table,
    prefix: String,
    file: &Path,
    origins: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    for (key, value) in fragment {
        let dotted = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match (into.get_mut(&key), value) {
            (Some(Value::Table(existing)), Value::Table(incoming)) => {
                merge(existing, incoming, dotted, file, origins)?;
            }
            (Some(_), _) => {
                let first = origins
                    .iter()
                    .find(|(defined, _)| *defined == dotted || dotted.starts_with(&format!("{defined}.")))
                    .map(|(_, origin)| origin.display().to_string())
                    .with_context(|| format!("{dotted} was defined without a recorded origin"))?;
                bail!(
                    "{dotted} is defined in both {first} and {}. A key belongs to exactly one \
                     section file; delete one of the two definitions.",
                    file.display()
                );
            }
            (None, value) => {
                origins.push((dotted, file.to_path_buf()));
                into.insert(key, value);
            }
        }
    }
    Ok(())
}




