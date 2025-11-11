# Doclayer

Doclayer is a Rust library that provides a simple and efficient way to manage document storage with support for various backends, including in-memory storage and MongoDB. It offers features like type-safe querying, filtering, sorting, pagination, and schema migrations.

## Key Features

- **Asynchronous API** - Built with async/await for non-blocking operations
- **Type-safe document storage** - Define your data structures with Serde and store them safely
- **Multiple backends** - Support for in-memory and MongoDB storage with an extensible trait system
- **Flexible querying** - Powerful, composable query API for filtering and sorting consistently across backends
- **Schema migrations** - Versioned migrations for evolving your data models
- **Dynamic dispatch** - Runtime selection of backends without compile-time type knowledge

## Quick Start

Or add the git repo to your `Cargo.toml`:

```toml
[dependencies]
doclayer = { git = "https://github.com/wizrds/doclayer-rs", tag = "0.1.0" }
```

To enable the MongoDB backend:

```toml
[dependencies]
doclayer = { git = "https://github.com/wizrds/doclayer-rs", tag = "0.1.0", features = ["mongodb"] }
```

## Usage

### Defining Documents

All documents must implement the `Document` trait. Start by defining a struct with `Serialize` and `Deserialize` derives:

```rust
use doclayer::prelude::*;
use bson::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl Document for User {
    fn id(&self) -> &Uuid {
        &self.id
    }

    fn collection_name() -> &'static str {
        "users"
    }
}
```

The `id()` method returns the document's unique identifier (UUID), and `collection_name()` specifies which collection this document type belongs to.

### Setting Up a Document Store

#### In-Memory Store (Development/Testing)

```rust
use doclayer::{prelude::*, memory::InMemoryStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory store backend
    let store = DocumentStore::new(InMemoryStore::builder().build().await?);

    // Get a typed collection for User documents
    let user_collection = store.typed_collection::<User>();

    // Now you can perform operations on the collection
    user_collection
        .insert(vec![
            User {
                id: Uuid::new(),
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
            },
            User {
                id: Uuid::new(),
                name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
            },
        ])
        .await?;

    // Shutdown the store when done
    store.shutdown().await?;
    Ok(())
}
```

#### MongoDB Store (Production)

```rust
use doclayer::{prelude::*, mongodb::MongoDbStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a MongoDB store backend
    let store = DocumentStore::new(
        MongoDbStore::builder(
            "mongodb://localhost:27017",
            "my_database",
        )
        .build()
        .await?
    );

    let user_collection = store.typed_collection::<User>();

    // Perform operations...

    store.shutdown().await?;
    Ok(())
}
```

### Inserting Documents

Insert documents into a collection using the `insert` method:

```rust
// Insert a single user
let user = User {
    id: Uuid::new(),
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

user_collection.insert(vec![user]).await?;

// Insert multiple users
let users = vec![
    User {
        id: Uuid::new(),
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    },
    User {
        id: Uuid::new(),
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    },
];

user_collection.insert(users).await?;
```

### Querying Documents

The library provides a fluent query builder API with support for filtering, sorting, and pagination:

#### Simple Queries

```rust
// Query all documents in a collection
let all_users = user_collection.query(Query::builder().build()).await?;

// Limit and offset for pagination
let page_one = user_collection
    .query(
        Query::builder()
            .limit(10)
            .offset(0)
            .build()
    )
    .await?;

let page_two = user_collection
    .query(
        Query::builder()
            .limit(10)
            .offset(10)
            .build()
    )
    .await?;
```

#### Filtering

The `Filter` API provides various comparison and logical operators:

```rust
// Equality filter
let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::eq("name", "Alice"))
            .build()
    )
    .await?;

// Other comparison operators
let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::ne("name", "Bob"))
            .build()
    )
    .await?;

// String operations
let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::starts_with("name", "A"))
            .build()
    )
    .await?;

let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::contains("email", "@example.com"))
            .build()
    )
    .await?;

// Existence checks
let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::exists("email"))
            .build()
    )
    .await?;
```

#### Complex Filters

Combine multiple filters using logical operators:

```rust
// AND filter - all conditions must match
let results = user_collection
    .query(
        Query::builder()
            .filter(
                Filter::and(vec![
                    Filter::starts_with("name", "A"),
                    Filter::contains("email", "@example.com"),
                ])
            )
            .build()
    )
    .await?;

// OR filter - any condition can match
let results = user_collection
    .query(
        Query::builder()
            .filter(
                Filter::or(vec![
                    Filter::eq("name", "Alice"),
                    Filter::eq("name", "Bob"),
                ])
            )
            .build()
    )
    .await?;

// NOT filter - negate a condition
let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::not(Filter::eq("name", "Bob")))
            .build()
    )
    .await?;

// Array operations
let results = user_collection
    .query(
        Query::builder()
            .filter(Filter::any_of("name", vec!["Alice", "Bob", "Charlie"]))
            .build()
    )
    .await?;
```

#### Sorting

Sort query results in ascending or descending order:

```rust
// Sort ascending
let results = user_collection
    .query(
        Query::builder()
            .sort("name", SortDirection::Asc)
            .build()
    )
    .await?;

// Sort descending
let results = user_collection
    .query(
        Query::builder()
            .sort("email", SortDirection::Desc)
            .build()
    )
    .await?;
```

#### Combined Queries

Combine all query features for complex operations:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub tags: Vec<String>,
}

impl Document for Post {
    fn id(&self) -> &Uuid { &self.id }
    fn collection_name() -> &'static str { "posts" }
}

