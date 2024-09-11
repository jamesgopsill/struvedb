use std::fs;
use std::os::unix::fs::FileExt;
use std::{fmt::Debug, io::BufRead, io::BufReader};

use chrono::Utc;
use serde::{de::DeserializeOwned, Serialize};

use crate::Document;

use super::collection::Collection;

impl<T> Collection<T>
where
    T: Document<T> + Serialize + DeserializeOwned + Clone + Sync + Send + 'static + Debug,
{
    pub fn load_structs_from_file(&mut self) {
        if self.path.is_none() {
            return;
        }
        let path = self.path.as_ref().unwrap();
        let f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(path);
        if f.is_err() {
            println!("Error opening {:?}", path);
            return;
        }
        let f = f.unwrap();
        let reader = BufReader::new(&f);
        for line in reader.lines() {
            let line = line.unwrap();
            let document = serde_json::from_str(&line.trim());
            if document.is_err() {
                break;
            }
            let document: T = document.unwrap();
            self.documents.insert(document.primary_key(), document);
        }
        self.file = Some(f);
    }

    pub fn write_new_document_to_file(&mut self, doc: &T) -> Result<(), &str> {
        let json = serde_json::to_string(&doc);
        if json.is_err() {
            return Err("Error turning struct into JSON");
        }
        let json = json.unwrap();
        let byte_length = json.len();
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
        let padded_string = format!("{:width$}\n", json, width = self.max_byte_length);
        let offset: u64 = (self.documents.len() * (self.max_byte_length + 1))
            .try_into()
            .unwrap();

        let file = self.file.as_ref().unwrap();
        let write_success = file.write_at(padded_string.as_bytes(), offset);
        if write_success.is_err() {
            return Err("Failed to write");
        }

        Ok(())
    }

    pub fn write_updated_document_to_file(&mut self, doc: &T) -> Result<(), &str> {
        let json = serde_json::to_string(&doc);
        if json.is_err() {
            return Err("Error turning struct into JSON");
        }
        let json = json.unwrap();
        let byte_length = json.len();
        if byte_length > self.max_byte_length {
            let div = (byte_length / self.byte_length_increment) + 1;
            self.max_byte_length = self.byte_length_increment * div;
            let resize_success = self.resize_db();
            if resize_success.is_err() {
                return Err("Failed to resize DB");
            }
        }

        let padded_string = format!("{:width$}\n", json, width = self.max_byte_length);
        // Write right location in the file
        let idx = self.documents.get_index_of(&doc.primary_key());
        if idx.is_none() {
            return Err("Row idx cannot be found");
        }
        let idx = idx.unwrap();
        let offset: u64 = (idx * (self.max_byte_length + 1)).try_into().unwrap();
        let file = self.file.as_ref().unwrap();
        let write_success = file.write_at(padded_string.as_bytes(), offset);
        if write_success.is_err() {
            return Err("Failed to write");
        }

        Ok(())
    }

    pub fn resize_db(&self) -> Result<(), &str> {
        let file = self.file.as_ref().unwrap();
        let cleared = file.set_len(0);
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
            let file = self.file.as_ref().unwrap();
            let write_success = file.write_at(padded_string.as_bytes(), offset);
            if write_success.is_err() {
                return Err("Failed to write");
            }
        }
        Ok(())
    }

    pub fn rewrite_file(&self) -> Result<(), &str> {
        // Clear and re-populate the DB
        let file = self.file.as_ref().unwrap();
        let cleared = file.set_len(0);
        if cleared.is_err() {
            return Err("Failed to clear contents of DB.");
        }

        for (idx, doc) in self.documents.iter().enumerate() {
            let json = serde_json::to_string(&doc);
            if json.is_err() {
                return Err("Error turning struct into JSON");
            }
            let json = json.unwrap();
            let byte_length = json.len();
            if byte_length > self.max_byte_length {
                return Err("Struct is to large");
            }
            let padded_string = format!("{:width$}\n", json, width = self.max_byte_length);
            let offset: u64 = (idx * (self.max_byte_length + 1)).try_into().unwrap();
            let write_success = file.write_at(padded_string.as_bytes(), offset);
            if write_success.is_err() {
                return Err("Failed to write");
            }
        }

        Ok(())
    }
}
