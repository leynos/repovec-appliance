Feature: Docs gate classification
  Scenario: Markdown-only changes trigger the docs gate
    Given the changed file list contains docs/roadmap.md
    When the docs gate policy is evaluated
    Then the docs gate runs
    And Mermaid validation is skipped
    And the docs gate reason is documentation-changed
    And the docs gate matches docs/roadmap.md

  Scenario: Code-only changes skip the docs gate
    Given the changed file list contains crates/repovec-core/src/lib.rs
    When the docs gate policy is evaluated
    Then the docs gate is skipped
    And Mermaid validation is skipped
    And the docs gate reason is no-documentation-changes

  Scenario: Mixed changes still trigger the docs gate
    Given the changed file list contains crates/repovec-core/src/lib.rs
    And the changed file list contains README.md
    When the docs gate policy is evaluated
    Then the docs gate runs
    And Mermaid validation is skipped
    And the docs gate reason is documentation-changed
    And the docs gate matches README.md

  Scenario: Documentation tooling changes trigger the docs gate conservatively
    Given the changed file list contains .markdownlint-cli2.jsonc
    When the docs gate policy is evaluated
    Then the docs gate runs
    And Mermaid validation is required
    And the docs gate reason is documentation-changed
    And the docs gate matches .markdownlint-cli2.jsonc
    And the conservative fallback count is 0

  Scenario: Mermaid-bearing docs changes require nixie
    Given the changed file list contains docs/users-guide.md
    And the Mermaid-bearing file is docs/users-guide.md
    When the docs gate policy is evaluated
    Then the docs gate runs
    And Mermaid validation is required
    And the docs gate reason is documentation-changed
    And the docs gate matches docs/users-guide.md

  Scenario: Unreadable changed Markdown triggers conservative fallback
    Given the changed file list contains docs/users-guide.md
    And the unreadable Markdown file is docs/users-guide.md
    When the docs gate policy is evaluated
    Then the docs gate runs
    And Mermaid validation is required
    And the docs gate reason is documentation-changed
    And the conservative fallback count is 1
    And the conservative fallback list contains docs/users-guide.md

  Scenario: Missing changed-file input runs the docs gate conservatively
    Given the changed file list is unavailable
    When the docs gate policy is evaluated
    Then the docs gate runs
    And Mermaid validation is required
    And the docs gate reason is missing-changed-files
