//! General utility functions and types.

#[cfg(test)]
pub use self::tests::*;
use diesel::{r2d2::ConnectionManager, Connection, PgConnection};
use r2d2::CustomizeConnection;
use std::ops::Deref;
use uuid::Uuid;
use validator::{Validate, ValidationErrors};

/// Type aliases for DB connections
pub type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type PooledConnection =
    r2d2::PooledConnection<ConnectionManager<PgConnection>>;

/// A small wrapper struct to indicate that the wrapped value has been
/// validated. Built on top of [validator]. This struct can only be constructed
/// via [Valid::validate].
#[derive(Copy, Clone, Debug)]
pub struct Valid<T: Validate> {
    inner: T,
}

impl<T: Validate> Valid<T> {
    /// Validate the given value. If validation succeeds, wrap it in a
    /// [Valid] to indicate it's valid.
    pub fn validate(value: T) -> Result<Self, ValidationErrors> {
        // We can't do a blanket TryFrom<T: Validate> implementation because of
        // this bug https://github.com/rust-lang/rust/issues/50133
        // Will have to wait for better specialization
        value.validate()?;
        Ok(Self { inner: value })
    }
}

impl<T: Validate> Deref for Valid<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

/// A DB connection customizer that wraps each connection in a transaction
/// before returning it. This should be used in all unit tests to prevent
/// make changes to the DB.
#[derive(Copy, Clone, Debug)]
struct TestConnectionCustomizer;

impl CustomizeConnection<PgConnection, diesel::r2d2::Error>
    for TestConnectionCustomizer
{
    fn on_acquire(
        &self,
        conn: &mut PgConnection,
    ) -> Result<(), diesel::r2d2::Error> {
        conn.begin_test_transaction()
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }

    fn on_release(&self, _conn: PgConnection) {}
}

/// Initialize a new DB connection pool, for use in the webserver.
pub fn init_db_conn_pool(database_url: &str) -> Result<Pool, r2d2::Error> {
    let manager = ConnectionManager::new(database_url);
    r2d2::Pool::builder().build(manager)
}

/// Initialize a new DB connection pool for use in tests. Reads the DB URL from
/// the environment. Also, all connections are wrapped in a test transaction
/// to prevent making modifications to the DB.
pub fn init_test_db_conn_pool() -> Result<Pool, r2d2::Error> {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let manager = ConnectionManager::new(database_url);
    r2d2::Pool::builder()
        .connection_customizer(Box::new(TestConnectionCustomizer))
        .build(manager)
}

/// Converts a UUID to a Juniper (GraphQL) ID.
pub fn uuid_to_gql_id(uuid: Uuid) -> juniper::ID {
    juniper::ID::new(uuid.to_string())
}

/// Converts a Juniper (GraphQL) ID to a UUID. If the given string is not a
/// valid UUID, then just return the nil UUID (all zeroes). This is useful in
/// the API because we want to handle malformed UUIDs the same way we handle
/// UUIDs that aren't in the database.
pub fn gql_id_to_uuid(id: &juniper::ID) -> Uuid {
    parse_uuid(&id.to_string())
}

/// Parses the given string into a UUID. If the string cannot be parsed
/// properly, this just returns the default UUID (all zeroes).
pub fn parse_uuid(id: &str) -> Uuid {
    Uuid::parse_str(id).unwrap_or_default()
}

/// Converts a map to a GraphQL object. Takes in an iterator of (K, V) so that
/// any map type can be used (HashMap, BTreeMap, etc.).
pub fn map_to_gql_object<K: ToString, V>(
    map: impl ExactSizeIterator<Item = (K, V)>,
    mapper: impl Fn(V) -> juniper::Value,
) -> juniper::Value {
    let len = map.len();
    let obj = map.fold(
        juniper::Object::with_capacity(len),
        |mut acc, (field, value)| {
            acc.add_field(field.to_string(), mapper(value));
            acc
        },
    );
    juniper::Value::Object(obj)
}

#[cfg(test)]
mod tests {
    /// Assert that the first value is an Err, and that its string form matches
    /// the second argument.
    #[macro_export]
    macro_rules! assert_err {
        ($res:expr, $msg:tt $(,)?) => {
            match $res {
                Ok(_) => panic!("Expected Err, got Ok"),
                Err(err) => assert_eq!(err.to_string(), $msg),
            }
        };
    }
}
