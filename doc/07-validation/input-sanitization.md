---
doc_type: spoke
status: active
date_created: 2025-11-10
primary_category: validation
hub_document: /Users/lmergen/git/trapperkeeper/doc/07-validation/README.md
tags:
  - input-sanitization
  - security
  - owasp
  - injection-prevention
---

# Input Sanitization

## Context

Input sanitization is security-critical for preventing injection attacks and ensuring data integrity. This document consolidates OWASP security patterns with validation domain architecture clarifications, providing comprehensive specifications for UTF-8 validation, HTML escaping, SQL injection prevention, command injection prevention, and path traversal prevention.

The need for explicit input sanitization arises from two distinct validation domains in TrapperKeeper:

- **Domain 1 (Internal System Schemas - STRICT)**: tk-types, database schemas, rule definitions are strictly validated at API layer with zero tolerance for malformed data
- **Domain 2 (External User Data - WEAK, BEST-EFFORT)**: Incoming events, sensor data require sanitization with graceful degradation for operational resilience

This clarification is critical for greenfield projects to be explicit about where OWASP sanitization applies (primarily Domain 2 external data at ingestion boundaries, while Domain 1 applies strict structural validation).

**Hub Document**: This document is part of the Validation Architecture. See Validation Hub for strategic overview and relationships to other validation layers.

## Five Core Security Controls

Input sanitization enforces five security controls following OWASP Input Validation Cheat Sheet guidelines. All controls are enforced at the API layer as the primary security boundary, with defense-in-depth re-validation at runtime.

### Control 1: UTF-8 Validation

Reject control characters that can cause rendering issues, logging vulnerabilities, and injection attacks.

**Control 2: HTML Escaping**

Prevent cross-site scripting (XSS) attacks through automatic context-appropriate escaping.

**Control 3: SQL Injection Prevention**

Prevent SQL injection through parameterized queries separating SQL structure from user data.

**Control 4: Command Injection Prevention**

Eliminate command injection risk by architectural constraint (never spawn processes).

**Control 5: Path Traversal Prevention**

Prevent unauthorized file access through path canonicalization and boundary validation.

**Cross-References**:

- Responsibility Matrix: Layer enforcement assignments for input sanitization
- API Validation: API layer enforcement patterns
- Runtime Validation: Defense-in-depth re-validation patterns

## UTF-8 Validation (Control 1)

### Requirement

All user input must be valid UTF-8 with additional control character filtering per OWASP Input Validation Cheat Sheet.

### Validation Rules

1. **Valid UTF-8 Encoding**: Follow Go's string requirements; reject invalid UTF-8 sequences and surrogates
2. **Control Character Filtering**: Reject control characters 0x00-0x1F EXCEPT:
   - 0x09 (tab)
   - 0x0A (line feed)
   - 0x0D (carriage return)
3. **No Null Bytes**: Explicitly reject 0x00 (null byte) in all string inputs

### Rationale

Control characters can cause:

- Rendering issues in web browsers
- Log injection vulnerabilities (newline injection, log forging)
- Terminal injection attacks (escape sequences, ANSI codes)
- String termination issues in C-based libraries

Tab, LF, and CR are allowed for formatting in multi-line text fields (rule descriptions, comments).

### Layer Distribution

- **API Layer (Primary)**: Validate all string inputs using UTF-8 validation function before processing; return 422 error with clear message if validation fails
- **Runtime Layer**: Re-validate user input at processing time (defense in depth) in both SDK execution and server processing
- **UI Layer**: Not applicable (validation performed server-side)
- **Database Layer**: Store validated UTF-8 text with TEXT column type

### Implementation

```go
import (
    "fmt"
    "unicode/utf8"
)

// ValidateUTF8Input validates UTF-8 string with control character filtering
func ValidateUTF8Input(input string) error {
    // Go strings are already valid UTF-8 by construction
    // Check for disallowed control characters
    for i, ch := range input {
        code := uint32(ch)
        if code <= 0x1F && code != 0x09 && code != 0x0A && code != 0x0D {
            return &ValidationError{
                Type:     "InvalidUTF8",
                Position: i,
                Character: code,
                Message:  fmt.Sprintf("Control character 0x%02X not allowed (except tab/LF/CR)", code),
            }
        }
    }
    return nil
}
```

