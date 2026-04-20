Feature: Qdrant Quadlet contract
  The appliance ships a checked-in Quadlet for the local Qdrant service.

  Scenario: The checked-in Quadlet satisfies the appliance contract
    Given the checked-in Qdrant Quadlet
    When the Quadlet is validated
    Then the Quadlet is accepted

  Scenario: The REST port remains loopback-only
    Given the checked-in Qdrant Quadlet
    And the REST port is published on 0.0.0.0
    When the Quadlet is validated
    Then the validation fails with a loopback error for port 6333

  Scenario: The gRPC port must be present
    Given the checked-in Qdrant Quadlet
    And the gRPC port mapping is removed
    When the Quadlet is validated
    Then the validation fails because the gRPC port is missing

  Scenario: Persistent storage remains mounted
    Given the checked-in Qdrant Quadlet
    And the persistent storage mount is removed
    When the Quadlet is validated
    Then the validation fails because the storage mount is missing

  Scenario: Podman auto-update remains enabled
    Given the checked-in Qdrant Quadlet
    And Podman auto-update is removed
    When the Quadlet is validated
    Then the validation fails because auto-update is missing
