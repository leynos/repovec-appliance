//! Local test binary for the GitHub OAuth device-flow client.

use std::{convert::Infallible, error::Error, io};

use oauth2_test_server::OAuthTestServer;
use repovec_core::github_oauth::{ClientId, TokenPollOutcome};
use repovecd::{
    github_oauth_client::{DeviceFlowEndpoints, GitHubOAuthClient},
    github_token_store::{CredentialEncryptor, EncryptedGitHubTokenStore},
};
use serde_json::json;

#[derive(Clone, Debug)]
struct ExampleEncryptor;

impl CredentialEncryptor for ExampleEncryptor {
    type Error = Infallible;

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut ciphertext = b"example-encrypted:".to_vec();
        ciphertext.extend(plaintext.iter().rev());
        Ok(ciphertext)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let encrypted = ciphertext.strip_prefix(b"example-encrypted:").unwrap_or(ciphertext);
        Ok(encrypted.iter().rev().copied().collect())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    let server = runtime.block_on(OAuthTestServer::start());
    let registered_client = runtime.block_on(server.register_client(json!({
        "scope": "openid profile",
        "grant_types": ["urn:ietf:params:oauth:grant-type:device_code"],
        "client_name": "repovec-device-flow-test"
    })));
    let oauth_client = GitHubOAuthClient::new(DeviceFlowEndpoints::new(
        server.endpoints.device_code.clone(),
        server.endpoints.device_token.clone(),
    ))?;
    let client_id = ClientId::new(registered_client.client_id);
    let authorization = oauth_client.request_device_code(&client_id, ["openid", "profile"])?;

    runtime.block_on(
        server.approve_device_code(authorization.device_code.secret(), "repovec-test-user"),
    );
    let outcome = oauth_client.poll_token(&client_id, &authorization.device_code)?;
    let TokenPollOutcome::Authorized(token) = outcome else {
        return Err(io::Error::other("mock OAuth server did not return an access token").into());
    };

    let tempdir = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(tempdir.path())
        .ok_or_else(|| io::Error::other("temporary path was not UTF-8"))?;
    let store = EncryptedGitHubTokenStore::open(root, ExampleEncryptor)?;
    store.store_token(&token)?;
    let stored_token = store.load_token()?;
    if stored_token.secret() != token.secret() {
        return Err(io::Error::other("stored token did not match the issued token").into());
    }

    Ok(())
}
