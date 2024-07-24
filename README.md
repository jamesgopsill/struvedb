# `struvedb` - A minimal in-memory with persistance struct vec document collection store.

`struvedb` is a minimal wrapper around a vec of structs in Rust that provides flat file persistence. Updates are persisted on inserts and updates and file modifications are at the per struct level to minimise I/O. This is unlike polling mechanisms that typically write the entire DB to file every few minutes.

I created it as a I needed minimal DB like querying and persistence when making demonstrators for my research and innovation projects.

### Features

- Flat file persistance. Each struct is stored on a separate line. `max_byte_length` dictates how large a struct can become. I use this to conveniently identify the file write offest for struct updates and padding up to that length with spaces. You'll see this if you open up the flat file.
- `max_byte_length` is dynamically controlled and will increment a default of `128` or a user specified amount when an object goes beyond the limit.
- Querys use the Rust filter and find logic. Results are cloned out. Any changes need to be made by passing an updated struct through the update function.
- Delete function to remove a struct from the DB.
- `does_not_clash` trait fcn to provide uniqueness checks.
- Minimal I/O. No polling/regular dumps of the in-memory db to a file. Updates are persisted as they are submitted.
- `Collection::new` and `Collection::new_arc` to instantiate a collection. The latter is useful for multi-threaded/async applications.

### Roadmap

- More testing.
- Make a YouTube video.

## Getting started

The package is **not** on [crates.io](https://crates.io/) yet so you will have to add it to your dependencies through the git url.

```toml
[dependencies]
struvedb = { git = "https://github.com/jamesgopsill/struvedb" }
```

There are a couple of examples in the `examples` folder that can be run using:

```bash
> cargo run --example [demo|user]
```

in your terminal.

Below is the `examples/demo.rs` example.

```rust
use serde::{Deserialize, Serialize};
use struvedb::{Collection, Document};
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
    fn does_not_clash(&self, doc: &User) -> Result<(), &str> {
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
    fp.push("my_users.col");

    // Create the collection and specify the max_byte_size
    // and file if you wish to persist the data
    let mut users = Collection::<User>::new(Some(fp), None);

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

```


