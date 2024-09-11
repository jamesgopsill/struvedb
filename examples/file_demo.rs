use serde::{Deserialize, Serialize};
use struvedb::{Collection, CollectionBackend, Document};
use uuid::Uuid;

/// The struct we want to manage in struvecdb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub uuid: Uuid,
    pub name: String,
}

/// User must implement the Document Traits
impl Document<User> for User {
    /// How we derive the primary key for the document
    fn primary_key(&self) -> Uuid {
        self.uuid.clone()
    }
    /// A fcn that needs to be satisfied to prevent any clashes
    /// Can contain as many checks as you like.
    /// E.g., unique fields.
    fn intersects(&self, doc: &User) -> Result<(), &str> {
        if self.name == doc.name {
            return Err("Name is already in use.");
        }
        return Ok(());
    }
}

impl User {
    pub fn new(name: String) -> User {
        User {
            uuid: Uuid::new_v4(),
            name,
        }
    }
}

fn main() {
    // Provide a file if you want persistence storage
    let mut fp = std::env::current_dir().unwrap();
    fp.push("collections");
    fp.push("users.col");

    // Create the collection and specify the max_byte_size
    // and file if you wish to persist the data
    let mut users = Collection::<User>::new(CollectionBackend::File, Some(fp));

    let user = User::new("demo".to_string());
    println!("{:?}", user);

    // Adding a new user
    users.insert(user).unwrap();

    // Querying using standard Rust iterators
    let all_users = users.filter(|_| true);
    println!("{:?}", all_users);

    // A user
    let mut a_user = users.find(|_| true).unwrap();
    println!("{:?}", a_user);

    a_user.name = "example".to_string();
    println!("{:?}", a_user);
    users.update(a_user).unwrap();
}
