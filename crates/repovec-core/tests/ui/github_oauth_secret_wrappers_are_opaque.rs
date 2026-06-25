//! Compile-fail fixture proving OAuth secret wrapper fields stay private.

use repovec_core::github_oauth::{AccessToken, DeviceCode, UserCode};

fn main() {
    let _constructed_device_code = DeviceCode("device-secret".to_owned());

    let user_code = UserCode::new("ABCD-1234");
    let _raw_user_code = user_code.0;

    let token = AccessToken::new("gho_secret", ["repo"]);
    let _raw_token = token.token;
}
