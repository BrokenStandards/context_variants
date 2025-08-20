// Test case: incomplete field coverage should be caught
use context_variants::variants;

#[variants(
    Create: requires(name), // ERROR: email not specified and no default
    suffix = "Coverage"
)]
#[derive(Debug)]
struct CoverageTest {
    pub name: String,
    pub email: String,
}

fn main() {}