### Examples

**Valid Input**:

```go
"sensor_name: \"Temperature-01\""  // Valid UTF-8, no control chars
"description: \"Line 1\nLine 2\""  // Newline allowed for multi-line text
"comment: \"Indented\twith\ttabs\""  // Tabs allowed for formatting
```

**Invalid Input**:

```go
"sensor_name: \"Temp\x00Sensor\""  // Null byte 0x00
"sensor_name: \"Temp\x1BSensor\""  // Escape character 0x1B (ANSI escape)
"log_entry: \"User input\x07\""    // Bell character 0x07
```

### Error Response

API returns 422 Unprocessable Entity with field-specific error:

```json
{
  "error": "Validation failed",
  "code": "VALIDATION_ERROR",
  "request_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "details": [
    {
      "field": "rule_name",
      "message": "Control character 0x1B not allowed (except tab/LF/CR)",
      "code": "INVALID_UTF8",
      "position": 4
    }
  ]
}
```

**Cross-References**:

- OWASP Input Validation Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html
- API Validation: Structured error response format

## HTML Escaping (Control 2)

### Requirement

Use html/template for automatic context-appropriate HTML escaping. NEVER construct HTML manually from user input.

### Validation Rules

1. **html/template Auto-Escaping**: All user input rendered through html/template with auto-escaping enabled (default)
2. **No Manual HTML Construction**: NEVER use fmt.Sprintf() or string concatenation to build HTML containing user input
3. **Context-Appropriate Escaping**: html/template automatically selects correct escaping based on context (HTML, JavaScript, URL)

### Rationale

Manual HTML construction is error-prone and creates cross-site scripting (XSS) vulnerabilities. html/template provides:

- Parse-time template validation
- Automatic escaping based on context
- Type-safe template execution
- Secure by default

### Layer Distribution

- **UI Layer (Primary)**: Use html/template for all HTML rendering in web-ui service; code review enforces no manual HTML construction
- **API Layer**: Not applicable (API returns JSON, not HTML)
- **Runtime Layer**: Not applicable (no HTML rendering in runtime)
- **Database Layer**: Store raw user input; escaping performed at render time

### Implementation

All web UI templates use html/template with auto-escaping enabled:

```go
import "html/template"

type RuleDetailTemplate struct {
    RuleName    string  // Automatically escaped by html/template
    Description string  // Automatically escaped by html/template
    CreatedBy   string  // Automatically escaped by html/template
}

tmpl, err := template.ParseFiles("rule_detail.html")
if err != nil {
    return err
}
err = tmpl.Execute(w, data)
```

### Examples

**Correct Usage (html/template Auto-Escaping)**:

```html
<!-- rule_detail.html template -->
<h1>{{ .RuleName }}</h1>
<p>{{ .Description }}</p>
<span>Created by: {{ .CreatedBy }}</span>
```

**User Input**:

```
rule_name: "Test<script>alert(1)</script>"
description: "<img src=x onerror=alert(1)>"
created_by: "admin' OR '1'='1"
```

**Rendered Output** (safe, escaped):

```html
<h1>Test&lt;script&gt;alert(1)&lt;/script&gt;</h1>
<p>&lt;img src=x onerror=alert(1)&gt;</p>
<span>Created by: admin&#39; OR &#39;1&#39;=&#39;1</span>
```

**Incorrect Usage (Manual HTML Construction - FORBIDDEN)**:

```go
// FORBIDDEN: Manual HTML construction creates XSS vulnerability
fmt.Sprintf("<h1>%s</h1>", ruleName)  // XSS vulnerability
fmt.Sprintf("<p>%s</p>", description)  // XSS vulnerability
```

### Code Review Checklist

- [ ] All user input rendered through html/template
- [ ] No fmt.Sprintf() or string concatenation used to build HTML
- [ ] No manual HTML construction
- [ ] html/template auto-escaping enabled (default, never disabled)

