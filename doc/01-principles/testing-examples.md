---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: testing
hub_document: doc/01-principles/README.md
tags:
  - examples
  - property-based
  - mocking
  - web-ui
---

# Testing Examples

## Context

This document provides comprehensive examples implementing TrapperKeeper's Integration-First Testing philosophy. Examples cover environment setup, integration tests, property-based tests, mocking patterns, and web UI testing.

All examples follow patterns defined in Testing Integration Patterns and principles established in Testing Philosophy.

**Hub Document**: This document is part of the Principles Hub. See [Principles Overview](README.md) and Testing Philosophy for strategic context.

## Environment Setup

Integration tests initialize Docker environment and establish database connections:

```go
// tests/integration/common_test.go
package integration

import (
    "context"
    "testing"

    "github.com/testcontainers/testcontainers-go"
    "github.com/testcontainers/testcontainers-go/modules/postgres"
)

type TestEnvironment struct {
    DB     *sql.DB
    APIURL string
    WebURL string
}

func setupTestEnvironment(t *testing.T) *TestEnvironment {
    ctx := context.Background()

    // Start PostgreSQL container
    pgContainer, err := postgres.Run(ctx, "postgres:16-alpine",
        postgres.WithDatabase("trapperkeeper_test"),
        postgres.WithUsername("test"),
        postgres.WithPassword("test"),
        postgres.BasicWaitStrategies(),
    )
    if err != nil {
        t.Fatalf("failed to start postgres container: %v", err)
    }

    // Register cleanup
    t.Cleanup(func() {
        if err := pgContainer.Terminate(ctx); err != nil {
            t.Errorf("failed to terminate postgres container: %v", err)
        }
    })

    // Connect to database
    connStr, err := pgContainer.ConnectionString(ctx)
    if err != nil {
        t.Fatalf("failed to get connection string: %v", err)
    }

    db, err := sql.Open("postgres", connStr)
    if err != nil {
        t.Fatalf("failed to connect to database: %v", err)
    }

    // Run migrations
    runTestMigrations(t, db)

    return &TestEnvironment{
        DB:     db,
        APIURL: "http://localhost:50051",
        WebURL: "http://localhost:8080",
    }
}
```

**Usage in Tests**:

```go
func TestRuleEvaluation(t *testing.T) {
    env := setupTestEnvironment(t)

    // Test logic here using env.DB, env.APIURL
    // ...

    // Cleanup happens automatically via t.Cleanup
}
```

**Cross-References**:

- Testing Integration Patterns Section 1: Docker fixture configuration
- Testing Integration Patterns Section 2: Database seeding patterns

## Integration Test Examples

### Full-Flow Event Submission

Complete test validating auth → rule sync → evaluation → storage → query:

```go
// tests/integration/sensor_api_test.go
package integration

import (
    "context"
    "encoding/json"
    "testing"

    "github.com/stretchr/testify/assert"
    "trapperkeeper/sdk"
)

func TestEventSubmissionFullFlow(t *testing.T) {
    env := setupTestEnvironment(t)

    // Create API key via Web UI
    apiKey := createAPIKeyViaWebUI(t, env, "test-sensor")

    // Initialize SDK
    sensor, err := sdk.NewSensor(env.APIURL, apiKey)
    assert.NoError(t, err)

    // Create rule via Web UI
    rule := Rule{
        Name:       "Detect high temperature",
        Expression: "$.temperature > 80",
        Severity:   "critical",
    }
    ruleID := createRuleViaWebUI(t, env, rule)

    // Sync rules to sensor
    err = sensor.SyncRules(context.Background())
    assert.NoError(t, err)

    // Submit event matching rule
    event := map[string]interface{}{
        "sensor_id":   "temp-sensor-01",
        "temperature": 95.5,
        "timestamp":   "2025-10-29T10:00:00Z",
    }
    eventID, err := sensor.SubmitEvent(context.Background(), event)
    assert.NoError(t, err)
    assert.NotEmpty(t, eventID)

    // Validate event in database
    var storedEvent StoredEvent
    err = env.DB.QueryRow(
        "SELECT id, severity, matched_rule_id FROM events WHERE id = $1",
        eventID,
    ).Scan(&storedEvent.ID, &storedEvent.Severity, &storedEvent.MatchedRuleID)
    assert.NoError(t, err)

    assert.Equal(t, eventID, storedEvent.ID)
    assert.Equal(t, "critical", storedEvent.Severity)
    assert.Equal(t, ruleID, storedEvent.MatchedRuleID)
}
```

