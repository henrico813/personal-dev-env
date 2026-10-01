# Pytest mechanics

Load and follow the `behavior-focused-testing` skill for test purpose, scope,
naming, observable assertions, determinism, doubles, and regression coverage.
This reference covers Python and pytest mechanics only.

## Contents

- [Choose the test style](#choose-the-test-style)
- [Parameterize comparable cases](#parameterize-comparable-cases)
- [Keep fixtures safe](#keep-fixtures-safe)
- [Use built-in isolation tools](#use-built-in-isolation-tools)
- [Place shared fixtures deliberately](#place-shared-fixtures-deliberately)
- [Check reusable fakes against real behavior](#check-reusable-fakes-against-real-behavior)
- [Use specialized techniques for specific risks](#use-specialized-techniques-for-specific-risks)
- [Check documentation and packages when affected](#check-documentation-and-packages-when-affected)

## Choose the test style

Check whether the repository runs pytest: its configured test command, CI, and
development dependencies. When it does, write new tests in pytest style and
rewrite a touched unittest-style test to pytest. Leave untouched tests in place:

> pytest supports running Python unittest-based tests out of the box.

— [pytest, How to use unittest-based tests with pytest](https://docs.pytest.org/en/stable/how-to/unittest.html)

When it does not, keep new and touched tests in the existing unittest style.
`python -m unittest` does not collect pytest-style test functions, so a
converted test would silently stop running. Do not add pytest or change CI to
make a conversion possible; mention the limitation in the handoff instead.

## Parameterize comparable cases

Use `@pytest.mark.parametrize` when cases have the same setup, action, and
assertion shape. Use literal expected values and give each row a readable ID.
Excerpt from [test_counts.py](../examples/tests/test_counts.py):

```python
@pytest.mark.parametrize(
    ("text", "expected"),
    [
        pytest.param("1", 1, id="minimum"),
        pytest.param(" 4 \n", 4, id="whitespace"),
        pytest.param("32", 32, id="maximum"),
    ],
)
def test_parses_supported_counts(text: str, expected: int) -> None:
    assert parse_worker_count(text) == expected
```

A single case does not need a table. Split cases when setup or assertions
branch, as the read-failure and parse-failure tests in the same file do.
Parameter values are passed as-is, not copied; avoid mutation that leaks
between cases.

## Keep fixtures safe

Use fixtures to provide scoped resources or hide irrelevant construction, not
the values that explain the scenario. Prefer one state-changing acquisition per
fixture when that makes teardown reliable.

> If a yield fixture raises an exception before yielding, pytest won’t try to run the teardown code after that yield fixture’s `yield` statement.

— [pytest, Handling errors for yield fixture](https://docs.pytest.org/en/stable/how-to/fixtures.html#handling-errors-for-yield-fixture)

Split staged resources into fixtures or use context managers and finalizers so
earlier successful setup is protected. Do not imply that splitting fixtures
makes remote APIs atomic. Check cleanup failure and partial provisioning when
they are caller-visible risks.

## Use built-in isolation tools

Use `tmp_path` for isolated filesystem behavior and `monkeypatch` for temporary
attribute, environment, mapping, or path changes. Let fixture teardown restore
state. Avoid process-global changes when an explicit dependency is clearer.

When using `unittest.mock.patch`, patch the namespace where the code looks up
the name:

> The basic principle is that you patch where an object is looked up, which is not necessarily the same place as where it is defined.

— [Python documentation, Where to patch](https://docs.python.org/3/library/unittest.mock.html#where-to-patch)

The [directory example](../examples/src/python_skill_examples/directory.py)
shows both lookup-site replacement and explicit dependency supply.

## Place shared fixtures deliberately

Keep a fixture in the nearest test module when only that module uses it. Put it
in a package-level `conftest.py` when tests below that directory share it. Use a
plugin for fixtures shared across independently collected suites. Avoid importing
from `conftest.py`; pytest discovers it by directory scope.

## Check reusable fakes against real behavior

For an important reusable fake, run one small shared behavior suite against the
fake and a real implementation. Keep separate integration checks for wiring,
authentication, serialization, and behavior the fake cannot establish.

> Running the same tests against both implementations ensures both versions behave the same way.

— [PythonSpeed, Verified fakes](https://pythonspeed.com/articles/verified-fakes/#verified-fakes-testing-both-the-real-and-fake-implementation)

This assurance is limited to the represented cases. The
[object-store tests](../examples/tests/test_object_store.py) run one fixture
parameter against memory and filesystem implementations.

## Use specialized techniques for specific risks

- Use [`pytester`](https://docs.pytest.org/en/stable/how-to/writing_plugins.html#testing-plugins)
  for pytest plugin discovery, configuration, hooks, and isolated test projects.
- Use [Hypothesis stateful testing](https://hypothesis.readthedocs.io/en/latest/stateful.html)
  when failures depend on operation sequences. Use ordinary property tests when
  broad input spaces and invariants are the risk.
- Use doctest for short stable examples whose exact or configured output is part
  of their value. Volatile output remains brittle.

## Check documentation and packages when affected

With pytest doctest collection configured, run selected module or text examples
as part of `pytest`. Sphinx projects may use its doctest builder:

> If you mark the code blocks as shown here, the `doctest` builder will collect them and run them as doctest tests.

— [Sphinx, sphinx.ext.doctest](https://www.sphinx-doc.org/en/master/usage/extensions/doctest.html)

When packaging changes, build the distribution and test an installed artifact in
a clean environment. A source layout helps prevent accidental working-tree
imports, but verify the artifact rather than relying on layout alone.
