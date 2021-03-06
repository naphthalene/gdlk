#![deny(clippy::all)]

use crate::utils::{ContextBuilder, QueryRunner};
use diesel::{dsl, QueryDsl, RunQueryDsl};
use gdlk_api::{
    models::{Factory, NewUser, NewUserProvider},
    schema::users,
};
use juniper::InputValue;
use maplit::hashmap;
use serde_json::json;

mod utils;

static QUERY: &str = r#"
    mutation InitializeUserMutation(
        $username: String!,
    ) {
        initializeUser(input: {
            username: $username
        }) {
            userEdge {
                node {
                    username
                }
            }
        }
    }
"#;

/// Initialize a user successfully
#[test]
fn test_initialize_user_success() {
    let mut context_builder = ContextBuilder::new();
    let conn = context_builder.db_conn();

    let user_provider = NewUserProvider {
        sub: "sub",
        provider_name: "provider",
        user_id: None,
    }
    .create(conn);
    context_builder.set_user_provider(user_provider);

    let runner = QueryRunner::new(context_builder);
    assert_eq!(
        runner.query(
            QUERY,
            hashmap! {
                "username" => InputValue::scalar("user1"),
            }
        ),
        (
            json!({
                "initializeUser": {
                    "userEdge": {
                        "node": {
                            "username": "user1"
                        }
                    }
                }
            }),
            vec![]
        )
    );
}

/// Try to initialize a user while not logged in.
#[test]
fn test_initialize_user_not_logged_in() {
    let context_builder = ContextBuilder::new();
    let runner = QueryRunner::new(context_builder);

    assert_eq!(
        runner.query(
            QUERY,
            hashmap! {
                "username" => InputValue::scalar("user1"),
            }
        ),
        (
            serde_json::Value::Null,
            vec![json!({
                "message": "Not logged in",
                "locations": [{"line": 5, "column": 9}],
                "path": ["initializeUser"],
            })]
        )
    );
}

/// Setting a username that's already taken should return an error.
#[test]
fn test_initialize_user_duplicate_username() {
    let mut context_builder = ContextBuilder::new();
    let conn = context_builder.db_conn();

    NewUser { username: "user1" }.create(conn);
    let user_provider = NewUserProvider {
        sub: "sub",
        provider_name: "provider",
        user_id: None,
    }
    .create(conn);
    context_builder.set_user_provider(user_provider);

    let runner = QueryRunner::new(context_builder);
    assert_eq!(
        runner.query(
            QUERY,
            hashmap! {
                "username" => InputValue::scalar("user1"), // Already taken
            }
        ),
        (
            serde_json::Value::Null,
            vec![json!({
                "message": "This resource already exists",
                "locations": [{"line": 5, "column": 9}],
                "path": ["initializeUser"],
            })]
        )
    );
}

/// Setting a username that doesn't pass validation should return an error
#[test]
fn test_initialize_user_invalid_username() {
    let mut context_builder = ContextBuilder::new();
    let conn = context_builder.db_conn();

    let user_provider = NewUserProvider {
        sub: "sub",
        provider_name: "provider",
        user_id: None,
    }
    .create(conn);
    context_builder.set_user_provider(user_provider);

    let runner = QueryRunner::new(context_builder);
    assert_eq!(
        runner.query(
            QUERY,
            hashmap! {
                "username" => InputValue::scalar(""), // Invalid username
            }
        ),
        (
            serde_json::Value::Null,
            vec![json!({
                "message": "Input validation error(s)",
                "locations": [{"line": 5, "column": 9}],
                "path": ["initializeUser"],
                "extensions": {
                    "username": [
                        {"min": "1", "value": "\"\"", "max": "20"},
                    ]
                },
            })]
        )
    );
    assert_eq!(
        runner.query(
            QUERY,
            hashmap! {
                // Length limit is 20 chars
                "username" => InputValue::scalar("012345678901234567890"),
            }
        ),
        (
            serde_json::Value::Null,
            vec![json!({
                "message": "Input validation error(s)",
                "locations": [{"line": 5, "column": 9}],
                "path": ["initializeUser"],
                "extensions": {
                    "username": [
                        {"min": "1", "value": "\"012345678901234567890\"", "max": "20"},
                    ]
                },
            })]
        )
    );
}

/// Trying to initialize a user that's already been initialized should return
/// an error.
#[test]
fn test_initialize_user_already_initialized() {
    let mut context_builder = ContextBuilder::new();
    let conn = context_builder.db_conn();

    let user = NewUser { username: "user1" }.create(conn);
    let user_provider = NewUserProvider {
        sub: "sub",
        provider_name: "provider",
        user_id: Some(user.id),
    }
    .create(conn);
    context_builder.set_user_provider(user_provider);

    let runner = QueryRunner::new(context_builder);
    assert_eq!(
        runner.query(
            QUERY,
            hashmap! {
                "username" => InputValue::scalar("user2"),
            }
        ),
        (
            serde_json::Value::Null,
            vec![json!({
                // This is a shitty error message, but fuck it. This shouldn't
                // be possible to hit from the UI anyway
                "message": "Not found",
                "locations": [{"line": 5, "column": 9}],
                "path": ["initializeUser"],
            })]
        )
    );

    // Make sure there's still only one user in the DB, to ensure that the
    // user creation got rolled back
    assert_eq!(
        users::table
            .select(dsl::count_star())
            .get_result::<i64>(runner.db_conn())
            .unwrap(),
        1
    );
}