### ETAG-Based Rule Synchronization

```go
func TestRuleSyncWithETagCaching(t *testing.T) {
    env := setupTestEnvironment(t)
    apiKey := createAPIKeyViaWebUI(t, env, "test-sensor")
    sensor, err := sdk.NewSensor(env.APIURL, apiKey)
    assert.NoError(t, err)

    ctx := context.Background()

    // First sync - fetches all rules
    rulesV1, err := sensor.SyncRules(ctx)
    assert.NoError(t, err)
    assert.NotEmpty(t, rulesV1)
    etagV1 := sensor.GetETag()

    // Second sync - should use ETAG (no rules returned)
    rulesV2, err := sensor.SyncRules(ctx)
    assert.NoError(t, err)
    assert.Empty(t, rulesV2) // Empty means "not modified"
    etagV2 := sensor.GetETag()
    assert.Equal(t, etagV1, etagV2)

    // Create new rule
    createRuleViaWebUI(t, env, Rule{
        Name:       "New Rule",
        Expression: "$.value > 100",
        Severity:   "warning",
    })

    // Third sync - ETAG changed, fetches updated rules
    rulesV3, err := sensor.SyncRules(ctx)
    assert.NoError(t, err)
    assert.NotEmpty(t, rulesV3)
    etagV3 := sensor.GetETag()
    assert.NotEqual(t, etagV2, etagV3)
}
```

**Cross-References**:

- API Service Architecture: ETAG-based sync protocol
- Testing Integration Patterns Section 3: API testing strategies

## Property-Based Test Examples

### Arbitrary JSON Evaluation

Python hypothesis example for schema-agnostic testing:

```python
# tests/property/test_rule_evaluation.py
from hypothesis import given
import hypothesis.strategies as st

@given(st.recursive(
    st.none() | st.booleans() | st.floats() | st.text(),
    lambda children: st.lists(children) | st.dictionaries(st.text(), children)
))
def test_rule_evaluation_handles_arbitrary_json(data):
    """
    Property: Rule evaluation never crashes, regardless of data shape.

    This single test validates millions of data variations:
    - Nested objects of arbitrary depth
    - Mixed types in arrays
    - Unicode strings with emoji/control chars
    - Null values at any level
    - Empty objects and arrays
    """
    sensor = Sensor(api_key="test-key")

    # Rule: $.users[*].age > 18
    # Expected: Evaluates without crashing, handles missing fields gracefully
    result = sensor.evaluate_record(data)

    # Postcondition: Either matched, skipped, or failed gracefully
    assert result in ['matched', 'skipped', 'type_error']
    assert sensor.is_alive()  # Did not crash
```

### Wildcard Field Path Resolution

```python
# tests/property/test_field_paths.py
from hypothesis import given, strategies as st
import trapperkeeper.sdk as tk

@given(st.recursive(
    st.none() | st.booleans() | st.integers() | st.floats() | st.text(),
    lambda children: st.lists(children) | st.dictionaries(st.text(), children),
    max_leaves=100,
))
def test_wildcard_resolution_never_crashes(data):
    """
    Property: Wildcard field path resolution handles arbitrary data shapes.

    Tests millions of variations:
    - Nested objects of arbitrary depth
    - Arrays of mixed types
    - Missing keys at any level
    - Null values
    """
    sensor = tk.Sensor(api_key="test-key", mode="dry-run")

    # Rule with wildcard: $.users[*].profile.age
    sensor.add_rule(rule_id="test-rule", expression="$.users[*].profile.age > 18")

    # Evaluate arbitrary data
    result = sensor.evaluate(data)

    # Postcondition: Evaluation completes without exception
    assert result.status in ['matched', 'not_matched', 'skipped']
    assert sensor.is_healthy()
```

### Go rapid Property Testing Example

