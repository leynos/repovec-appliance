# Environment-policy scan samples

Sample sources for `environment_policy_source_scan.rs`. Each is loaded with
`include_str!` and handed to the scan as text; none is compiled.

The `.rs.txt` extension is deliberate. A `.rs` file here would be picked up by
the workspace scan itself and reported as an offence, which is the one thing
the scan must not do to its own fixtures.
