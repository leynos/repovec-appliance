//! Compile-fail fixture proving the validated repository identity newtypes
//! cannot be built or read around their validating constructors.

use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner};

fn main() {
    let _constructed_id = RepositoryId(1);
    let _constructed_owner = RepositoryOwner("octocat".to_owned());
    let _constructed_name = RepositoryName("hello-world".to_owned());

    if let Ok(owner) = RepositoryOwner::new("octocat") {
        let _raw_owner = owner.0;
    }
}