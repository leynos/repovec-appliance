"""Property-based contracts for the build-standard probe helpers.

The probe helpers walk parsed TOML, join Make's continued recipe lines, and
classify CI run blocks. Example-based tests cover the shapes the repository
uses today; these properties cover the shapes it could use tomorrow, with a
small independent reference implementation for each walker.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shlex

from hypothesis import given, settings, strategies as st
from hypothesis.strategies import DrawFn

from build_standard_probe import (
    COMPILING_CARGO_SUBCOMMANDS,
    COMPILING_MAKE_TARGETS,
    cargo_subcommands,
    iter_keys,
    iter_strings,
    join_continuations,
    make_targets,
    run_value_compiles,
    shell_tokens,
)

#: Keep the suite quick: these properties are structural, not statistical.
SETTINGS = settings(max_examples=50, deadline=None)

#: Scalars a parsed TOML document can hold. Floats exclude NaN and infinity
#: because the walkers only care about container shape.
_SCALARS = st.one_of(
    st.none(),
    st.booleans(),
    st.integers(),
    st.floats(allow_nan=False, allow_infinity=False),
    st.text(),
)

_NESTED = st.recursive(
    _SCALARS,
    lambda children: st.one_of(
        st.lists(children, max_size=4),
        st.dictionaries(st.text(max_size=5), children, max_size=4),
    ),
    max_leaves=20,
)


def reference_keys(
    value: object, path: tuple[object, ...] = ()
) -> list[tuple[object, ...]]:
    """Return every mapping key with its path, independently of the probe."""
    if isinstance(value, dict):
        return _mapping_key_paths(value, path)
    if isinstance(value, list):
        return _sequence_key_paths(value, path)
    return []


def _mapping_key_paths(
    mapping: dict[object, object], path: tuple[object, ...]
) -> list[tuple[object, ...]]:
    """Visit each key before descending into its value, in insertion order."""
    return [
        descendant
        for key, child in mapping.items()
        for descendant in ((*path, key), *reference_keys(child, (*path, key)))
    ]


def _sequence_key_paths(
    items: list[object], path: tuple[object, ...]
) -> list[tuple[object, ...]]:
    """Descend into each element in index order without recording indices."""
    return [
        descendant
        for index, child in enumerate(items)
        for descendant in reference_keys(child, (*path, index))
    ]


def reference_strings(value: object) -> list[str]:
    """Return every string value, independently of the probe."""
    if isinstance(value, dict):
        return [found for child in value.values() for found in reference_strings(child)]
    if isinstance(value, list):
        return [found for child in value for found in reference_strings(child)]
    return [value] if isinstance(value, str) else []


@SETTINGS
@given(_NESTED)
def test_iter_keys_matches_a_reference_walker(value: object) -> None:
    assert iter_keys(value) == reference_keys(value)


@SETTINGS
@given(_NESTED)
def test_iter_strings_matches_a_reference_walker(value: object) -> None:
    assert iter_strings(value) == reference_strings(value)


#: Tokens may contain shell metacharacters, spaces and quotes. Newlines and
#: NUL are excluded because the helpers are line-oriented.
_TOKEN = st.text(
    alphabet=st.characters(blacklist_characters="\n\r\x00"),
    max_size=8,
)


def continued_lines(groups: list[list[str]]) -> list[str]:
    """Render token groups as Make-style continued recipe lines."""
    return [line for group in groups for line in _group_lines(group)]


def _group_lines(group: list[str]) -> list[str]:
    """Render one group as its chunks, or a single blank line when empty."""
    if not group:
        return [""]
    chunks = [group[index : index + 2] for index in range(0, len(group), 2)]
    return [
        _chunk_line(chunk, is_last=position == len(chunks) - 1)
        for position, chunk in enumerate(chunks)
    ]


def _chunk_line(chunk: list[str], *, is_last: bool) -> str:
    """Quote one token chunk, marking all but the last as continued."""
    text = " ".join(shlex.quote(token) for token in chunk)
    return text if is_last else text + " \\"


@SETTINGS
@given(st.lists(st.lists(_TOKEN, max_size=4), max_size=4))
def test_join_continuations_preserves_tokens(groups: list[list[str]]) -> None:
    """Scenario: a recipe is split across continued lines.

    Invariant: joining restores one line per logical command, the tokens
    survive shell parsing unchanged, and no joined line ends with a
    continuation marker.
    """
    joined = join_continuations(continued_lines(groups))
    expected = [group for group in groups if group]

    assert len(joined) == len(expected)
    for line, group in zip(joined, expected):
        assert shell_tokens(line) == group
    assert all(not line.endswith("\\") for line in joined)


@SETTINGS
@given(st.lists(st.text(max_size=10), max_size=10))
def test_join_continuations_is_idempotent(lines: list[str]) -> None:
    once = join_continuations(lines)

    assert join_continuations(once) == once
    assert all(line.strip() for line in once)


#: Commands the generated run blocks may contain. One command per line keeps
#: the reference predicate simple: the probe's regexes find exactly one match.
_MAKE_TARGETS = ("build", "test", "lint", "spelling", "release", "fmt")
_CARGO_SUBCOMMANDS = ("build", "test", "clippy", "fmt", "metadata", "run")


@st.composite
def run_values(draw: DrawFn) -> tuple[str, list[str], list[str]]:
    """Generate a run block and the targets and subcommands it contains."""
    lines: list[str] = []
    expected_make: list[str] = []
    expected_cargo: list[str] = []
    for _ in range(draw(st.integers(min_value=0, max_value=5))):
        kind = draw(st.sampled_from(("make", "cargo", "comment", "blank")))
        if kind == "make":
            target = draw(st.sampled_from(_MAKE_TARGETS))
            lines.append(f"make {target}")
            expected_make.append(target)
        elif kind == "cargo":
            subcommand = draw(st.sampled_from(_CARGO_SUBCOMMANDS))
            lines.append(f"cargo {subcommand}")
            expected_cargo.append(subcommand)
        elif kind == "comment":
            lines.append("# make build && cargo test")
        else:
            lines.append("")
    return "\n".join(lines), expected_make, expected_cargo


@SETTINGS
@given(run_values())
def test_make_targets_matches_a_reference_predicate(
    case: tuple[str, list[str], list[str]],
) -> None:
    run_value, expected_make, _ = case

    assert make_targets(run_value) == expected_make


@SETTINGS
@given(run_values())
def test_cargo_subcommands_matches_a_reference_predicate(
    case: tuple[str, list[str], list[str]],
) -> None:
    run_value, _, expected_cargo = case

    assert cargo_subcommands(run_value) == expected_cargo


@SETTINGS
@given(run_values())
def test_run_value_compiles_matches_a_reference_predicate(
    case: tuple[str, list[str], list[str]],
) -> None:
    run_value, expected_make, expected_cargo = case
    expected = any(target in COMPILING_MAKE_TARGETS for target in expected_make) or any(
        subcommand in COMPILING_CARGO_SUBCOMMANDS for subcommand in expected_cargo
    )

    assert run_value_compiles(run_value) is expected


@SETTINGS
@given(
    st.one_of(
        st.none(),
        st.integers(),
        st.lists(st.text()),
        st.dictionaries(st.text(), st.text()),
    )
)
def test_non_string_run_values_are_inert(value: object) -> None:
    assert make_targets(value) == []
    assert cargo_subcommands(value) == []
    assert run_value_compiles(value) is False
