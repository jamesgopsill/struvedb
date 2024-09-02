use std::{
    collections::HashMap,
    fmt::Debug,
    fs::{self, read_dir},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

/// Any struct that wants to be managed by a collection
/// needs to satisfy these traits
pub trait Document<T> {
    // Returns the primary key for the document.
    fn primary_key(&self) -> Uuid;
    // Identifies whether is intersects with an existing document.
    // e.g., Can't have users with two emails.
    fn intersects(&self, doc: &T) -> Result<(), &str>;
}

/// A collection manages a set of Documents
/// that we want to persist beyond the life
/// of the service.
pub struct Collection<T: Document<T> + Debug + Serialize + DeserializeOwned + Clone + Sync + Send> {
    documents: HashMap<Uuid, T>,
    dir: Option<PathBuf>,
}

impl<T> Collection<T>
where
    T: Document<T> + Serialize + DeserializeOwned + Clone + Sync + Send + 'static + Debug,
{
    /// Create a new collection.
    /// Accepts an options PathBuf for writing to the filesystem.
    /// An In-Memory DB.
    pub fn new(dir: Option<PathBuf>) -> Self {
        let mut collection = Collection {
            documents: HashMap::new(),
            dir,
        };

        if collection.dir.is_some() {
            collection.load_structs_from_dir();
        }

        return collection;
    }

    /// Load in a directory of json objects.
    pub fn load_structs_from_dir(&mut self) {
        let dir = self.dir.as_ref().unwrap();
        let paths = read_dir(dir);
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

    pub fn new_arc(fp: Option<PathBuf>) -> Arc<RwLock<Collection<T>>> {
        let c = Collection::new(fp);
        return Arc::new(RwLock::new(c));
    }

    /// Insert a new Document
    pub fn insert(&mut self, new_doc: T) -> Result<(), &str> {
        let new_doc_pk = new_doc.primary_key();

        if self.documents.contains_key(&new_doc_pk) {
            return Err("Primary key used");
        }

        for (_, doc) in self.documents.iter() {
            // No clash on self as you may be updating it.
            if new_doc_pk != doc.primary_key() {
                let ans = new_doc.intersects(&doc);
                if ans.is_err() {
                    return Err("Intersection occurred");
                }
            }
        }

        if self.dir.is_some() {
            let path = self.dir.as_ref().unwrap().clone();
            let file_name = format!("{}.json", new_doc_pk);
            let path = path.join(file_name);
            let json = serde_json::to_string(&new_doc).unwrap();
            let err = fs::write(path, json);
            if err.is_err() {
                dbg!("Error writing to disk.");
            }
        }

        self.documents.insert(new_doc_pk, new_doc);

        return Ok(());
    }

    /// Update a document
    pub fn update(&mut self, updated_doc: T) -> Result<(), &str> {
        let updated_pk = updated_doc.primary_key();
        for (doc_pk, doc) in self.documents.iter() {
            // No clash on self as you may be updating it.
            if updated_pk != *doc_pk {
                let ans = updated_doc.intersects(&doc);
                match ans {
                    Ok(()) => {}
                    Err(_) => return Err("Intersection occurred"),
                }
            }
        }

        // Update the db
        if self.dir.is_some() {
            let path = self.dir.as_ref().unwrap().clone();
            let file_name = format!("{}.json", updated_pk);
            let path = path.join(file_name);
            let json = serde_json::to_string(&updated_doc).unwrap();
            let err = fs::write(path, json);
            if err.is_err() {
                dbg!("Error writing to disk.");
            }
        }

        self.documents.insert(updated_pk, updated_doc);

        return Ok(());
    }

    /// Find all documents that meet the criteria.
    /// Returns a vector of immutable references.
    pub fn filter(&self, f: impl Fn(&T) -> bool) -> Vec<T> {
        self.documents
            .iter()
            .filter(|(_, v)| f(v))
            .map(|(_, v)| v)
            .cloned()
            .collect()
    }

    /// Find the first document that satisfies the criteria.
    pub fn find(&self, f: impl Fn(&T) -> bool) -> Option<T> {
        self.documents
            .iter()
            .find(|(_, v)| f(v))
            .map(|(_, v)| v)
            .cloned()
    }

    /// Get a document by its uuid
    pub fn by_primary_key(&self, uuid: &Uuid) -> Option<T> {
        return self.documents.get(uuid).cloned();
    }

    /// Remove a document from the DB
    pub fn delete(&mut self, pk: &Uuid) -> Result<(), &str> {
        let exists = self.documents.contains_key(pk);
        if !exists {
            return Err("Key does not exist");
        }

        // Delete file
        if self.dir.is_some() {
            let path = self.dir.as_ref().unwrap().clone();
            let file_name = format!("{}.json", pk);
            let path = path.join(file_name);
            let err = fs::remove_file(path);
            if err.is_err() {
                dbg!("Error removing file from dir (it may not exist)");
            }
        }

        self.documents.remove(pk);

        return Ok(());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::Deserialize;
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
        fp.push("users");
        let _ = fs::remove_dir_all(&fp);
        let _ = fs::create_dir_all(&fp);
        let mut c = Collection::<User>::new(Some(fp));

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
