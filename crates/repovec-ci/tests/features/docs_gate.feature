Feature: Docs gate classification
  Scenario: Markdown-only changes trigger the docs gate
    Given the changed file list contains docs/roadmap.md
    When the docs gate policy is evaluated
    Then the docs gate runs
    And the docs gate reason is markdown-changed
    And the docs gate matches docs/roadmap.md

  Scenario: Code-only changes skip the docs gate
    Given the changed file list contains crates/repovec-core/src/lib.rs
    When the docs gate policy is evaluated
    Then the docs gate is skipped
    And the docs gate reason is no-markdown-changes

  Scenario: Mixed changes still trigger the docs gate
    Given the changed file list contains crates/repovec-core/src/lib.rs
    And the changed file list contains README.md
    When the docs gate policy is evaluated
    Then the docs gate runs
    And the docs gate reason is markdown-changed
    And the docs gate matches README.md

  Scenario: Missing changed-file input runs the docs gate conservatively
    Given the changed file list is unavailable
    When the docs gate policy is evaluated
    Then the docs gate runs
    And the docs gate reason is missing-changed-files
