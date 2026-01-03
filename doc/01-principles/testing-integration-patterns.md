---
doc_type: spoke
status: active
primary_category: testing
hub_document: doc/01-principles/README.md
tags:
  - docker
  - database-fixtures
  - api-testing
  - grpc
---

# Testing Integration Patterns

## Context

Integration tests require consistent, repeatable patterns for Docker fixtures, database seeding, API testing, and gRPC communication. These patterns enable high-confidence testing while minimizing maintenance burden for a five-engineer team.

This document provides concrete implementation patterns for integration testing infrastructure supporting TrapperKeeper's Integration-First Testing philosophy.

**Hub Document**: This document is part of the Principles Hub. See [Principles Overview](README.md) and Testing Philosophy for strategic overview of integration-first testing approach.

## Docker Fixture Configuration

All integration tests run against containerized services using Docker Compose.

### Test Environment Structure

```yaml
# docker-compose.test.yml
version: "3.8"
services:
  sensor-api:
    image: trapperkeeper/sensor-api:test
    environment:
      - DATABASE_URL=postgres://test@db:5432/trapperkeeper_test
      - AUTH_SECRET=test-secret-key
    depends_on:
      - db
    ports:
      - "50051:50051" # gRPC

  db:
    image: postgres:16-alpine
    environment:
      - POSTGRES_USER=test
      - POSTGRES_PASSWORD=test
      - POSTGRES_DB=trapperkeeper_test
    tmpfs:
      - /var/lib/postgresql/data # In-memory for speed

  web-ui:
    image: trapperkeeper/web-ui:test
    environment:
      - DATABASE_URL=postgres://test@db:5432/trapperkeeper_test
      - SESSION_SECRET=test-session-key
    depends_on:
      - db
    ports:
      - "8080:8080" # HTTP
```

**Key Features**:

- Isolated fresh state per test suite
- Database runs in-memory tmpfs for maximum speed
- Services exposed on standard ports
- Environment variables for test configuration
- Automatic cleanup after execution

### Lifecycle Management

```go
// tests/helpers/docker.go
package helpers

import (
    "context"
    "os/exec"
    "testing"
    "time"
)

func startTestEnvironment(t *testing.T) {
    ctx := context.Background()

    // Start containers
    cmd := exec.CommandContext(ctx, "docker-compose",
        "-f", "docker-compose.test.yml", "up", "-d")
    if err := cmd.Run(); err != nil {
        t.Fatalf("failed to start test environment: %v", err)
    }

    // Wait for services to be healthy
    time.Sleep(5 * time.Second)

    // Register cleanup
    t.Cleanup(func() {
        stopTestEnvironment()
    })
}

func stopTestEnvironment() {
    cmd := exec.Command("docker-compose",
        "-f", "docker-compose.test.yml", "down", "-v")
    _ = cmd.Run() // Best effort cleanup
}
```

**Cross-References**:

- Testing Examples Section 1: Complete environment setup code
- Testing Philosophy: Ephemeral test environment principles

## Database Seeding Patterns

Test database initialization follows consistent factory pattern.

### Test Database Creation

```go
// tests/helpers/db.go
package helpers

import (
    "context"
    "database/sql"
    "os"
    "testing"

    _ "github.com/lib/pq"
    "github.com/pressly/goose/v3"
)

func createTestDB(t *testing.T) *sql.DB {
    databaseURL := os.Getenv("TEST_DATABASE_URL")
    if databaseURL == "" {
        databaseURL = "postgres://test:test@localhost/trapperkeeper_test?sslmode=disable"
    }

    db, err := sql.Open("postgres", databaseURL)
    if err != nil {
        t.Fatalf("failed to connect to test database: %v", err)
    }

    // Run migrations
    if err := goose.Up(db, "./migrations"); err != nil {
        t.Fatalf("failed to run migrations: %v", err)
    }

    // Clean test data
    _, err = db.Exec("TRUNCATE users, rules, events CASCADE")
    if err != nil {
        t.Fatalf("failed to clean test data: %v", err)
    }

    t.Cleanup(func() {
        db.Close()
    })

    return db
}
```

### Factory Functions