**Cross-References**:

- Web Framework: html/template specifications
- UI Validation: Form validation and error display patterns

## SQL Injection Prevention (Control 3)

### Requirement

Use database/sql parameterized queries EXCLUSIVELY. NEVER use fmt.Sprintf() or string concatenation for SQL construction.

### Validation Rules

1. **Parameterized Queries Only**: All database queries use database/sql with `?` placeholders
2. **No String Concatenation**: NEVER use fmt.Sprintf() or string concatenation to build SQL queries
3. **Code Review Enforcement**: Manual code review rejects any SQL string construction

### Rationale

Parameterized queries prevent SQL injection by:

- Separating SQL structure from user data
- Automatically escaping special characters
- Runtime SQL validation
- Type-safe query parameters

String concatenation creates injection vulnerabilities because user input can contain SQL metacharacters (quotes, semicolons, comments) that alter query structure.

### Layer Distribution

- **Database Layer (Primary)**: Use database/sql parameterized queries EXCLUSIVELY in all database operations; code review enforces this pattern
- **API Layer**: Not applicable (database/sql used in database layer, not API layer)
- **Runtime Layer**: Not applicable (no SQL construction in runtime)
- **UI Layer**: Not applicable (no direct database access from UI)

### Implementation

All database operations use database/sql parameterized queries:

```go
import "database/sql"

// Correct: Parameterized query with ? placeholder
func GetRuleByName(db *sql.DB, name string) (*Rule, error) {
    var rule Rule
    err := db.QueryRow("SELECT rule_id, name, description FROM rules WHERE name = ?", name).
        Scan(&rule.RuleID, &rule.Name, &rule.Description)
    if err != nil {
        return nil, err
    }
    return &rule, nil
}

// Correct: Multiple parameters
func CreateRule(db *sql.DB, name, description string) (string, error) {
    ruleID := generateUUID()
    _, err := db.Exec(
        "INSERT INTO rules (rule_id, name, description) VALUES (?, ?, ?)",
        ruleID,
        name,
        description,
    )
    if err != nil {
        return "", err
    }
    return ruleID, nil
}
```

### Examples

**Correct Usage (Parameterized Queries)**:

```go
// SELECT with user input
db.QueryRow("SELECT * FROM rules WHERE name = ?", userInput)

// INSERT with multiple parameters
db.Exec(
    "INSERT INTO rules (rule_id, name, description) VALUES (?, ?, ?)",
    ruleID, name, description,
)

// UPDATE with user input
db.Exec("UPDATE rules SET description = ? WHERE rule_id = ?", desc, id)

// DELETE with user input
db.Exec("DELETE FROM rules WHERE rule_id = ?", ruleID)
```

**Incorrect Usage (String Concatenation - FORBIDDEN)**:

```go
// FORBIDDEN: fmt.Sprintf() creates SQL injection vulnerability
db.Query(fmt.Sprintf("SELECT * FROM rules WHERE name = '%s'", userInput))

// FORBIDDEN: String concatenation
db.Query("SELECT * FROM rules WHERE name = '" + userInput + "'")

// FORBIDDEN: Manual escaping is error-prone
escaped := strings.ReplaceAll(userInput, "'", "''")  // Still vulnerable
db.Query(fmt.Sprintf("SELECT * FROM rules WHERE name = '%s'", escaped))
```

### Attack Example (Why String Concatenation Fails)

**Vulnerable Code**:

```go
// VULNERABLE
query := fmt.Sprintf("SELECT * FROM rules WHERE name = '%s'", userInput)
db.Query(query)
```

**Malicious Input**:

```
userInput = "'; DROP TABLE rules; --"
```

**Resulting Query** (SQL injection successful):

```sql
SELECT * FROM rules WHERE name = ''; DROP TABLE rules; --'
-- Result: rules table deleted
```

**Protected Code (Parameterized Query)**:

```go
// SAFE
db.Query("SELECT * FROM rules WHERE name = ?", userInput)
```

**Same Malicious Input**:

```
userInput = "'; DROP TABLE rules; --"
```

**Resulting Query** (attack prevented):

```sql
SELECT * FROM rules WHERE name = '''; DROP TABLE rules; --'
-- Result: Searches for literal string "'; DROP TABLE rules; --"
-- No SQL injection, just a failed search
```

### Code Review Checklist

- [ ] All database queries use database/sql with placeholders
- [ ] No fmt.Sprintf() or string concatenation used to build SQL queries
- [ ] No manual escaping or quote handling in code
- [ ] Static analysis tools could enforce this pattern (future consideration)

**Cross-References**:

- Database Backend: database/sql parameterized query specifications
- Database Validation: Database constraint specifications

## Command Injection Prevention (Control 4)

### Requirement

This project NEVER spawns sub-processes. Command injection is an explicit non-requirement.

### Validation Rules

1. **No Process Spawning**: NEVER use os/exec or equivalent
2. **No Shell Execution**: No shell invocation anywhere in codebase
3. **Architectural Constraint**: System design eliminates need for external process execution

### Rationale

Simplicity principle and security-by-design. Eliminating process spawning:

- Removes entire class of vulnerabilities (command injection, shell injection, path injection)
- Reduces attack surface significantly
- Simplifies security audits
- Improves operational predictability

### Layer Distribution

- **All Layers**: No process spawning anywhere in system; code review enforces architectural constraint
- **API Layer**: Not applicable (no process spawning)
- **Runtime Layer**: Not applicable (no process spawning)
- **UI Layer**: Not applicable (no process spawning)
- **Database Layer**: Not applicable (no process spawning)

### Implementation

**Architectural Constraint**: System design eliminates need for external process execution:

- No external command-line tools invoked
- No shell scripts executed
- No image processing pipelines
- No file format conversions requiring external tools
- All functionality implemented in Go natively

### Examples

**What NOT to Do (Forbidden Patterns)**:

```go
// FORBIDDEN: Never spawn processes
import "os/exec"

// FORBIDDEN
exec.Command("rm", "-rf", userInput).Run()

// FORBIDDEN
exec.Command("sh", "-c", userInput).Run()

// FORBIDDEN
exec.Command(userInput).Run()
```

**Correct Approach (Native Go Implementations)**:

```go
// Correct: Use Go standard library
import "os"
os.RemoveAll(path)  // Instead of rm -rf

// Correct: Use Go libraries for file operations
import "os"
file, err := os.Create(path)
if err != nil {
    return err
}
defer file.Close()
file.WriteString(content)  // Instead of echo > file

// Correct: Use Go libraries for data processing
import "encoding/json"
var data map[string]interface{}
json.Unmarshal([]byte(input), &data)  // Instead of jq
```

### Code Review Checklist

- [ ] No os/exec usage anywhere in codebase
- [ ] No shell invocation or command execution
- [ ] All functionality implemented natively in Go
- [ ] No external tool dependencies requiring process spawning

**Cross-References**:

- Architectural Principles: Simplicity principle and security-by-design

## Path Traversal Prevention (Control 5)

### Requirement

Canonicalize all file paths and validate they remain within data directory. Reject paths containing ".." or absolute paths from user input.

### Validation Rules

1. **Path Canonicalization**: Use filepath.Abs() and filepath.EvalSymlinks() to resolve symbolic links and normalize paths
2. **Directory Boundary Validation**: Verify canonicalized path starts with data directory prefix
3. **Reject Traversal Attempts**: Return 422 error if path contains ".." or resolves outside data directory
4. **No Absolute User Paths**: Reject absolute paths from user input (e.g., "/etc/passwd")

### Rationale

Path traversal attacks can:

- Access sensitive files outside intended directories (/etc/passwd, /etc/shadow)
- Overwrite critical system files
- Read configuration files containing secrets
- Bypass access controls

Canonicalization prevents:

- Symbolic link attacks (symlink to /etc/passwd)
- Relative path attacks (../../../etc/passwd)
- Double-encoded path attacks (%2e%2e%2f)

### Layer Distribution

