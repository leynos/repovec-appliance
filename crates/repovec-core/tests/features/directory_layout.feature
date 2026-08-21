Feature: repovec directory-layout contract
  The appliance ships checked-in sysusers.d and tmpfiles.d assets that
  provision the repovec user and its private directory tree.

  Scenario: The checked-in layout assets satisfy the appliance contract
    Given the checked-in repovec layout assets
    When the directory-layout assets are validated
    Then the directory-layout asset set is accepted

  Scenario: The tmpfiles asset must declare every required data directory
    Given the checked-in repovec layout assets
    And the worktrees directory entry is removed from the tmpfiles asset
    When the directory-layout assets are validated
    Then validation fails because a required directory entry is missing

  Scenario: The tmpfiles asset must not declare unexpected directories
    Given the checked-in repovec layout assets
    And an unexpected directory entry is added to the tmpfiles asset
    When the directory-layout assets are validated
    Then validation fails because an unexpected directory entry is present

  Scenario: Data directories must be private to the repovec user
    Given the checked-in repovec layout assets
    And the worktrees directory mode is widened to 0750
    When the directory-layout assets are validated
    Then validation fails because the directory mode is incorrect

  Scenario: Data directories must be owned by the repovec user
    Given the checked-in repovec layout assets
    And the git-mirrors directory owner is changed to root
    When the directory-layout assets are validated
    Then validation fails because the directory owner is incorrect

  Scenario: A data directory group must be repovec
    Given the checked-in repovec layout assets
    And the grepai directory group is changed to wheel
    When the directory-layout assets are validated
    Then validation fails because the directory group is incorrect

  Scenario: Directory entries must set explicit modes and ownership
    Given the checked-in repovec layout assets
    And the worktrees directory mode is replaced with the default token
    When the directory-layout assets are validated
    Then validation fails because a required field is not explicit

  Scenario: The tmpfiles asset must not declare the secrets directory
    Given the checked-in repovec layout assets
    And an /etc/repovec entry is added to the tmpfiles asset
    When the directory-layout assets are validated
    Then validation fails because the secrets directory has a single authority

  Scenario: A malformed tmpfiles line is rejected
    Given the checked-in repovec layout assets
    And a malformed line is inserted into the tmpfiles asset
    When the directory-layout assets are validated
    Then validation fails because a line is malformed

  Scenario: The sysusers asset must declare the repovec user home
    Given the checked-in repovec layout assets
    And the sysusers home path is changed away from /var/lib/repovec
    When the directory-layout assets are validated
    Then validation fails because the sysusers home is incorrect

  Scenario: The sysusers asset must keep the nologin shell
    Given the checked-in repovec layout assets
    And the sysusers shell is changed to /bin/bash
    When the directory-layout assets are validated
    Then validation fails because the sysusers shell is incorrect