```go
// tests/helpers/factories.go
package helpers

import (
    "database/sql"
    "testing"

    "github.com/google/uuid"
    "trapperkeeper/models"
)

// CreateTestUser creates a test user in the database
func createTestUser(
    t *testing.T,
    db *sql.DB,
    username string,
    forcePasswordChange bool,
) models.User {
    var user models.User

    err := db.QueryRow(`
        INSERT INTO users (id, username, password_hash, force_password_change)
        VALUES ($1, $2, $3, $4)
        RETURNING id, username, password_hash, force_password_change
    `,
        uuid.Must(uuid.NewV7()),
        username,
        hashPassword("test-password"),
        forcePasswordChange,
    ).Scan(&user.ID, &user.Username, &user.PasswordHash, &user.ForcePasswordChange)

    if err != nil {
        t.Fatalf("failed to create test user: %v", err)
    }

    return user
}

// CreateTestRule creates a test rule in the database
func createTestRule(
    t *testing.T,
    db *sql.DB,
    name string,
    expression string,
    state models.RuleState,
) models.Rule {
    var rule models.Rule

    err := db.QueryRow(`
        INSERT INTO rules (id, name, expression, state)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, expression, state
    `,
        uuid.Must(uuid.NewV7()),
        name,
        expression,
        state,
    ).Scan(&rule.ID, &rule.Name, &rule.Expression, &rule.State)

    if err != nil {
        t.Fatalf("failed to create test rule: %v", err)
    }

    return rule
}
```

**Usage Pattern**:

```go
func TestRuleEvaluation(t *testing.T) {
    db := createTestDB(t)
    rule := createTestRule(t, db, "High Temperature", "$.temperature > 80", models.RuleStateActive)

    // Test logic using seeded data
    // ...
}
```

**Cross-References**:

- Database Backend: SQLite/PostgreSQL configuration
- Database Migrations: sqlx migration strategy

## API Testing Strategies

### gRPC Client Testing

```go
// tests/integration/sensor_api_test.go
package integration

import (
    "context"
    "testing"

    "github.com/stretchr/testify/assert"
    "google.golang.org/grpc"
    "google.golang.org/grpc/credentials/insecure"
    "google.golang.org/grpc/metadata"

    pb "trapperkeeper/proto"
)

func TestSyncRulesWithETag(t *testing.T) {
    startTestEnvironment(t)

    conn, err := grpc.NewClient("localhost:50051",
        grpc.WithTransportCredentials(insecure.NewCredentials()))
    assert.NoError(t, err)
    t.Cleanup(func() { conn.Close() })

    client := pb.NewSensorAPIClient(conn)
    ctx := context.Background()

    // First sync - no ETAG
    req := &pb.SyncRulesRequest{
        Tags: []string{"production"},
    }

    var header metadata.MD
    resp, err := client.SyncRules(ctx, req, grpc.Header(&header))
    assert.NoError(t, err)

    etag := header.Get("etag")
    assert.NotEmpty(t, etag)

    // Second sync - with ETAG (should return empty)
    ctx2 := metadata.AppendToOutgoingContext(ctx, "if-none-match", etag[0])
    resp2, err := client.SyncRules(ctx2, req)
    assert.NoError(t, err)
    assert.Empty(t, resp2.Rules)
}
```

### Full-Flow Event Submission Test

```go
func TestEventSubmissionFullFlow(t *testing.T) {
    // Setup: Start docker-compose environment
    apiURL := "http://localhost:50051"
    db := connectToTestDB(t)

    // Create API key via Web UI
    apiKey := createAPIKeyViaWebUI(t, "test-sensor")

    // Initialize SDK
    sensor, err := sdk.NewSensor(apiURL, apiKey)
    assert.NoError(t, err)

    // Create rule via Web UI
    rule := Rule{
        Name:       "Detect high temperature",
        Expression: "$.temperature > 80",
        Severity:   "critical",
    }
    ruleID := createRuleViaWebUI(t, rule)

    // Sync rules to sensor
    ctx := context.Background()
    err = sensor.SyncRules(ctx)
    assert.NoError(t, err)

    // Submit event matching rule
    event := map[string]interface{}{
        "sensor_id":   "temp-sensor-01",
        "temperature": 95.5,
        "timestamp":   "2025-10-29T10:00:00Z",
    }
    eventID, err := sensor.SubmitEvent(ctx, event)
    assert.NoError(t, err)
    assert.NotEmpty(t, eventID)

    // Validate event in database
    var storedEvent StoredEvent
    err = db.QueryRow(
        "SELECT id, severity, matched_rule_id FROM events WHERE id = $1",
        eventID,
    ).Scan(&storedEvent.ID, &storedEvent.Severity, &storedEvent.MatchedRuleID)
    assert.NoError(t, err)

    assert.Equal(t, eventID, storedEvent.ID)
    assert.Equal(t, "critical", storedEvent.Severity)
    assert.Equal(t, ruleID, storedEvent.MatchedRuleID)
}
```