- **API Layer (Primary)**: Validate all file paths before file operations; centralize path validation in tk-types library
- **Runtime Layer**: Re-validate paths at processing time (defense in depth) in both SDK execution and server processing
- **UI Layer**: Not applicable (file paths not accepted from web UI in MVP)
- **Database Layer**: Store relative paths only; canonicalization performed at access time

### Implementation

```go
import (
    "fmt"
    "path/filepath"
    "strings"
)

// ValidatePath validates file path remains within data directory
func ValidatePath(dataDir, userInput string) (string, error) {
    // Reject absolute paths from user input
    if filepath.IsAbs(userInput) {
        return "", &ValidationError{
            Type:    "PathTraversal",
            Path:    userInput,
            Message: "Absolute paths not allowed from user input",
        }
    }

    // Construct full path relative to data directory
    fullPath := filepath.Join(dataDir, userInput)

    // Canonicalize to resolve symbolic links and normalize
    canonical, err := filepath.EvalSymlinks(fullPath)
    if err != nil {
        return "", &ValidationError{
            Type:    "PathTraversal",
            Path:    userInput,
            Message: fmt.Sprintf("Path canonicalization failed: %v", err),
        }
    }
    canonical, _ = filepath.Abs(canonical)

    // Verify canonicalized path is within data directory
    if !strings.HasPrefix(canonical, dataDir) {
        return "", &ValidationError{
            Type:    "PathTraversal",
            Path:    userInput,
            Message: "Path resolves outside data directory",
        }
    }

    return canonical, nil
}
```

### Examples

**Configuration**:

```go
dataDir := "/var/lib/trapperkeeper/data"
// Assume dataDir is canonicalized at startup
```

**Valid Inputs**:

```go
// Relative path within data directory
ValidatePath(dataDir, "events/2025/11/event.jsonl")
// Result: /var/lib/trapperkeeper/data/events/2025/11/event.jsonl

// Subdirectory
ValidatePath(dataDir, "exports")
// Result: /var/lib/trapperkeeper/data/exports

// Filename only
ValidatePath(dataDir, "config.json")
// Result: /var/lib/trapperkeeper/data/config.json
```

**Invalid Inputs (Rejected with 422 Error)**:

```go
// Path traversal with ..
ValidatePath(dataDir, "../../etc/passwd")
// Error: Path resolves outside data directory

// Absolute path
ValidatePath(dataDir, "/etc/passwd")
// Error: Absolute paths not allowed from user input

// Hidden path traversal
ValidatePath(dataDir, "data/../../../etc/passwd")
// Error: Path resolves outside data directory

// Symbolic link to sensitive file
// Assume: /var/lib/trapperkeeper/data/evil_symlink -> /etc/passwd
ValidatePath(dataDir, "evil_symlink")
// Error: Path resolves outside data directory (after canonicalization)
```

### Attack Example (Why Canonicalization Matters)

**Scenario 1: Relative Path Traversal**:

```go
userInput := "../../etc/passwd"
fullPath := "/var/lib/trapperkeeper/data/../../etc/passwd"
canonical := "/etc/passwd"  // After canonicalization
// Rejected: Does not start with /var/lib/trapperkeeper/data
```

**Scenario 2: Symbolic Link Attack**:

```bash
# Attacker creates symbolic link inside data directory
cd /var/lib/trapperkeeper/data
ln -s /etc/passwd sensitive_file
```

```go
userInput := "sensitive_file"
fullPath := "/var/lib/trapperkeeper/data/sensitive_file"
canonical := "/etc/passwd"  // After canonicalization (follows symlink)
// Rejected: Does not start with /var/lib/trapperkeeper/data
```

### Error Response

API returns 422 Unprocessable Entity with path-specific error:

```json
{
  "error": "Validation failed",
  "code": "VALIDATION_ERROR",
  "request_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "details": [
    {
      "field": "file_path",
      "message": "Path resolves outside data directory",
      "code": "PATH_TRAVERSAL",
      "received": "../../etc/passwd"
    }
  ]
}
```

### Code Review Checklist

