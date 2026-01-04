# scripts/

Validation automation for documentation integrity.

## Files

| File                 | What                      | When to read                                |
| -------------------- | ------------------------- | ------------------------------------------- |
| `README.md`          | Validation tool usage     | Understanding subcommands, troubleshooting  |
| `validate.py`        | Main validation script    | Implementing validation, debugging failures |
| `rule_evaluator.py`  | Template-driven engine    | Adding new validation rules                 |
| `predicates.py`      | DSL predicates            | Implementing section_exists, forbidden_text |
| `markdown_parser.py` | Markdown structure parser | Parsing sections, headings, debugging parse |

## Subdirectories

| Directory   | What          | When to read                      |
| ----------- | ------------- | --------------------------------- |
| `fixtures/` | Test fixtures | Understanding validation examples |
