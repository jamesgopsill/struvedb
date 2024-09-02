use std::vec;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use struvedb::{Collection, Document};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserScopes {
    USER,
    ADMIN,
}

/// Our user struct that we wish to become a document
/// and be stored in a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub uuid: Uuid,
    pub email: String,
    pub name: String,
    pub scopes: Vec<UserScopes>,
    pub active: bool,
    pub created_date: DateTime<Utc>,
    pub last_logged_in_date: DateTime<Utc>,
    pwd_hash: String,
}

/// Implementation of the Document Traits
/// This enables to have different types of primay key
/// And different checking criteria (e.g., unique keys)
/// for all our documents.
impl Document<User> for User {
    fn primary_key(&self) -> Uuid {
        self.uuid.clone()
    }

    fn intersects(&self, doc: &User) -> Result<(), &str> {
        if self.email == doc.email {
            return Err("Email is already in use.");
        }
        return Ok(());
    }
}

impl User {
    pub fn new(name: String, email: String, pwd: String, scopes: Vec<UserScopes>) -> User {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let pwd_hash = argon2
            .hash_password(pwd.as_bytes(), &salt)
            .unwrap()
            .to_string();
        let now = Utc::now();
        User {
            uuid: Uuid::new_v4(),
            name,
            email,
            pwd_hash,
            scopes,
            active: true,
            last_logged_in_date: now,
            created_date: now,
        }
    }
}

fn main() {
    // Provide a file if you want persistence storage
    let mut fp = std::env::current_dir().unwrap();
    fp.push("collections");
    fp.push("users");

    // Create the collection
    let mut user_collection = Collection::<User>::new(Some(fp));

    let user = User::new(
        "example".to_string(),
        "example@e.g.com".to_string(),
        "example".to_string(),
        vec![UserScopes::USER],
    );
    println!("{:?}", user);
    user_collection.insert(user).unwrap();

    let all_users = user_collection.find(|_| true);
    println!("{:?}", all_users)
}
