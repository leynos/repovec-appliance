//! GitHub repository wire fixtures and page targets for listing tests.
//!
//! These builders produce the JSON GitHub documents and the request targets
//! the listing profiles send, so a test states only the page partition it is
//! about. They live here rather than in a test module because every listing
//! integration test binary needs them, and a library crate keeps shared
//! fixtures free of per-binary dead-code noise.

/// The fake user token every user-mode listing test presents.
///
/// These are deliberately fictive: no test reaches GitHub, and a real token
/// must never appear in a request or a log assertion.
pub const USER_TOKEN: &str = "gho_fake_user_token";

/// The fake installation token every installation-mode listing test presents.
pub const INSTALLATION_TOKEN: &str = "ghs_fake_installation_token";

/// The fixed query the user repository profile sends, in request order.
pub const USER_QUERY: &[(&str, &str)] = &[
    ("visibility", "all"),
    ("affiliation", "owner,collaborator,organization_member"),
    ("sort", "full_name"),
    ("direction", "asc"),
    ("per_page", "100"),
];

/// The fixed query the installation repository profile sends.
pub const INSTALLATION_QUERY: &[(&str, &str)] = &[("per_page", "100")];

/// One repository record a wire fixture describes.
///
/// A record names three things the adapter reads, so a test passes them as one
/// value rather than as a positional run of a number and two strings.
#[derive(Clone, Copy, Debug)]
pub struct Repository<'a> {
    /// Identifier GitHub reported.
    pub id: u64,
    /// Login of the owner GitHub reported.
    pub owner: &'a str,
    /// Repository name GitHub reported.
    pub name: &'a str,
}

/// A relation name a test's `Link` header carries.
///
/// The variants cover the relation lists the listing contract distinguishes;
/// [`LinkRelation::NextThenLast`] is the space-separated list GitHub sends for
/// a page that is not the last one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkRelation {
    /// The next continuation of the listing.
    Next,
    /// The last page of the listing.
    Last,
    /// A relation list naming next before last.
    NextThenLast,
}

impl LinkRelation {
    /// Returns the `rel` parameter text this relation writes.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Last => "last",
            Self::NextThenLast => "next last",
        }
    }
}

/// Builds one repository JSON object with every field the adapter reads.
///
/// The object is serialized rather than formatted, so a name or login that
/// carries a quote, backslash or control character still produces the JSON
/// GitHub would send. Object keys are written in the serializer's sorted
/// order; no test depends on the field order of this fixture.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::{Repository, repository_json};
///
/// let record = repository_json(Repository {
///     id: 1,
///     owner: "octocat",
///     name: "hello-world",
/// });
///
/// assert!(
///     record.contains(r#""owner":{"login":"octocat"}"#),
///     "the owner login should be nested under its owner object"
/// );
///
/// let quoted = repository_json(Repository {
///     id: 2,
///     owner: "acme",
///     name: r#"a"b"#,
/// });
/// assert!(
///     quoted.contains(r#""name":"a\"b""#),
///     "a quote in the name should be escaped rather than end the string"
/// );
/// ```
#[must_use]
pub fn repository_json(repository: Repository<'_>) -> String {
    let Repository { id, owner, name } = repository;
    serde_json::json!({
        "id": id,
        "name": name,
        "private": false,
        "default_branch": "main",
        "owner": { "login": owner },
    })
    .to_string()
}

/// Builds a bare JSON array body from repository objects.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::{Repository, bare_page, repository_json};
///
/// let body = bare_page(&[repository_json(Repository {
///     id: 1,
///     owner: "octocat",
///     name: "hello-world",
/// })]);
///
/// assert!(
///     body.starts_with('['),
///     "a bare page should be a JSON array, got {body}"
/// );
/// ```
#[must_use]
pub fn bare_page(items: &[String]) -> String { format!("[{}]", items.join(",")) }

