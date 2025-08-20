// Test case: complex field conflict with all_fields()
use context_variants::variants;

#[variants(
    Create: requires(id).optional(all_fields().except(password, admin)), // ERROR: id mentioned as both required and optional
    suffix = "Complex"
)]
#[derive(Debug)]
struct ComplexConflictTest {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub password: String,
    pub admin: bool,
}

fn main() {}