**Cross-References**:

- API Service Architecture: gRPC protocol specification
- SDK Model: SDK initialization and usage patterns
- Testing Examples Section 2: Additional API test examples

## Web UI Testing Framework

Server-side HTML validation using httptest utilities without JavaScript browser testing.

### HTTP Testing Library Setup

```go
// tests/helpers/web_ui.go
package helpers

import (
    "io"
    "net/http"
    "net/http/httptest"
    "net/url"
    "strings"
    "testing"

    "github.com/PuerkitoBio/goquery"
    "github.com/stretchr/testify/assert"
)

// PostForm submits form data to a path and returns response
func postForm(
    t *testing.T,
    handler http.Handler,
    path string,
    form url.Values,
) *http.Response {
    req := httptest.NewRequest("POST", path, strings.NewReader(form.Encode()))
    req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

    rr := httptest.NewRecorder()
    handler.ServeHTTP(rr, req)

    return rr.Result()
}

// AssertRedirectsTo verifies response redirects to expected path
func assertRedirectsTo(t *testing.T, resp *http.Response, expectedPath string) {
    assert.Equal(t, http.StatusSeeOther, resp.StatusCode)
    location := resp.Header.Get("Location")
    assert.Equal(t, expectedPath, location)
}

// AssertContainsTestID verifies HTML contains element with data-testid
func assertContainsTestID(t *testing.T, html string, testID string) {
    doc, err := goquery.NewDocumentFromReader(strings.NewReader(html))
    assert.NoError(t, err)

    selector := "[data-testid='" + testID + "']"
    assert.NotZero(t, doc.Find(selector).Length(),
        "Expected to find element with data-testid='%s'", testID)
}

// GetTestIDText extracts text content from element with data-testid
func getTestIDText(t *testing.T, html string, testID string) string {
    doc, err := goquery.NewDocumentFromReader(strings.NewReader(html))
    assert.NoError(t, err)

    selector := "[data-testid='" + testID + "']"
    return strings.TrimSpace(doc.Find(selector).First().Text())
}
```

### Session and Cookie Handling

```go
import (
    "net/http"
    "net/http/httptest"
    "testing"

    "github.com/gorilla/sessions"
    "github.com/stretchr/testify/assert"
)

func TestAuthenticatedSession(t *testing.T) {
    db := createTestDB(t)
    store := sessions.NewCookieStore([]byte("test-secret-key"))

    app := createApp(db, store)
    server := httptest.NewServer(app)
    t.Cleanup(server.Close)

    // Create authenticated session
    req := httptest.NewRequest("GET", "/dashboard", nil)
    rr := httptest.NewRecorder()

    // Manually set session cookie
    session, _ := store.Get(req, "session")
    session.Values["user_id"] = "test-user-123"
    session.Save(req, rr)

    // Extract cookie from recorder
    cookies := rr.Result().Cookies()
    assert.NotEmpty(t, cookies)

    // Make request with session cookie
    req2 := httptest.NewRequest("GET", "/dashboard", nil)
    for _, cookie := range cookies {
        req2.AddCookie(cookie)
    }

    rr2 := httptest.NewRecorder()
    app.ServeHTTP(rr2, req2)

    assert.Equal(t, http.StatusOK, rr2.Code)
}
```

### HTML Validation with data-testid

```go
func TestLoginFailureShowsError(t *testing.T) {
    db := createTestDB(t)
    user := createTestUser(t, db, "testuser", false)

    app := createTestApp(t, db)

    // Submit login with wrong password
    form := url.Values{}
    form.Set("username", "testuser")
    form.Set("password", "wrong-password")

    resp := postForm(t, app, "/login", form)

    // Assert: Returns to login page with error
    assert.Equal(t, http.StatusOK, resp.StatusCode)

    body, err := io.ReadAll(resp.Body)
    assert.NoError(t, err)
    html := string(body)

    // Verify: Error message displayed with data-testid
    assertContainsTestID(t, html, "error-message")
    errorText := getTestIDText(t, html, "error-message")
    assert.Contains(t, errorText, "Invalid username or password")
}
```