- [ ] All file paths from user input validated with canonicalization
- [ ] Absolute paths rejected at API boundary
- [ ] Canonicalized paths verified within data directory
- [ ] Symbolic links handled correctly (canonicalization follows them)
- [ ] No manual ".." or "/" parsing (use filepath.EvalSymlinks)

**Cross-References**:

- Configuration Management: data_dir configuration specifications
- API Validation: Structured error response format

## Validation Domain Architecture

TrapperKeeper distinguishes between two validation domains with different enforcement strategies.

### Domain 1: Internal System Schemas (STRICT)

**Scope**: tk-types, database schemas, rule definitions, API requests, configuration files

**Enforcement Strategy**: Strict validation at API layer with zero tolerance for malformed data.

**Validation Types**:

- Structural format validation (UUIDs, timestamps, field paths)
- Rule expression validation (operator/field_type compatibility, nested wildcards)
- Type validation (enums, boolean, integer, duration, port, path, URL, log level)
- Authentication validation (password hashing, API key format, HMAC signatures)
- Configuration validation (type validation, dependency validation, security validation)

**Error Handling**: Fail fast with 422 Unprocessable Entity for validation errors; reject invalid data immediately.

**Rationale**: Internal schemas are under system control. Strict validation prevents cascading failures, ensures data integrity, and provides clear error feedback during development and integration.

### Domain 2: External User Data (WEAK, BEST-EFFORT)

**Scope**: Incoming events, sensor data, client metadata, custom field values

**Enforcement Strategy**: Sanitization with graceful degradation for operational resilience.

**Validation Types**:

- Input sanitization (UTF-8 validation, control character filtering)
- Path traversal prevention (canonicalization, boundary validation)
- Runtime field resolution (missing field detection, type coercion)
- Type coercion (numeric/text/boolean with null-like value handling)

**Error Handling**: Apply on_missing_field policies (skip/fail/match); log validation failures; continue processing when safe.

**Rationale**: External data sources are heterogeneous and unpredictable. Strict validation would create operational brittleness (sensor failures cascade to system unavailability). Best-effort sanitization maintains security boundaries while allowing operational flexibility.

### Where OWASP Sanitization Applies

**Primary Application (Domain 2)**:

- UTF-8 validation with control character filtering applies to incoming events and sensor data at ingestion boundaries
- Path traversal prevention applies to file paths from external sources
- HTML escaping applies when rendering external data in web UI
- SQL injection prevention applies when storing external data (parameterized queries)

**Secondary Application (Domain 1)**:

- HTML escaping applies when rendering internal system data (rule names, usernames) in web UI
- SQL injection prevention applies to all database operations regardless of data source
- Configuration validation applies to all configuration sources (files, environment, CLI)

### Defense-in-Depth Strategy

**Layer 1 (API Boundary)**: Primary enforcement point for all sanitization controls

- UTF-8 validation with control character filtering
- Path canonicalization and traversal prevention
- Format validation (UUIDs, timestamps, field paths)
- Authentication validation (credentials, tokens, signatures)

**Layer 2 (Runtime Processing)**: Re-validation for defense-in-depth

- UTF-8 re-validation before processing
- Path re-validation before file operations
- Type coercion with null-like value handling
- Missing field detection with on_missing_field policies

**Layer 3 (Database Storage)**: Final enforcement with constraints

- Foreign key constraints for referential integrity
- Unique indexes for identifiers
- NOT NULL constraints where applicable
- Parameterized queries prevent injection

**Cross-References**:

- Responsibility Matrix: Layer enforcement assignments across all validation types
- Runtime Validation: Type coercion and missing field handling specifications
- Resilience Hub: Failure modes and degradation strategies

## Layer Responsibility Matrix

Complete matrix showing which layers enforce input sanitization controls.

