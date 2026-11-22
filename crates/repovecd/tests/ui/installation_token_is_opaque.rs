//! Compile-fail fixture proving an installation token cannot be read from
//! outside the adapter that owns it.

use repovecd::github_repository_client::InstallationAccessToken;

fn main() {
    let token = match InstallationAccessToken::new("ghs_installation") {
        Ok(token) => token,
        Err(_) => return,
    };

    let _raw_value = token.value;
    let _secret = token.secret();
}