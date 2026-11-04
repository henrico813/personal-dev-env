# Naming

Use `snake_case` for variables, fields, functions, methods, and modules;
`UpperCamelCase` for structs, enums, variants, and traits; and
`SCREAMING_SNAKE_CASE` for constants and statics. Treat acronyms like words:
`HttpClient`, not `HTTPClient`. Keep abbreviations conventional and unambiguous.

Name values for their meaning: `retry_count`, `output_path`, `request_timeout`.
Avoid vague names such as `data`, `temp`, or `manager` when a more specific name
exists. Short conventional names are fine in small, obvious scopes.

For example, `timeout` is ambiguous when its value is an integer. Prefer
`timeout_ms` when the representation really is milliseconds, or a `Duration`
when the code can carry the unit in its type. Do not put the type into a name
such as `report_string` unless the distinction from another representation matters.

Name actions with verbs, such as `parse_config` and `write_report`. Name boolean
queries clearly, such as `is_empty`. Ordinary getters usually omit `get_`:
`report.title()`. Follow Rust's conversion conventions: `as_` for cheap views,
`to_` for conversions that do work, and `into_` for ownership-consuming conversions.

For test names and behavioral test design, also load and follow the
`behavior-focused-testing` skill. Its seven-word maximum is required.
