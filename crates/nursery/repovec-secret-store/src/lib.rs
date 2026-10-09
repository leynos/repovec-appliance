//! Backend-neutral interfaces for protecting and durably storing secrets.
//!
//! This crate is in the repovec nursery. It separates secret protection from
//! persistence and makes exposure of secret bytes explicit.

use std::{error::Error, fmt};

/// A logical name used to address one secret.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SecretName(String);

impl SecretName {
    /// Creates a secret name.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }

    /// Returns the name text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Plaintext secret bytes with a redacted default debug representation.
#[derive(Eq, PartialEq)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    /// Creates a plaintext secret value.
    #[must_use]
    pub fn new(value: impl Into<Vec<u8>>) -> Self { Self(value.into()) }

    /// Explicitly exposes the plaintext bytes.
    #[must_use]
    pub fn expose(&self) -> &[u8] { &self.0 }

    /// Consumes the wrapper and exposes the plaintext bytes.
    #[must_use]
    pub fn into_exposed(self) -> Vec<u8> { self.0 }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("SecretBytes").field(&"REDACTED").finish()
    }
}

/// Protected secret bytes with a redacted default debug representation.
#[derive(Clone, Eq, PartialEq)]
pub struct ProtectedSecret(Vec<u8>);

impl ProtectedSecret {
    /// Creates a protected secret value.
    #[must_use]
    pub fn new(value: impl Into<Vec<u8>>) -> Self { Self(value.into()) }

    /// Explicitly exposes the protected representation to an adapter.
    #[must_use]
    pub fn expose(&self) -> &[u8] { &self.0 }

    /// Consumes the wrapper and exposes the protected representation.
    #[must_use]
    pub fn into_exposed(self) -> Vec<u8> { self.0 }
}

impl fmt::Debug for ProtectedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ProtectedSecret").field(&"REDACTED").finish()
    }
}

/// Requested binding policy for a protected secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectionPolicy {
    /// Bind protection to host-held key material.
    Host,
    /// Bind protection to a Trusted Platform Module 2.0 device.
    Tpm2,
    /// Require both host-held key material and a Trusted Platform Module.
    HostAndTpm2,
    /// Use the protection backend's documented default policy.
    BackendDefault,
}

/// Protects and recovers secret bytes independently of persistence.
pub trait SecretCodec {
    /// Error returned by the protection adapter.
    type Error: Error + Send + Sync + 'static;

    /// Protects plaintext for durable storage.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when protection fails.
    fn protect(
        &self,
        name: &SecretName,
        plaintext: &SecretBytes,
        policy: ProtectionPolicy,
    ) -> Result<ProtectedSecret, Self::Error>;

    /// Recovers plaintext from a protected representation.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when authentication or recovery fails.
    fn unprotect(
        &self,
        name: &SecretName,
        protected: &ProtectedSecret,
    ) -> Result<SecretBytes, Self::Error>;
}

/// Persists protected secret representations.
///
/// Filesystem implementations of [`replace`](Self::replace) must document
/// whether they provide restrictive creation permissions, same-directory
/// temporary creation, complete writes, file synchronization, atomic rename,
/// and parent-directory synchronization. Implementations must not claim durable
/// replacement when any required guarantee is absent.
pub trait SecretRepository {
    /// Error returned by the persistence adapter.
    type Error: Error + Send + Sync + 'static;

    /// Loads a protected secret when one exists.
    ///
    /// # Errors
    ///
    /// Returns the repository error when the value cannot be read.
    fn load(&self, name: &SecretName) -> Result<Option<ProtectedSecret>, Self::Error>;

    /// Durably replaces a protected secret.
    ///
    /// # Errors
    ///
    /// Returns the repository error when the replacement cannot be committed.
    fn replace(&self, name: &SecretName, protected: &ProtectedSecret) -> Result<(), Self::Error>;

    /// Removes a protected secret when present.
    ///
    /// # Errors
    ///
    /// Returns the repository error when removal cannot be committed.
    fn remove(&self, name: &SecretName) -> Result<(), Self::Error>;
}

/// High-level protected secret storage operations.
pub trait SecretStore {
    /// Error returned by the composed store.
    type Error: Error + Send + Sync + 'static;

    /// Protects and durably stores plaintext bytes.
    ///
    /// # Errors
    ///
    /// Returns a codec or repository error.
    fn store(
        &self,
        name: &SecretName,
        plaintext: &SecretBytes,
        policy: ProtectionPolicy,
    ) -> Result<(), Self::Error>;

    /// Loads and recovers plaintext bytes when a value exists.
    ///
    /// # Errors
    ///
    /// Returns a repository or codec error.
    fn load(&self, name: &SecretName) -> Result<Option<SecretBytes>, Self::Error>;

