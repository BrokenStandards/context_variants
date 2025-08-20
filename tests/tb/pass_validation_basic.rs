// Test case: proper validation passing test
use context_variants::variants;

#[variants(
    Create: requires(name, email).excludes(id).default(optional),
    suffix = "Valid"
)]
#[derive(Debug)]
struct ValidTest {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub optional_field: String,
}

fn main() {
    let create = CreateValid {
        name: "test".to_string(),
        email: "test@example.com".to_string(),
        optional_field: Some("optional".to_string()),
    };
    // This should compile without errors
    assert!(create.name == "test");
    assert!(create.email == "test@example.com");
    assert!(create.optional_field == Some("optional".to_string()));
}
