Feature: Discover repositories visible to an authenticated principal
  Scenario: A user sees personal and organization repositories across pages
    Given a user credential accepted by the repository service
    And a personal repository on the first page
    And an organization repository on a later page
    When repository listing completes
    Then both repository observations are returned in page order

  Scenario Outline: An installation sees repositories belonging to its owner
    Given an installation credential for a <owner> account
    And repositories for that account on two pages
    When repository listing completes
    Then both repository observations for that account are returned
    Examples:
      | owner        |
      | personal     |
      | organization |

  Scenario: A failed later page cannot masquerade as a complete listing
    Given a successful first page containing a repository
    And the next page rejects the credential
    When repository listing is attempted
    Then authentication failure is reported without a completed listing

  Scenario: An empty page can have a continuation
    Given an empty first page with a next link
    And a repository on the final page
    When repository listing completes
    Then the final repository is returned