/// Builds an enveloped JSON body from repository objects.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::{
///     Repository,
///     envelope_page,
///     repository_json,
/// };
///
/// let body = envelope_page(&[repository_json(Repository {
///     id: 1,
///     owner: "acme",
///     name: "billing",
/// })]);
///
/// assert!(
///     body.contains(r#""total_count":1"#),
///     "the envelope should count its single record, got {body}"
/// );
/// ```
#[must_use]
pub fn envelope_page(items: &[String]) -> String {
    format!(
        r#"{{"total_count":{},"repositories":[{}]}}"#,
        items.len(),
        items.join(",")
    )
}

/// Builds the path and query of one user-mode page request.
///
/// A continuation keeps every fixed filter, so this is also the target a
/// scripted `next` link must name.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::user_page;
///
/// assert!(
///     user_page(2).starts_with("/user/repos?visibility=all&"),
///     "a user page should start with the profile's path and fixed filters"
/// );
/// assert!(
///     user_page(2).ends_with("&page=2"),
///     "a user page should end with the requested page number"
/// );
/// ```
#[must_use]
pub fn user_page(page: u32) -> String { page_target(USER_PROFILE, page) }

/// Builds the path and query of one installation-mode page request.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::installation_page;
///
/// assert_eq!(
///     installation_page(2),
///     "/installation/repositories?per_page=100&page=2",
///     "an installation page should carry the installation path and query"
/// );
/// ```
#[must_use]
pub fn installation_page(page: u32) -> String { page_target(INSTALLATION_PROFILE, page) }

/// Builds a `Link` header value naming one continuation.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::{LinkRelation, link_header};
///
/// assert_eq!(
///     link_header("/user/repos?page=2", LinkRelation::Next),
///     r#"</user/repos?page=2>; rel="next""#,
///     "a next link should wrap its target and name the next relation"
/// );
/// ```
#[must_use]
pub fn link_header(target: &str, relation: LinkRelation) -> String {
    format!("<{target}>; rel=\"{}\"", relation.as_str())
}

/// A request target split into its path and decoded query parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTarget {
    /// The request path, without the query string.
    pub path: String,
    /// Query parameters in received order, percent-decoded.
    pub query: Vec<(String, String)>,
}

/// Decodes a request target's path and query parameters.
///
/// Comparing decoded pairs makes a test about protocol meaning rather than
/// about the client library's percent-encoding choices.
///
/// # Errors
///
/// Returns the URL parser's error when the target is not a valid relative URL
/// path. An adapter bug could produce such a target, so this stays fallible
/// rather than panicking inside a shared fixture.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::decode_target;
///
/// let target = decode_target("/user/repos?affiliation=owner%2Ccollaborator")?;
///
/// assert_eq!(
///     target.path, "/user/repos",
///     "the decoded path should keep the request path"
/// );
/// assert_eq!(
///     target.query,
///     vec![("affiliation".to_owned(), "owner,collaborator".to_owned())],
///     "the query should be decoded into its key and value"
/// );
///
/// # Ok::<(), url::ParseError>(())
/// ```
pub fn decode_target(target: &str) -> Result<DecodedTarget, url::ParseError> {
    let url = url::Url::parse(&format!("http://placeholder{target}"))?;
    let query = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    Ok(DecodedTarget {
        path: url.path().to_owned(),
        query,
    })
}

/// The request shape one listing profile sends for every page.
#[derive(Clone, Copy, Debug)]
struct PageProfile {
    /// Path of every page request.
    path: &'static str,
    /// Query fixed for the whole traversal, in request order.
    fixed_query: &'static [(&'static str, &'static str)],
}

/// The user repository profile.
const USER_PROFILE: PageProfile = PageProfile {
    path: "/user/repos",
    fixed_query: USER_QUERY,
};

/// The installation repository profile.
const INSTALLATION_PROFILE: PageProfile = PageProfile {
    path: "/installation/repositories",
    fixed_query: INSTALLATION_QUERY,
};

/// Builds a page target from a profile and a page number.
fn page_target(profile: PageProfile, page: u32) -> String {
    let mut target = profile.path.to_owned();
    for (index, (key, value)) in profile.fixed_query.iter().enumerate() {
        target.push(if index == 0 { '?' } else { '&' });
        target.push_str(key);
        target.push('=');
        target.push_str(value);
    }
    target.push_str("&page=");
    target.push_str(&page.to_string());
    target
}