```go
// tests/property/field_path_test.go
package property

import (
    "testing"

    "pgregory.net/rapid"
    "trapperkeeper/sdk"
)

func TestFieldPathResolutionNeverPanics(t *testing.T) {
    rapid.Check(t, func(t *rapid.T) {
        // Generate arbitrary JSON-like data
        data := rapid.Custom(func(t *rapid.T) interface{} {
            return generateArbitraryJSON(t, 5)
        }).Draw(t, "data")

        sensor := sdk.NewDryRunSensor()
        err := sensor.AddRule(Rule{
            Expression: "$.data[*].value > 10",
            Action:     ActionObserve,
        })
        if err != nil {
            return // Invalid rule is acceptable
        }

        // Should never panic regardless of input
        result := sensor.Evaluate(data)

        // Either succeeds or fails gracefully
        _ = result // Result can be success or error
    })
}

func generateArbitraryJSON(t *rapid.T, maxDepth int) interface{} {
    if maxDepth == 0 {
        return rapid.OneOf(
            rapid.Just(nil),
            rapid.Bool().AsAny(),
            rapid.Float64().AsAny(),
            rapid.String().AsAny(),
        ).Draw(t, "leaf")
    }

    return rapid.OneOf(
        rapid.Just(nil),
        rapid.Bool().AsAny(),
        rapid.Float64().AsAny(),
        rapid.String().AsAny(),
        rapid.SliceOf(rapid.Custom(func(t *rapid.T) interface{} {
            return generateArbitraryJSON(t, maxDepth-1)
        })).AsAny(),
        rapid.MapOf(rapid.String(), rapid.Custom(func(t *rapid.T) interface{} {
            return generateArbitraryJSON(t, maxDepth-1)
        })).AsAny(),
    ).Draw(t, "node")
}
```

**Seed-Based Reproduction**:

```go
func TestSpecificFailureCase(t *testing.T) {
    // Reproduce failure from CI with specific seed
    // Seed value appears in test output when failure occurs
    rapid.Check(t, func(t *rapid.T) {
        // Test logic here
    }, rapid.Seed(0x1234567890abcdef)) // Seed from CI failure log
}
```

**Cross-References**:

- Testing Philosophy Section 5: Property-based testing strategy
- Field Path Resolution: Wildcard resolution implementation

## Mocking Examples

### Clock Drift Scenario

Legitimate mock for UUIDv7 generation when client clock is ahead:

```go
// Testing UUIDv7 generation when client clock is ahead of server
package sdk

import (
    "testing"
    "time"

    "github.com/stretchr/testify/assert"
)

// TimeProvider is the interface for clock operations
type TimeProvider interface {
    Now() time.Time
}

// MockTimeProvider implements TimeProvider for testing
type MockTimeProvider struct {
    currentTime time.Time
}

func (m *MockTimeProvider) Now() time.Time {
    return m.currentTime
}

func TestUUIDv7WithClockDrift(t *testing.T) {
    // Mock clock set 2 hours ahead
    mockClock := &MockTimeProvider{
        currentTime: time.Now().Add(2 * time.Hour),
    }

    sensor := NewSensorWithClock(apiKey, mockClock)
    eventID := sensor.GenerateEventID()

    // Verify: System warns but does not reject
    assert.Contains(t, logs, "clock drift detected")
    assert.NotEmpty(t, eventID)
}
```

### gRPC Error Code Injection

```go
func TestRateLimitingBackoff(t *testing.T) {
    // Start mock gRPC server
    lis := bufconn.Listen(1024 * 1024)
    s := grpc.NewServer()

    // Register mock service that returns RESOURCE_EXHAUSTED
    mockServer := &MockSensorAPIServer{
        syncRulesErr: status.Error(codes.ResourceExhausted, "Rate limit exceeded"),
    }
    pb.RegisterSensorAPIServer(s, mockServer)

    go s.Serve(lis)
    t.Cleanup(s.Stop)

    // Create client connected to mock server
    conn, err := grpc.Dial("bufnet",
        grpc.WithContextDialer(func(context.Context, string) (net.Conn, error) {
            return lis.Dial()
        }),
        grpc.WithTransportCredentials(insecure.NewCredentials()),
    )
    assert.NoError(t, err)
    t.Cleanup(func() { conn.Close() })

    sensor := NewSensorWithConn(conn, "test-key")

    // Should trigger exponential backoff
    err = sensor.SyncRules(context.Background())
    assert.Error(t, err)
    assert.Contains(t, logs, "rate limit exceeded, backing off")
}
```

**Rationale**: Real rate limiting is difficult to trigger reliably in tests. Mock enables deterministic testing of backoff logic.

**Cross-References**:

- Testing Philosophy Section 7: Mocking guidelines
- Failure Modes and Degradation: Backoff strategy

