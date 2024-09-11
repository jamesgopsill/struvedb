use std::fmt::Debug;
use std::fs;

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::Document;

use super::collection::Collection;

impl<T> Collection<T>
where
    T: Document<T> + Serialize + DeserializeOwned + Clone + Sync + Send + 'static + Debug,
{
    pub fn load_structs_from_dir(&mut self) {
        if self.path.is_none() {
            return;
        }
        let path = self.path.as_ref().unwrap();
        let paths = fs::read_dir(path);
        if paths.is_err() {
            dbg!("Error reading ReadDir");
            return;
        }
        let paths = paths.unwrap();
        for path in paths {
            if path.is_err() {
                dbg!("Error reading DirEntry");
                continue;
            }
            let path = path.unwrap().path();
            if path.extension().unwrap() != "json" {
                continue;
            }
            let f = fs::OpenOptions::new().read(true).open(&path);
            if f.is_err() {
                dbg!("Error opening {}", &path);
                continue;
            }
            let f = f.unwrap();
            // Could better handle serde parsing errors.
            let doc: T = serde_json::from_reader(f).unwrap();
            self.documents.insert(doc.primary_key(), doc);
        }
    }

    pub fn write_to_dir(&self, doc: &T) -> Result<(), &str> {
        let json = serde_json::to_string(&doc).unwrap();
        let path = self.path.clone().unwrap();
        let file_name = format!("{}.json", doc.primary_key());
        let path = path.join(file_name);
        let err = fs::write(path, json);
        if err.is_err() {
            return Err("Error writing to disk.");
        }
        return Ok(());
    }

    pub fn remove_from_dir(&self, pk: &Uuid) -> Result<(), &str> {
        // Delete file
        let path = self.path.clone().unwrap();
        let file_name = format!("{}.json", pk);
        let path = path.join(file_name);
        let err = fs::remove_file(path);
        if err.is_err() {
            return Err("Error removing file from dir (it may not exist)");
        }
        Ok(())
    }
}
