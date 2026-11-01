"""Shared fixtures for the CV-005 coverage contract tests."""

from __future__ import annotations

import pytest
from coverage_contract_support import Documents, fresh_documents


@pytest.fixture
def documents() -> Documents:
    """Give each coverage contract test its own copy of the workflows."""
    return fresh_documents()