## Web UI Test Examples

### Complete Login Flow

```go
// tests/integration/web_ui_test.go
package integration

import (
    "net/http"
    "net/http/httptest"
    "net/url"
    "strings"
    "testing"

    "github.com/PuerkitoBio/goquery"
    "github.com/stretchr/testify/assert"
)

func TestLoginRedirectsOnForcePasswordChange(t *testing.T) {
    // Setup: Create database and test user
    db := createTestDB(t)
    user := createTestUser(t, db, "testuser", true) // force_password_change = true

    app := createTestApp(t, db)
    server := httptest.NewServer(app)
    t.Cleanup(server.Close)

    // Action: Submit login form
    form := url.Values{}
    form.Set("username", "testuser")
    form.Set("password", "test-password")

    req, _ := http.NewRequest("POST", server.URL+"/login",
        strings.NewReader(form.Encode()))
    req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

    resp, err := http.DefaultClient.Do(req)
    assert.NoError(t, err)
    defer resp.Body.Close()

    // Assert: Redirects to change password page
    assertRedirectsTo(t, resp, "/change-password")

    // Extract session cookie
    cookies := resp.Cookies()
    var sessionCookie *http.Cookie
    for _, c := range cookies {
        if c.Name == "session" {
            sessionCookie = c
            break
        }
    }
    assert.NotNil(t, sessionCookie)

    // Verify: Accessing protected route redirects to change password
    req2, _ := http.NewRequest("GET", server.URL+"/dashboard", nil)
    req2.AddCookie(sessionCookie)

    resp2, err := http.DefaultClient.Do(req2)
    assert.NoError(t, err)
    defer resp2.Body.Close()

    assertRedirectsTo(t, resp2, "/change-password")
}
```

### Password Change Flow

```go
func TestChangePasswordClearsForceFlag(t *testing.T) {
    db := createTestDB(t)
    user := createTestUser(t, db, "testuser", true)

    app := createTestApp(t, db)
    server := httptest.NewServer(app)
    t.Cleanup(server.Close)

    // Login and get session cookie
    loginForm := url.Values{}
    loginForm.Set("username", "testuser")
    loginForm.Set("password", "test-password")

    req, _ := http.NewRequest("POST", server.URL+"/login",
        strings.NewReader(loginForm.Encode()))
    req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

    loginResp, err := http.DefaultClient.Do(req)
    assert.NoError(t, err)
    defer loginResp.Body.Close()

    var sessionCookie *http.Cookie
    for _, c := range loginResp.Cookies() {
        if c.Name == "session" {
            sessionCookie = c
            break
        }
    }
    assert.NotNil(t, sessionCookie)

    // Change password
    passwordForm := url.Values{}
    passwordForm.Set("new_password", "new-secure-password")
    passwordForm.Set("confirm_password", "new-secure-password")

    req2, _ := http.NewRequest("POST", server.URL+"/change-password",
        strings.NewReader(passwordForm.Encode()))
    req2.Header.Set("Content-Type", "application/x-www-form-urlencoded")
    req2.AddCookie(sessionCookie)

    changeResp, err := http.DefaultClient.Do(req2)
    assert.NoError(t, err)
    defer changeResp.Body.Close()

    // Assert: Redirects to dashboard after password change
    assertRedirectsTo(t, changeResp, "/dashboard")

    // Verify: Database updated
    var userUpdated User
    err = db.QueryRow(
        "SELECT * FROM users WHERE username = $1",
        "testuser",
    ).Scan(&userUpdated.ID, &userUpdated.Username, &userUpdated.PasswordHash,
        &userUpdated.ForcePasswordChange)
    assert.NoError(t, err)
    assert.False(t, userUpdated.ForcePasswordChange)
}
```

### Rule CRUD Operations

