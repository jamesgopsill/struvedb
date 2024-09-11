use std::{
    fmt::Debug,
    fs::File,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use indexmap::IndexMap;
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::Document;

pub enum CollectionBackend {
    InMemory,
    Dir,
    File,
}

pub struct Collection<T: Document<T> + Debug + Serialize + DeserializeOwned + Clone + Sync + Send> {
    pub path: Option<PathBuf>,
    pub documents: IndexMap<Uuid, T>,
    pub backend: CollectionBackend,
    pub max_byte_length: usize,
    pub byte_length_increment: usize,
    pub file: Option<File>,
}

impl<T> Collection<T>
where
    T: Document<T> + Serialize + DeserializeOwned + Clone + Sync + Send + 'static + Debug,
{
    pub fn new(backend: CollectionBackend, path: Option<PathBuf>) -> Self {
        let mut collection = Collection {
            path,
            documents: IndexMap::new(),
            backend,
            max_byte_length: 128,
            byte_length_increment: 64,
            file: None,
        };

        match collection.backend {
            CollectionBackend::Dir => collection.load_structs_from_dir(),
            CollectionBackend::File => collection.load_structs_from_file(),
            CollectionBackend::InMemory => {}
        }

        collection
    }

    pub fn new_arc(
        backend: CollectionBackend,
        path: Option<PathBuf>,
    ) -> Arc<RwLock<Collection<T>>> {
        let c = Collection::new(backend, path);
        return Arc::new(RwLock::new(c));
    }

    pub fn insert(&mut self, new_doc: T) -> Result<(), &str> {
        if self.documents.contains_key(&new_doc.primary_key()) {
            return Err("Primary key used");
        }

        for (_, doc) in self.documents.iter() {
            // No clash on self as you may be updating it.
            if new_doc.primary_key() != doc.primary_key() {
                let ans = new_doc.intersects(&doc);
                if ans.is_err() {
                    return Err("Intersection occurred");
                }
            }
        }

        match self.backend {
            CollectionBackend::Dir => {
                let s = self.write_to_dir(&new_doc);
                if s.is_err() {
                    return Err("Error writing to DB");
                }
            }
            CollectionBackend::File => {
                let s: Result<(), &str> = self.write_new_document_to_file(&new_doc);
                if s.is_err() {
                    return Err("Error writing to DB");
                }
            }
            CollectionBackend::InMemory => {}
        }

        self.documents.insert(new_doc.primary_key(), new_doc);

        return Ok(());
    }

    /// Update a document
    pub fn update(&mut self, updated_doc: T) -> Result<(), &str> {
        for (doc_pk, doc) in self.documents.iter() {
            // No clash on self as you may be updating it.
            if updated_doc.primary_key() != *doc_pk {
                let ans = updated_doc.intersects(&doc);
                match ans {
                    Ok(()) => {}
                    Err(_) => return Err("Intersection occurred"),
                }
            }
        }

        match self.backend {
            CollectionBackend::Dir => {
                let s = self.write_to_dir(&updated_doc);
                if s.is_err() {
                    return Err("Error writing to DB");
                }
            }
            CollectionBackend::File => {
                let s: Result<(), &str> = self.write_updated_document_to_file(&updated_doc);
                if s.is_err() {
                    return Err("Error writing to DB");
                }
            }
            CollectionBackend::InMemory => {}
        }

        self.documents
            .insert(updated_doc.primary_key(), updated_doc);

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

        // Potential error between the persistent filestore
        // and hashmap if the backends are not successful
        // in writing the data.
        self.documents.shift_remove(pk);

        match self.backend {
            CollectionBackend::Dir => {
                let s = self.remove_from_dir(pk);
                if s.is_err() {
                    return Err("Error removing from DB");
                }
            }
            CollectionBackend::File => {
                let s: Result<(), &str> = self.rewrite_file();
                if s.is_err() {
                    return Err("Error writing to DB");
                }
            }
            CollectionBackend::InMemory => {}
        }

        return Ok(());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::Deserialize;
    use std::fs;
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
    fn test_dir_based() {
        let mut fp = std::env::current_dir().unwrap();
        fp.push("collections");
        fp.push("users");
        let _ = fs::remove_dir_all(&fp);
        let _ = fs::create_dir_all(&fp);
        let mut c = Collection::<User>::new(CollectionBackend::Dir, Some(fp));

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

    #[test]
    fn test_in_memory() {
        let mut c = Collection::<User>::new(CollectionBackend::InMemory, None);

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

    #[test]
    fn test_file_based() {
        let mut fp = std::env::current_dir().unwrap();
        fp.push("collections");
        fp.push("user.col");
        let _ = fs::remove_file(fp.clone());
        let mut c = Collection::<User>::new(CollectionBackend::File, Some(fp));

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
