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

/// Builds one repository JSON object with every field the adapter reads.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::repository_json;
///
/// let record = repository_json(1, "octocat", "hello-world");
///
/// assert!(record.contains(r#""owner":{"login":"octocat"}"#));
/// ```
#[must_use]
pub fn repository_json(id: u64, owner: &str, name: &str) -> String {
    format!(
        concat!(
            r#"{{"id":{id},"name":"{name}","private":false,"#,
            r#""default_branch":"main","owner":{{"login":"{owner}"}}}}"#
        ),
        id = id,
        name = name,
        owner = owner
    )
}

/// Builds a bare JSON array body from repository objects.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::{bare_page, repository_json};
///
/// let body = bare_page(&[repository_json(1, "octocat", "hello-world")]);
///
/// assert!(body.starts_with('['));
/// ```
#[must_use]
pub fn bare_page(items: &[String]) -> String { format!("[{}]", items.join(",")) }

/// Builds an enveloped JSON body from repository objects.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::{envelope_page, repository_json};
///
/// let body = envelope_page(&[repository_json(1, "acme", "billing")]);
///
/// assert!(body.contains(r#""total_count":1"#));
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
/// assert!(user_page(2).starts_with("/user/repos?visibility=all&"));
/// assert!(user_page(2).ends_with("&page=2"));
/// ```
#[must_use]
pub fn user_page(page: u32) -> String { page_target("/user/repos", USER_QUERY, page) }

/// Builds the path and query of one installation-mode page request.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::installation_page;
///
/// assert_eq!(
///     installation_page(2),
///     "/installation/repositories?per_page=100&page=2"
/// );
/// ```
#[must_use]
pub fn installation_page(page: u32) -> String {
    page_target("/installation/repositories", INSTALLATION_QUERY, page)
}

/// Builds a `Link` header value naming one continuation.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing_wire::link_header;
///
/// assert_eq!(
///     link_header("/user/repos?page=2", "next"),
///     r#"</user/repos?page=2>; rel="next""#
/// );
/// ```
#[must_use]
pub fn link_header(target: &str, relation: &str) -> String {
    format!("<{target}>; rel=\"{relation}\"")
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
/// assert_eq!(target.path, "/user/repos");
/// assert_eq!(
///     target.query,
///     vec![("affiliation".to_owned(), "owner,collaborator".to_owned())]
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

/// Builds a page target from a path, its fixed query, and a page number.
fn page_target(path: &str, fixed_query: &[(&str, &str)], page: u32) -> String {
    let mut target = path.to_owned();
    for (index, (key, value)) in fixed_query.iter().enumerate() {
        target.push(if index == 0 { '?' } else { '&' });
        target.push_str(key);
        target.push('=');
        target.push_str(value);
    }
    target.push_str("&page=");
    target.push_str(&page.to_string());
    target
}