```go
func TestRuleCRUDOperations(t *testing.T) {
    db := createTestDB(t)
    user := createTestUser(t, db, "testuser", false)

    app := createTestApp(t, db)
    server := httptest.NewServer(app)
    t.Cleanup(server.Close)

    // Login
    loginForm := url.Values{}
    loginForm.Set("username", "testuser")
    loginForm.Set("password", "test-password")

    req, _ := http.NewRequest("POST", server.URL+"/login",
        strings.NewReader(loginForm.Encode()))
    req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

    loginResp, err := http.DefaultClient.Do(req)
    assert.NoError(t, err)
    defer loginResp.Body.Close()

    var sessionCookie *http.Cookie
    for _, c := range loginResp.Cookies() {
        if c.Name == "session" {
            sessionCookie = c
            break
        }
    }
    assert.NotNil(t, sessionCookie)

    // Create rule via Web UI
    ruleForm := url.Values{}
    ruleForm.Set("name", "High Temperature Alert")
    ruleForm.Set("expression", "$.temperature > 80")
    ruleForm.Set("severity", "critical")

    req2, _ := http.NewRequest("POST", server.URL+"/rules",
        strings.NewReader(ruleForm.Encode()))
    req2.Header.Set("Content-Type", "application/x-www-form-urlencoded")
    req2.AddCookie(sessionCookie)

    createResp, err := http.DefaultClient.Do(req2)
    assert.NoError(t, err)
    defer createResp.Body.Close()

    // Assert: Redirects to rules list
    assertRedirectsTo(t, createResp, "/rules")

    // Verify: Rule stored in database
    var rule Rule
    err = db.QueryRow(
        "SELECT * FROM rules WHERE name = $1",
        "High Temperature Alert",
    ).Scan(&rule.ID, &rule.Name, &rule.Expression, &rule.Severity, &rule.State)
    assert.NoError(t, err)

    assert.Equal(t, "$.temperature > 80", rule.Expression)
    assert.Equal(t, "critical", rule.Severity)
    assert.Equal(t, RuleStateDraft, rule.State) // New rules start in draft
}
```

**Cross-References**:

- Testing Integration Patterns Section 4: Web UI testing framework
- Web Framework Selection: net/http httptest utilities
- Authentication and User Management: Cookie-based authentication

## Common Scenarios and Anti-Patterns

### Anti-Pattern: Testing Implementation Details

**Bad**:

```go
func TestInternalParserState(t *testing.T) {
    parser := NewRuleParser()
    parser.Parse("$.value > 10")

    // Testing internal state
    assert.Equal(t, 5, len(parser.tokens))
    assert.Equal(t, 2, parser.astDepth)
}
```

**Good**:

```go
func TestRuleParsesCorrectly(t *testing.T) {
    rule, err := ParseRule("$.value > 10")
    assert.NoError(t, err)

    // Testing observable behavior
    event := map[string]interface{}{"value": 15}
    assert.True(t, rule.Matches(event))
}
```

### Anti-Pattern: Brittle Mocks

**Bad**:

```go
func TestWithMockDatabase(t *testing.T) {
    mockDB := NewMockDatabase()
    mockDB.On("Query", "SELECT * FROM users WHERE id = ?", 1).
        Return([]User{{ID: 1, Name: "test"}}, nil).
        Once()

    // Breaks if query changes even slightly
}
```

**Good**:

```go
func TestWithRealDatabase(t *testing.T) {
    db := createTestDB(t)
    user := createTestUser(t, db, "test")

    // Uses real database, more resilient to refactoring
    foundUser, err := FindUserByID(db, user.ID)
    assert.NoError(t, err)
    assert.Equal(t, "test", foundUser.Name)
}
```

### Anti-Pattern: Testing Framework Internals

**Bad**:

```go
func TestHTTPRouting(t *testing.T) {
    // Testing standard library's routing logic
    mux := http.NewServeMux()
    mux.HandleFunc("/test", handler)
    // ...
}
```

**Good**:

```go
func TestEndpointReturnsCorrectResponse(t *testing.T) {
    app := createApp()
    server := httptest.NewServer(app)
    t.Cleanup(server.Close)

    resp, err := http.Get(server.URL + "/test")
    assert.NoError(t, err)
    defer resp.Body.Close()

    // Testing our application logic, not framework
    assert.Equal(t, http.StatusOK, resp.StatusCode)
}
```

**Cross-References**:

- Testing Philosophy Section 6: What NOT to test
- Testing Integration Patterns: Recommended patterns

## Related Documents

**Dependencies** (read these first):

- Testing Philosophy: Strategic overview and testing trophy model
- Testing Integration Patterns: Implementation patterns used in examples

**Related Spokes** (siblings in this hub):

- Testing Philosophy: Explains why these patterns are chosen
- Testing Integration Patterns: Provides pattern definitions these examples implement

**Extended by**:

- Web Framework Selection: Additional net/http testing guidance
- SDK Model: SDK-specific testing examples
