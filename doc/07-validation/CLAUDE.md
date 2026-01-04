# 07-validation/

4-layer validation architecture across UI, API, Runtime, and Database.

## Files

| File                       | What                            | When to read                                 |
| -------------------------- | ------------------------------- | -------------------------------------------- |
| `README.md`                | Validation hub                  | Understanding layer distribution, scope      |
| `responsibility-matrix.md` | Layer responsibility mapping    | Determining which layer enforces what        |
| `ui-validation.md`         | HTML5 form validation           | Implementing ARIA accessibility, forms       |
| `api-validation.md`        | API layer enforcement           | Implementing structured error responses      |
| `input-sanitization.md`    | OWASP injection prevention      | Implementing UTF-8, HTML escape, SQL safety  |
| `runtime-validation.md`    | Type coercion, field resolution | Implementing on_missing_field, buffer limits |
| `database-validation.md`   | Database constraints            | Implementing foreign keys, migrations        |
