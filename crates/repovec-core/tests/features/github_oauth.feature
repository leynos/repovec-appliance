Feature: GitHub OAuth device flow
  The appliance interprets GitHub device-flow polling outcomes deterministically.

  Scenario: Authorization completes with a token
    Given the active polling interval is 5 seconds
    When the token endpoint returns an access token
    Then polling completes successfully

  Scenario: Slow down increases the next poll delay
    Given the active polling interval is 5 seconds
    When the token endpoint asks the client to slow down
    Then the next polling interval is 10 seconds

  Scenario: Access denied is terminal
    Given the active polling interval is 5 seconds
    When the token endpoint reports access denied
    Then polling fails because access was denied

  Scenario: Expired token is terminal
    Given the active polling interval is 5 seconds
    When the token endpoint reports an expired token
    Then polling fails because the device code expired

  Scenario: Unsupported OAuth errors are not device-flow outcomes
    Given the OAuth error is temporarily_unavailable
    When the OAuth error is classified
    Then no device-flow polling outcome is produced
