use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::document::Document;

/// A collection manages a set of Documents
/// that we want to persist beyond the life
/// of the service.
pub struct FileBasedCollection<
    T: Document<T> + Debug + Serialize + DeserializeOwned + Clone + Sync + Send,
> {
    documents: Vec<T>,
    uuid_to_idx: HashMap<Uuid, usize>,
    max_byte_length: usize,
    byte_length_increment: usize,
    file: File,
}

impl<T> FileBasedCollection<T>
where
    T: Document<T> + Serialize + DeserializeOwned + Clone + Sync + Send + 'static + Debug,
{
    /// Create a new collection.
    /// Accepts an options PathBuf for writing to the filesystem.
    /// An In-Memory DB.
    pub fn new(fp: PathBuf, byte_length_increment: Option<usize>) -> Self {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&fp);
        if f.is_err() {
            dbg!("Error opening {}", &fp);
        }
        let file = f.unwrap();

        let mut collection = FileBasedCollection {
            documents: vec![],
            uuid_to_idx: HashMap::new(),
            max_byte_length: 0,
            byte_length_increment: byte_length_increment.unwrap_or(128),
            file,
        };

        collection.load_structs_from_file();

        return collection;
    }

    pub fn new_arc(
        fp: PathBuf,
        byte_length_increment: Option<usize>,
    ) -> Arc<RwLock<FileBasedCollection<T>>> {
        let c = FileBasedCollection::new(fp, byte_length_increment);
        return Arc::new(RwLock::new(c));
    }

    pub fn load_structs_from_file(&mut self) {
        let reader = BufReader::new(&self.file);

        for line in reader.lines() {
            let line = line.unwrap();
            let document = serde_json::from_str(&line.trim());
            if document.is_err() {
                break;
            }
            self.documents.push(document.unwrap());
        }
        // Update the hashmap for doc locations
        for (i, doc) in self.documents.iter().enumerate() {
            self.uuid_to_idx.insert(doc.primary_key(), i);
        }
    }

    /// Insert a new Document
    pub fn insert(&mut self, doc: T) -> Result<(), &str> {
        let key = doc.primary_key();

        if self.uuid_to_idx.contains_key(&key) {
            return Err("Primary key used");
        }

        for v in self.documents.iter() {
            // No clash on self as you may be updating it.
            if v.primary_key() != doc.primary_key() {
                let ans = v.intersects(&doc);
                match ans {
                    Ok(()) => {}
                    Err(_) => return Err("Clash occurred"),
                }
            }
        }

        // Write to db
        let string = serde_json::to_string(&doc);
        if string.is_err() {
            return Err("Error turning struct into JSON");
        }
        let string = string.unwrap();
        let byte_length = string.len();
        if byte_length > self.max_byte_length {
            let div = (byte_length / self.byte_length_increment) + 1;
            self.max_byte_length = self.byte_length_increment * div;
            println!(
                "{} > DB Resize New Byte Length: {}",
                Utc::now(),
                self.max_byte_length
            );
            let resize_success = self.resize_db();
            if resize_success.is_err() {
                return Err("Failed to resize DB");
            }
        }
        let padded_string = format!("{:width$}\n", string, width = self.max_byte_length);
        let offset: u64 = (self.documents.len() * (self.max_byte_length + 1))
            .try_into()
            .unwrap();

        let write_success = self.file.write_at(padded_string.as_bytes(), offset);
        if write_success.is_err() {
            return Err("Failed to write");
        }

        // Add to the db
        self.uuid_to_idx.insert(key, self.documents.len());
        self.documents.push(doc);

        return Ok(());
    }

    /// Update a document
    pub fn update(&mut self, doc: T) -> Result<(), &str> {
        for v in self.documents.iter() {
            // No clash on self as you may be updating it.
            if v.primary_key() != doc.primary_key() {
                let ans = v.intersects(&doc);
                match ans {
                    Ok(()) => {}
                    Err(_) => return Err("Clash occurred"),
                }
            }
        }

        // Update DB
        let string = serde_json::to_string(&doc);
        if string.is_err() {
            return Err("Error turning struct into JSON");
        }
        let string = string.unwrap();
        let byte_length = string.len();
        if byte_length > self.max_byte_length {
            let div = (byte_length / self.byte_length_increment) + 1;
            self.max_byte_length = self.byte_length_increment * div;
            let resize_success = self.resize_db();
            if resize_success.is_err() {
                return Err("Failed to resize DB");
            }
        }

        let padded_string = format!("{:width$}\n", string, width = self.max_byte_length);
        // Write right location in the file
        let idx = self.uuid_to_idx.get(&doc.primary_key());
        if idx.is_none() {
            return Err("Row idx cannot be found");
        }
        let idx = idx.unwrap();
        let offset: u64 = (idx * (self.max_byte_length + 1)).try_into().unwrap();

        let write_success = self.file.write_at(padded_string.as_bytes(), offset);
        if write_success.is_err() {
            return Err("Failed to write");
        }

        let idx = self.uuid_to_idx.get(&doc.primary_key());
        if idx.is_none() {
            return Err("Row idx cannot be found");
        }
        let idx = idx.unwrap();
        self.documents[*idx] = doc;

        return Ok(());
    }

    /// Find all documents that meet the criteria.
    /// Returns a vector of immutable references.
    pub fn filter(&self, f: impl Fn(&T) -> bool) -> Vec<T> {
        self.documents.iter().filter(|v| f(v)).cloned().collect()
    }

    /// Find the first document that satisfies the criteria.
    pub fn find(&self, f: impl Fn(&T) -> bool) -> Option<T> {
        self.documents.iter().find(|v| f(v)).cloned()
    }

    /// Get a document by its uuid
    pub fn by_primary_key(&self, uuid: &Uuid) -> Option<T> {
        let idx = self.uuid_to_idx.get(uuid);
        if idx.is_none() {
            return None;
        }
        let idx = idx.unwrap();
        let doc = self.documents[*idx].clone();
        return Some(doc);
    }

    /// Remove a document from the DB
    pub fn delete(&mut self, uuid: &Uuid) -> Result<(), &str> {
        let idx = self.uuid_to_idx.get(uuid);
        if idx.is_none() {
            return Err("No idx found");
        }
        let idx = idx.unwrap().clone();

        // decrement all the indexes above the one being removed
        for (_k, v) in self.uuid_to_idx.iter_mut() {
            if *v > idx {
                *v -= 1;
            }
        }

        // Remove from the map and vec.
        self.uuid_to_idx.remove(uuid);
        self.documents.remove(idx);

        // Clear and re-populate the DB
        let cleared = self.file.set_len(0);
        if cleared.is_err() {
            return Err("Failed to clear contents of DB.");
        }
        for (idx, doc) in self.documents.iter().enumerate() {
            let string = serde_json::to_string(&doc);
            if string.is_err() {
                return Err("Error turning struct into JSON");
            }
            let string = string.unwrap();
            let byte_length = string.len();
            if byte_length > self.max_byte_length {
                return Err("Struct is to large");
            }
            let padded_string = format!("{:width$}\n", string, width = self.max_byte_length);
            let offset: u64 = (idx * (self.max_byte_length + 1)).try_into().unwrap();
            let write_success = self.file.write_at(padded_string.as_bytes(), offset);
            if write_success.is_err() {
                return Err("Failed to write");
            }
        }

        return Ok(());
    }

    fn resize_db(&mut self) -> Result<(), &str> {
        let cleared = self.file.set_len(0);
        if cleared.is_err() {
            return Err("Failed to clear contents of DB.");
        }
        for (idx, doc) in self.documents.iter().enumerate() {
            let string = serde_json::to_string(&doc);
            if string.is_err() {
                return Err("Error turning struct into JSON");
            }
            let string = string.unwrap();
            let byte_length = string.len();
            if byte_length > self.max_byte_length {
                return Err("Struct is to large");
            }
            let padded_string = format!("{:width$}\n", string, width = self.max_byte_length);
            let offset: u64 = (idx * (self.max_byte_length + 1)).try_into().unwrap();
            let write_success = self.file.write_at(padded_string.as_bytes(), offset);
            if write_success.is_err() {
                return Err("Failed to write");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::Deserialize;
    use std::fs::remove_file;
    use uuid::Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct User {
        uuid: Uuid,
        name: String,
    }

    impl Document<User> for User {
        fn primary_key(&self) -> Uuid {
            self.uuid.clone()
        }

        fn intersects(&self, doc: &User) -> Result<(), &str> {
            if self.name == doc.name {
                return Err("Email is already in use.");
            }
            return Ok(());
        }
    }

    impl User {
        pub fn new(name: String) -> Self {
            User {
                uuid: Uuid::new_v4(),
                name,
            }
        }
    }

    #[test]
    fn test_insert() {
        let mut fp = std::env::current_dir().unwrap();
        fp.push("collections");
        fp.push("user.col");
        let _ = remove_file(fp.clone());
        let mut c = FileBasedCollection::<User>::new(fp, None);

        let user = User::new("bob".to_string());
        let mut user_cloned = user.clone();
        let res = c.insert(user);
        if res.is_err() {
            println!("{:?}", res.unwrap())
        }
        assert_eq!(res.is_ok(), true);

        let user = User::new("bill".to_string());
        let b_uuid = user.uuid.clone();
        let res = c.insert(user);
        assert_eq!(res.is_ok(), true);

        user_cloned.name = "Trevor".to_string();
        let res = c.update(user_cloned);
        assert_eq!(res.is_ok(), true);

        let user = User::new("dan".to_string());
        let uuid = user.uuid.clone();
        let res = c.insert(user);
        assert_eq!(res.is_ok(), true);

        let get_user = c.by_primary_key(&uuid);
        if get_user.is_some() {
            println!("{:?}", get_user.unwrap());
        }

        let del = c.delete(&b_uuid);
        assert_eq!(del.is_ok(), true);

        let get_user = c.by_primary_key(&uuid);
        if get_user.is_some() {
            println!("{:?}", get_user.unwrap());
        }
    }
}