    /// Removes a protected value when present.
    ///
    /// # Errors
    ///
    /// Returns a repository error.
    fn remove(&self, name: &SecretName) -> Result<(), Self::Error>;
}

/// Composes an independent protection codec and repository.
#[derive(Clone, Debug)]
pub struct CodecBackedSecretStore<C, R> {
    codec: C,
    repository: R,
}

impl<C, R> CodecBackedSecretStore<C, R> {
    /// Creates a composed secret store.
    #[must_use]
    pub const fn new(codec: C, repository: R) -> Self { Self { codec, repository } }

    /// Returns the protection codec.
    #[must_use]
    pub const fn codec(&self) -> &C { &self.codec }

    /// Returns the protected-secret repository.
    #[must_use]
    pub const fn repository(&self) -> &R { &self.repository }
}

/// Error returned by a codec-backed secret store.
#[derive(Debug)]
pub enum SecretStoreError<C, R> {
    /// The protection codec failed.
    Codec(C),
    /// The protected-secret repository failed.
    Repository(R),
}

impl<C, R> fmt::Display for SecretStoreError<C, R>
where
    C: fmt::Display,
    R: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(formatter, "secret codec failed: {error}"),
            Self::Repository(error) => write!(formatter, "secret repository failed: {error}"),
        }
    }
}

impl<C, R> Error for SecretStoreError<C, R>
where
    C: Error + Send + Sync + 'static,
    R: Error + Send + Sync + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Repository(error) => Some(error),
        }
    }
}

impl<C, R> SecretStore for CodecBackedSecretStore<C, R>
where
    C: SecretCodec,
    R: SecretRepository,
{
    type Error = SecretStoreError<C::Error, R::Error>;

    fn store(
        &self,
        name: &SecretName,
        plaintext: &SecretBytes,
        policy: ProtectionPolicy,
    ) -> Result<(), Self::Error> {
        let protected =
            self.codec.protect(name, plaintext, policy).map_err(SecretStoreError::Codec)?;
        self.repository.replace(name, &protected).map_err(SecretStoreError::Repository)
    }

    fn load(&self, name: &SecretName) -> Result<Option<SecretBytes>, Self::Error> {
        let protected = self.repository.load(name).map_err(SecretStoreError::Repository)?;
        protected
            .map(|value| self.codec.unprotect(name, &value).map_err(SecretStoreError::Codec))
            .transpose()
    }

    fn remove(&self, name: &SecretName) -> Result<(), Self::Error> {
        self.repository.remove(name).map_err(SecretStoreError::Repository)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, convert::Infallible};

    use super::{
        CodecBackedSecretStore, ProtectedSecret, ProtectionPolicy, SecretBytes, SecretCodec,
        SecretName, SecretRepository, SecretStore,
    };

    struct PrefixCodec;

    impl SecretCodec for PrefixCodec {
        type Error = Infallible;

        fn protect(
            &self,
            _name: &SecretName,
            plaintext: &SecretBytes,
            _policy: ProtectionPolicy,
        ) -> Result<ProtectedSecret, Self::Error> {
            let mut value = b"protected:".to_vec();
            value.extend_from_slice(plaintext.expose());
            Ok(ProtectedSecret::new(value))
        }

        fn unprotect(
            &self,
            _name: &SecretName,
            protected: &ProtectedSecret,
        ) -> Result<SecretBytes, Self::Error> {
            let value = protected.expose().strip_prefix(b"protected:").unwrap_or_default();
            Ok(SecretBytes::new(value.to_vec()))
        }
    }

    #[derive(Default)]
    struct MemoryRepository {
        value: RefCell<Option<ProtectedSecret>>,
    }

    impl SecretRepository for MemoryRepository {
        type Error = Infallible;

        fn load(&self, _name: &SecretName) -> Result<Option<ProtectedSecret>, Self::Error> {
            Ok(self.value.borrow().clone())
        }

        fn replace(
            &self,
            _name: &SecretName,
            protected: &ProtectedSecret,
        ) -> Result<(), Self::Error> {
            self.value.replace(Some(protected.clone()));
            Ok(())
        }

        fn remove(&self, _name: &SecretName) -> Result<(), Self::Error> {
            self.value.replace(None);
            Ok(())
        }
    }

    #[test]
    fn composed_store_round_trips_without_debug_exposure() {
        let store = CodecBackedSecretStore::new(PrefixCodec, MemoryRepository::default());
        let name = SecretName::new("oauth-token");
        let plaintext = SecretBytes::new(b"do-not-print".to_vec());

        assert!(store.store(&name, &plaintext, ProtectionPolicy::Host).is_ok());
        let Ok(Some(loaded)) = store.load(&name) else {
            panic!("stored value should exist");
        };

        assert_eq!(loaded.expose(), b"do-not-print");
        assert!(!format!("{loaded:?}").contains("do-not-print"));
    }
}
