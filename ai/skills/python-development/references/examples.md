# Worked examples

Start with the decision you need to understand. Full source lives in
[the example package](../examples/README.md). The modules are independent
lessons, not pieces every application should contain.

| Question | Explanation | Full source |
| --- | --- | --- |
| When is a function enough? | [Separate decisions from effects](design-choices.md#separate-decisions-from-effects) | [Count parsing](../examples/src/python_skill_examples/counts.py) |
| How should a file boundary translate failures? | [Give callers usable failures](errors-resources.md#give-callers-usable-failures) | [Count loading](../examples/src/python_skill_examples/counts.py), [tests](../examples/tests/test_counts.py) |
| How does partial setup keep cleanup? | [Own resources locally](errors-resources.md#own-resources-locally) | [ExitStack setup](../examples/src/python_skill_examples/resources.py), [tests](../examples/tests/test_resources.py) |
| How does a flush failure reach callers? | [Make completion failure visible](errors-resources.md#make-completion-failure-visible) | [Line writer](../examples/src/python_skill_examples/resources.py), [tests](../examples/tests/test_resources.py) |
| Where should a test replace an imported name? | [Use built-in isolation tools](testing.md#use-built-in-isolation-tools) | [Directory lookup](../examples/src/python_skill_examples/directory.py), [tests](../examples/tests/test_directory.py) |
| How can one suite check a fake and a real implementation? | [Check reusable fakes against real behavior](testing.md#check-reusable-fakes-against-real-behavior) | [Object stores](../examples/src/python_skill_examples/object_store.py), [tests](../examples/tests/test_object_store.py) |
| How can a model prevent invalid state changes? | [Model values and states directly](design-choices.md#model-values-and-states-directly) | [Jobs](../examples/src/python_skill_examples/jobs.py), [tests](../examples/tests/test_jobs.py) |
| How can async work stay bounded? | [Bound accepted and active work](concurrency.md#bound-accepted-and-active-work) | [Async jobs](../examples/src/python_skill_examples/async_jobs.py), [tests](../examples/tests/test_async_jobs.py) |
| How should a package expose a testable command? | [Define entry points narrowly](packaging.md#define-entry-points-narrowly) | [CLI](../examples/src/python_skill_examples/cli.py), [process tests](../examples/tests/test_cli.py) |
| What makes a useful executable docstring? | [Use doctest selectively](documentation.md#use-doctest-selectively) | [Parser doctest](../examples/src/python_skill_examples/counts.py) |

The package marker is
[`__init__.py`](../examples/src/python_skill_examples/__init__.py). It identifies
the examples package and deliberately exports no aggregate API.

The explanatory prose and example code were written for this skill. They are
not presented as copied source examples. See [sources](sources.md) for the
reference hierarchy and [evaluation](evaluation.md) for proposed checks.