| Security Control             | UI Layer                                                | API Layer                   | Runtime Layer                  | Database Layer                       |
| ---------------------------- | ------------------------------------------------------- | --------------------------- | ------------------------------ | ------------------------------------ |
| UTF-8 Validation             | N/A (server-side)                                       | ✓ Primary enforcement       | Re-validate (defense-in-depth) | TEXT column type                     |
| HTML Escaping                | ✓ html/template auto-escape                             | N/A (returns JSON)          | N/A                            | Store raw values                     |
| SQL Injection Prevention     | N/A                                                     | N/A                         | N/A                            | ✓ database/sql parameterized queries |
| Command Injection Prevention | Architectural constraint (no process spawning anywhere) |                             |                                |                                      |
| Path Traversal Prevention    | N/A (no file paths in MVP)                              | ✓ Canonicalize and validate | Re-validate (defense-in-depth) | Store relative paths                 |

**Legend**:

- ✓ = Primary enforcement point
- N/A = Not applicable to this layer
- Text = Supporting mechanism or constraint

**Cross-References**:

- Responsibility Matrix: Complete 12×4 matrix with all validation types
- API Validation: API layer enforcement patterns
- Runtime Validation: Defense-in-depth re-validation patterns

## OWASP Compliance Checklist

Security controls implemented from OWASP Input Validation Cheat Sheet.

### Implemented Controls

- ✓ UTF-8 validation with control character filtering
- ✓ HTML context-appropriate escaping (html/template)
- ✓ SQL parameterized queries (database/sql)
- ✓ Path canonicalization and traversal prevention
- ✓ Length limits on all user input fields
- ✓ Type validation (string, integer, boolean, UUID, timestamp)
- ✓ Allowlist validation for enums (field_type, operator, on_missing_field)
- ✓ Reject null bytes (0x00) in all string inputs

### Not Applicable

- Command injection prevention: System never spawns processes (architectural constraint)
- LDAP injection prevention: No LDAP integration
- XML injection prevention: No XML processing (JSON-only)

### Future Considerations

- Regular expression DoS (ReDoS) prevention: Audit regex patterns for catastrophic backtracking
- JSON schema validation: Formalize JSON validation using JSON Schema for API requests
- Content Security Policy (CSP): Add CSP headers to web UI responses
- Rate limiting: Add rate limiting to API endpoints to prevent abuse

**Reference**: OWASP Input Validation Cheat Sheet - https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html

**Cross-References**:

- Security Hub: Overall security architecture and threat model
- API Validation: Complete validation type specifications

## Edge Cases and Limitations

### Known Limitations

**UTF-8 Validation**:

- Limitation: Tab/LF/CR allowed in multi-line text may enable limited log injection if logs not properly escaped
- Rationale: Tab/LF/CR required for formatting in rule descriptions and comments
- Workaround: Structured logging with proper escaping of newlines in log aggregation systems

**Path Traversal Prevention**:

- Limitation: Canonicalization requires file/directory to exist; validation fails for non-existent paths
- Rationale: filepath.EvalSymlinks() requires filesystem access to resolve symbolic links
- Workaround: Create parent directories before validation, or validate parent directory exists

**HTML Escaping**:

- Limitation: html/template auto-escaping can be bypassed with template.HTML type
- Rationale: Some legitimate use cases require unescaped HTML (rich text editors)
- Workaround: Code review enforces no template.HTML usage; future: allowlist for specific templates

### Edge Cases

**UTF-8 Validation Edge Cases**:

- Unicode normalization not performed: "café" (NFC) and "café" (NFD) treated as different strings
- Zero-width characters allowed: Zero-width space (U+200B), zero-width joiner (U+200D) not filtered
- Right-to-left override characters allowed: May cause display issues in some contexts

**Path Traversal Edge Cases**:

- Case-insensitive filesystems: Validation may fail on macOS/Windows if dataDir has different case
- Network paths: UNC paths (\\server\share) and network mounts require special handling
- Permissions: Canonicalization may fail if process lacks read permissions on intermediate directories

**SQL Injection Edge Cases**:

- Table/column names from user input: Cannot be parameterized; require allowlist validation
- Dynamic ORDER BY clauses: Cannot use placeholders; require allowlist validation
- LIKE patterns: User-supplied % and \_ require escaping if used in LIKE clauses

**Cross-References**:

