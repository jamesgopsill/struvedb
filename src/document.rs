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