**Key Features**:

- Standard library httptest package (no external HTTP client)
- goquery library for HTML parsing with CSS selectors
- data-testid attributes for stable element selection
- No brittle XPath or regex-based HTML parsing
- Server-side rendering validation without JavaScript

**Cross-References**:

- Web Framework Selection: net/http handlers and html/template
- Testing Examples Section 3: Complete Web UI test implementations

## CI/CD Integration

GitHub Actions workflow for parallel test execution:

```yaml
# .github/workflows/test.yml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        component: [sensor-api, web-ui, go-sdk, python-sdk]

    steps:
      - uses: actions/checkout@v3

      - name: Cache Docker layers
        uses: actions/cache@v3
        with:
          path: /tmp/.buildx-cache
          key: ${{ runner.os }}-docker-${{ hashFiles('**/Dockerfile') }}

      - name: Start test environment
        run: docker-compose -f docker-compose.test.yml up -d

      - name: Run integration tests
        run: |
          docker-compose -f docker-compose.test.yml \
            run --rm ${{ matrix.component }}-tests

      - name: Collect logs on failure
        if: failure()
        run: docker-compose -f docker-compose.test.yml logs

      - name: Cleanup
        if: always()
        run: docker-compose -f docker-compose.test.yml down -v
```

**Parallel Execution Benefits**:

- Matrix strategy runs component tests in parallel
- Each matrix job gets isolated Docker environment
- Total test time: ~5-10 minutes (not 30+ minutes for sequential)
- Docker layer caching speeds up builds

**Failure Debugging**:

- All tests include `trace_id` in logs for request tracing
- Docker logs collected on test failure
- Failed property tests log seed value for reproduction

**Cross-References**:

- Testing Examples: Concrete test implementations
- Testing Philosophy: High-value test definition

## Test Data Management

### Property-Based Generation (Default)

Generate arbitrary JSON-serializable data using `rapid`:

```go
import (
    "testing"

    "pgregory.net/rapid"
    "trapperkeeper/sdk"
)

func TestFieldPathResolutionNeverCrashes(t *testing.T) {
    rapid.Check(t, func(t *rapid.T) {
        // Generate arbitrary JSON value
        data := generateArbitraryJSON(t, 5)

        sensor := sdk.NewDryRunSensor()
        err := sensor.AddRule(Rule{
            Expression: "$.users[*].profile.age > 18",
            // ...
        })
        if err != nil {
            return // Invalid rule is acceptable in this test
        }

        // Should not panic regardless of data shape
        result := sensor.Evaluate(data)
        _ = result // Result can be success or error, both acceptable
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

**Benefits**:

- Covers massive range of data shapes with minimal code
- Uses fixed seeds for reproducible test runs
- Automatic shrinking finds minimal failing case
- Perfect for schema-agnostic testing

### Static Test Data (When Required)

```
tests/
  fixtures/
    events/
      iot_waveform.parquet       # 500K-point sample from real customer
      compressed_json.json.gz    # Vendor-specific compressed format
      mqtt_csv_batch.csv         # Time-series batch from MQTT stream
    rules/
      complex_dnf_example.json   # Rule with 10+ clauses for perf testing
```

**Use Cases**:

- Authentication: Specific usernames, API keys, HMAC signatures
- Rule examples: Hand-crafted datasets demonstrating specific conditions
- Regression tests: Exact data that triggered past bugs

**Cross-References**:

- Testing Philosophy: Property-based testing strategy
- Testing Examples Section 3: Property-based test implementations

## Related Documents

**Dependencies** (read these first):

- Testing Philosophy: Integration-first approach and testing trophy model
- Principles Architecture: Ephemeral sensors principle informing test design

**Related Spokes** (siblings in this hub):

- Testing Examples: Concrete implementations using these patterns
- Testing Philosophy: Strategic overview of testing approach

**Extended by**:

- Web Framework Selection: net/http httptest utilities and HTML validation
- Database Migrations: sqlx migration testing strategy
