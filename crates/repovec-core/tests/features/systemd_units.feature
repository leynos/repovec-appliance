Feature: repovec systemd unit contract
  The appliance ships checked-in systemd units for its managed service group.

  Scenario: The checked-in unit set satisfies the appliance contract
    Given the checked-in repovec systemd units
    When the systemd units are validated
    Then the systemd unit set is accepted

  Scenario: The target wants every appliance service
    Given the checked-in repovec systemd units
    And cloudflared is removed from the target wants list
    When the systemd units are validated
    Then validation fails because the target does not want cloudflared

  Scenario: The target remains enableable
    Given the checked-in repovec systemd units
    And the target install binding is removed
    When the systemd units are validated
    Then validation fails because the target is not wanted by multi-user

  Scenario: Semicolon comments are accepted
    Given the checked-in repovec systemd units
    And a semicolon comment is added to the target
    When the systemd units are validated
    Then the systemd unit set is accepted

  Scenario: repovecd waits for Qdrant
    Given the checked-in repovec systemd units
    And the repovecd Qdrant ordering is removed
    When the systemd units are validated
    Then validation fails because repovecd does not start after Qdrant

  Scenario: repovec-mcpd waits for the control-plane daemon
    Given the checked-in repovec systemd units
    And the repovec-mcpd repovecd requirement is removed
    When the systemd units are validated
    Then validation fails because repovec-mcpd does not require repovecd

  Scenario: The generated Qdrant service name is required
    Given the checked-in repovec systemd units
    And repovecd requires qdrant.container instead of qdrant.service
    When the systemd units are validated
    Then validation fails because the Quadlet source name was used

  Scenario: repovecd keeps the appliance service identity
    Given the checked-in repovec systemd units
    And repovecd runs as root instead of repovec
    When the systemd units are validated
    Then validation fails because repovecd has the wrong service user

  Scenario: repovec-mcpd keeps the appliance home environment
    Given the checked-in repovec systemd units
    And the repovec-mcpd home environment is removed
    When the systemd units are validated
    Then validation fails because repovec-mcpd has no appliance home environment
