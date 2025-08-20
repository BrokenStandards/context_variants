use context_variants::variants;
use serde::{Deserialize, Serialize};

// Test when_* conditional attributes
#[variants(
    Login: requires(email, password).excludes(username, profile_pic),
    Profile: requires(username).optional(email).excludes(password, profile_pic),
    suffix = "Form"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConditionalForm {
    /// Email field with conditional attributes
    #[when_base(doc = "Base email field")]
    #[when_base(serde(rename = "base_email"))]
    #[when_required(serde(rename = "email_address"))]
    #[when_required(doc = "Required email for login")]
    #[when_optional(serde(skip_serializing_if = "Option::is_none"))]
    #[when_optional(doc = "Optional email for profile")]
    pub email: String,

    /// Password field with conditional attributes
    #[when_base(serde(rename = "base_password"))]
    #[when_required(serde(rename = "pwd"))]
    pub password: String,

    /// Username field with conditional attributes
    #[when_base(doc = "Base username field")]
    #[when_required(doc = "Required username")]
    #[when_optional(doc = "Optional username")]
    pub username: String,

    /// Profile picture field (excluded in current variants)
    pub profile_pic: Option<String>,
}

fn main() {
    // Test base struct - should use when_base attributes
    let base_form = ConditionalForm {
        email: "user@example.com".to_string(),
        password: "secret".to_string(),
        username: "john".to_string(),
        profile_pic: None,
    };
    
    let base_json = serde_json::to_string(&base_form).unwrap();
    println!("Base ConditionalForm JSON: {}", base_json);

    // Test LoginForm - should use when_required attributes
    let login = LoginForm {
        email: "user@example.com".to_string(),
        password: "secret".to_string(),
    };
    
    let login_json = serde_json::to_string(&login).unwrap();
    println!("LoginForm JSON: {}", login_json);

    // Test ProfileForm - should use when_optional for email, when_required for username
    let profile = ProfileForm {
        username: "jane".to_string(),
        email: Some("jane@example.com".to_string()),
    };
    
    let profile_json = serde_json::to_string(&profile).unwrap();
    println!("ProfileForm JSON: {}", profile_json);

    // Test with None email - should skip due to when_optional skip_serializing_if
    let profile_empty = ProfileForm {
        username: "empty".to_string(),
        email: None,
    };
    
    let empty_json = serde_json::to_string(&profile_empty).unwrap();
    println!("ProfileForm (empty) JSON: {}", empty_json);
}
