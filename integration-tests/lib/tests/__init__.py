"""Unit-test suite for the shared harness helpers in :mod:`lib`.

These tests pin parsing and value-object behaviour in
:mod:`lib.assertions` (the ``PasswdEntry`` / ``FileStat`` parsers) and
:mod:`lib.result` (the ``CommandResult`` dataclass and its rendering
branches) without dragging in container or cuprum infrastructure, so a
future regression is caught before the slower behavioural suites run.
"""