- Error Handling Strategy: Error handling patterns for edge cases
- Database Validation: Database constraint specifications and limitations

## Related Documents

**Dependencies** (read these first):

- Validation Hub: Strategic overview of validation architecture and layer distribution
- Responsibility Matrix: Layer enforcement assignments for all validation types

**Related Spokes** (siblings in this hub):

- API Validation: API layer enforcement patterns and structured error responses
- Runtime Validation: Defense-in-depth re-validation and type coercion patterns
- UI Validation: HTML5 form validation and html/template patterns
- Database Validation: Database constraints and parameterized query patterns

**Extended by** (documents building on this):

- Security Hub: Overall security architecture and threat model
- Testing Strategy: Fuzz testing requirements for input sanitization

**References**:

- Web Framework: html/template and CSRF protection
- Database Backend: database/sql parameterized query specifications
- Architectural Principles: Simplicity principle and security-by-design
- Configuration Management: dataDir configuration specifications

## Appendix A: Validation Error Codes

Complete list of input sanitization error codes returned by API layer.

| Code                    | HTTP Status | Description                                  | Example Field                            |
| ----------------------- | ----------- | -------------------------------------------- | ---------------------------------------- |
| `INVALID_UTF8`          | 422         | Invalid UTF-8 encoding or control characters | `name` (contains null byte)              |
| `PATH_TRAVERSAL`        | 422         | Path traversal attempt detected              | `filePath` contains ".."                 |
| `XSS_ATTEMPT`           | 422         | Potential XSS attack detected (future)       | `description` (manual HTML construction) |
| `SQL_INJECTION_ATTEMPT` | 422         | Potential SQL injection detected (future)    | `query` (manual SQL construction)        |

**Example Error Response**:

```json
{
  "error": "Validation failed",
  "code": "VALIDATION_ERROR",
  "request_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "details": [
    {
      "field": "rule_name",
      "message": "Control character 0x00 not allowed (except tab/LF/CR)",
      "code": "INVALID_UTF8",
      "position": 4
    }
  ]
}
```

**Cross-References**:

- API Validation: Complete error code reference across all validation types

## Appendix B: Implementation Checklist

Pre-implementation checklist for input sanitization controls.

### UTF-8 Validation

- [ ] Implement ValidateUTF8Input() in validation package
- [ ] Add control character filtering (reject 0x00-0x1F except tab/LF/CR)
- [ ] Add unit tests with valid/invalid UTF-8 sequences
- [ ] Add fuzz tests with random byte sequences
- [ ] Integrate with API layer validation pipeline
- [ ] Add structured error responses with character position
- [ ] Document in API documentation with examples

### HTML Escaping

- [ ] Configure html/template with auto-escaping enabled (default)
- [ ] Create HTML templates for all web UI pages
- [ ] Code review enforces no fmt.Sprintf() HTML construction
- [ ] Add integration tests with XSS payloads
- [ ] Document html/template usage patterns in developer guide
- [ ] Add linting rule to detect manual HTML construction (future)

### SQL Injection Prevention

- [ ] Use database/sql parameterized queries EXCLUSIVELY
- [ ] Code review enforces no SQL string concatenation
- [ ] Add integration tests with SQL injection payloads
- [ ] Document database/sql usage patterns in developer guide
- [ ] Add linting rule to detect SQL string concatenation (future)
- [ ] Add static analysis to enforce parameterized queries (future)

### Command Injection Prevention

- [ ] Verify no os/exec usage in codebase
- [ ] Code review enforces architectural constraint
- [ ] Add linting rule to detect process spawning (future)
- [ ] Document architectural constraint in developer guide

### Path Traversal Prevention

- [ ] Implement ValidatePath() in validation package
- [ ] Add path canonicalization with boundary validation
- [ ] Add unit tests with path traversal attempts
- [ ] Add integration tests with symbolic links
- [ ] Integrate with API layer validation pipeline
- [ ] Add structured error responses with path details
- [ ] Document in API documentation with examples

**Cross-References**:

- Testing Strategy: Complete testing requirements for validation
