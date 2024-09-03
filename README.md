# `struvedb` - A minimal in-memory with persistance struct vec document collection store.

`struvedb` is a minimal wrapper around a vecs and hashmaps of structs in Rust that provides in-memory, directory-based persitent, and file-based persistent collections. Updates are persisted on inserts and updates and file modifications are at the per struct level to minimise I/O. This is unlike polling mechanisms that typically write the entire DB to disk every few minutes.

I created it as a I needed minimal DB like querying and persistence when making demonstrators for my research and innovation projects.

## Document

You can design a document to meet your data needs. All it requires an implementation of `get_primary_key` and `intersects`. The `intersects` fcn provides a simple mechanism to implement any checks that would invalidate the insert or update of an document. For example, not having duplicate emails for different users.

## Collection Types

The crate features three collections that can fit many demonstrator needs. All collections implement `::new` and `::new_arc`. The latter is useful for multi-threaded/async applications. Querys use the Rust filter and find logic. Results are cloned out. Any changes need to be made by passing an updated struct through the update function.

### `InMemoryCollection`

The data does not persist if the applicaiton closes. Provides the same interface as the other collections so it can be easily swapped during development. 

#### Use Cases

Good for testing and demos that do not need the data after running.

### `FileBasedCollection`

The file-based collection stores the data in a single file on disk. Each struct is stored on a separate line. `max_byte_length` dictates how large a struct can become. I use this to conveniently identify the file write offset for struct updates and padding up to that length with spaces. You'll see this if you open up the file. There is minimal I/O with no polling/regular dumps of the in-memory db to a file. Updates are persisted as they are submitted.

- `fp` PathBuff detailing where you want to store the data.
- `max_byte_length` is dynamically controlled and will increment a default of `128` or a user specified amount when an object goes beyond the limit.

#### Use Cases

Good for data that is unlikely to be updated and structs are a fixed size. Demos where there may be lots of inserts that would result in many thousands of files if the directory-based approach was chosen. E.g., IoT demonstrators.

### `DirBasedCollection`

The directory based collection stores each struct instance in its own file within the directory. This can be space efficient when struct instances vary considerably in size.

#### Use Cases

I use this for demonstrator user accounts.

### Roadmap

- More testing.
- Make a YouTube video.
- Batch updates where they succeed only if all succeed.

## Getting started

The package is **not** on [crates.io](https://crates.io/) yet so you will have to add it to your dependencies through the git url.

```toml
[dependencies]
struvedb = { git = "https://github.com/jamesgopsill/struvedb" }
```

There are a couple of examples in the `examples` folder that can be run using:

```bash
> cargo run --example dir_demo.rs
```

in your terminal.

### Examples

More can be found in the `examples` folder.


```rust
use serde::{Deserialize, Serialize};
use struvedb::{DirBasedCollection, Document};
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
    fp.push("users");

    // Create the collection and pass the dir.
    let mut users = DirBasedCollection::<User>::new(fp);

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