// Find the 5 most recent posts by a specific user with certain tags
let posts = post_collection
    .query(
        Query::builder()
            .filter(
                Filter::and(vec![
                    Filter::eq("user_id", &alice_user_id),
                    Filter::any_of("tags", vec!["rust", "database"]),
                ])
            )
            .sort("created_at", SortDirection::Desc)
            .limit(5)
            .build()
    )
    .await?;
```

### Updating Documents

Update existing documents by reinserting them with the same ID:

```rust
let mut user = User {
    id: Uuid::new(),
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

// Insert initial document
user_collection.insert(vec![user.clone()]).await?;

// Update the document
user.email = "alice.new@example.com".to_string();
user_collection.insert(vec![user]).await?;
```

### Deleting Documents

Delete documents by their ID:

```rust
let user_id = Uuid::new();

user_collection.delete(vec![user_id]).await?;

// Delete multiple documents
let ids_to_delete = vec![
    Uuid::new(),
    Uuid::new(),
    Uuid::new(),
];

user_collection.delete(ids_to_delete).await?;
```

### Collection Management

Create and drop collections programmatically:

```rust
// Create a collection
store.create_collection("custom_collection").await?;

// List all collections
let collections = store.list_collections().await?;
println!("Collections: {:?}", collections);

// Drop a collection
store.drop_collection("custom_collection").await?;
```

### Dynamic Dispatch

For scenarios where the backend type is not known at compile time, use `DynDocumentStore`:

```rust
use doclayer::{prelude::*, memory::InMemoryStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a dynamically dispatched store
    let store = DynDocumentStore::new(
        Box::new(InMemoryStore::builder().build().await?)
    );

    let user_collection = store.typed_collection::<User>();

    // Use the collection exactly like a typed store
    let user = User {
        id: Uuid::new(),
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    user_collection.insert(vec![user]).await?;

    let results = user_collection
        .query(Query::builder().build())
        .await?;

    println!("Users: {:?}", results);

    // Shutdown using the special method for dynamic stores
    store.shutdown().await?;
    Ok(())
}
```

Or create the static store first and then convert it with `into_dyn()`:

```rust
let static_store = DocumentStore::new(InMemoryStore::builder().build().await?);
let dyn_store = static_store.into_dyn();
```

### Schema Migrations

Define and run versioned schema migrations to evolve your data models:

#### Define a Migration

```rust
use doclayer::migrate::{Migration, MigrationRef, Migrations, MigrateOp};
use async_trait::async_trait;

struct AddEmailToUsersMigration;

#[async_trait]
impl Migration for AddEmailToUsersMigration {
    fn id(&self) -> &'static str {
        "001_add_email_to_users"
    }

    fn previous_id(&self) -> Option<&'static str> {
        None  // This is the first migration
    }

    async fn up(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
        // Add a new field to all documents in the users collection
        // The default paramter takes any value that can be converted to a Bson value
        op.add_field("users", "email", bson::Bson::Null).await?;
        Ok(())
    }

    async fn down(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
        // Remove the field when downgrading
        op.remove_field("users", "email").await?;
        Ok(())
    }
}

struct AddNameFieldMigration;

#[async_trait]
impl Migration for AddNameFieldMigration {
    fn id(&self) -> &'static str {
        "002_add_name_field"
    }

    fn previous_id(&self) -> Option<&'static str> {
        Some("001_add_email_to_users")  // Depends on the previous migration
    }

    async fn up(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
        op.add_field("users", "name", "").await?;
        Ok(())
    }

    async fn down(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
        op.remove_field("users", "name").await?;
        Ok(())
    }
}
```

#### Register Migrations

```rust
struct AppMigrations;

impl Migrations for AppMigrations {
    fn migrations() -> Vec<MigrationRef> {
        vec![
            Box::new(AddEmailToUsersMigration),
            Box::new(AddNameFieldMigration),
        ]
    }
}
```

#### Run Migrations

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = DocumentStore::new(InMemoryStore::builder().build().await?);

    // Upgrade to the latest schema version
    store.upgrade::<AppMigrations>().await?;

    // Or upgrade to a specific version
    store.upgrade_to::<AppMigrations>("001_add_email_to_users").await?;

    // Downgrade to a specific version
    store.downgrade_to::<AppMigrations>("001_add_email_to_users").await?;

    // Downgrade to the beginning
    store.downgrade::<AppMigrations>().await?;

    store.shutdown().await?;
    Ok(())
}
```

### Field Operations (Schema Manipulation)

Directly add or remove fields from documents in a collection:

```rust
// Add a new field to all documents with a default value
store.add_field("users", "created_at", None).await?;

// Remove a field from all documents
store.drop_field("users", "created_at").await?;
```

## Available Backends

### In-Memory Backend

Ideal for development, testing, and scenarios requiring fast access to small datasets:

```rust
use doclayer::memory::InMemoryStore;

let store = DocumentStore::new(InMemoryStore::builder().build().await?);
```

### MongoDB Backend

For production deployments requiring persistent storage and horizontal scalability:

```rust
use doclayer::mongodb::MongoDbStore;

let store = DocumentStore::new(
    MongoDbStore::builder(
        "mongodb://user:password@host:27017",
        "database_name",
    )
    .build()
    .await?
);
```

## License
This project is licensed under ISC License.

## Support & Feedback
If you encounter any issues or have feedback, please open an issue.

Made with ❤️ by Tim Pogue
