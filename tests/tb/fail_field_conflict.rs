// Test case: field mentioned multiple times should be caught
use context_variants::variants;

#[variants(
    Create: requires(name).optional(name), // ERROR: name mentioned twice
    suffix = "Conflict"
)]
#[derive(Debug)]
struct ConflictTest {
    pub name: String,
    pub email: String,
}

fn main() {}
