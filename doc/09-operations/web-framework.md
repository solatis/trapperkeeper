---
doc_type: spoke
status: active
primary_category: deployment
hub_document: doc/09-operations/README.md
tags:
  - web-framework
  - net/http
  - http
  - server-side-rendering
  - csrf
---

# Web Framework

## Context

TrapperKeeper's web UI service requires HTTP routing for server-side rendered HTML with CSRF protection, form validation, and static asset serving. This document specifies the Go stdlib net/http framework selection, middleware chain pattern, html/template integration, and session management.

**Hub Document**: This spoke is part of [Operations Overview](README.md). See the hub's Web Framework section for strategic context.

## Framework Selection

### Framework Rationale

**Selected**: Go stdlib net/http with Go 1.22+ path parameters

**Benefits**:

- Zero external dependencies for core routing (stdlib-only)
- Composable middleware using standard http.Handler interface
- Strong type safety prevents common web vulnerabilities
- Excellent SSR support with html/template (no JavaScript required)
- Integrates seamlessly with scs for database-backed sessions
- High performance with efficient goroutine runtime
- Small dependency footprint

**Alternatives Evaluated**:

- **gin**: More batteries-included framework (conflicts with small-team simplicity principle)
- **chi**: Lightweight router (adds minimal value over Go 1.22+ stdlib routing)
- **echo**: Higher-level abstractions (requires more framework-specific patterns)

**Design Philosophy**: Prefer small, composable libraries over full frameworks per [simplicity principle](../01-principles/README.md).

**Cross-Reference**: See [Principles: Architectural Principles](../01-principles/README.md) for complete design philosophy.

### Core Dependencies

```go
import (
    "net/http"
    "html/template"
    "embed"

    "github.com/alexedwards/scs/v2"
    "github.com/gorilla/csrf"
)
```

**Dependency Purposes**:

- **net/http**: HTTP routing and request handling (stdlib)
- **html/template**: Server-side template rendering (stdlib)
- **embed**: Static asset embedding (stdlib)
- **scs**: Database-backed session storage
- **gorilla/csrf**: CSRF protection middleware

## Middleware Integration

### Middleware Chain Architecture

**Middleware Order** (outer to inner):

```go
mux := http.NewServeMux()
mux.HandleFunc("POST /rule", createRule)
mux.HandleFunc("GET /rule", listRules)

// Middleware chain wrapping (outer to inner)
handler := loggingMiddleware(
    sessionManager.LoadAndSave(
        csrf.Protect(csrfKey)(
            forcePasswordChangeMiddleware(mux),
        ),
    ),
)
```

**Execution Order**: Request flows outer -> inner, response flows inner -> outer.

**Example Flow**:

1. Request arrives -> loggingMiddleware (logging)
2. -> sessionManager.LoadAndSave (load session)
3. -> csrf.Protect (validate CSRF token)
4. -> forcePasswordChangeMiddleware (check force_password_change)
5. -> Route handler (listRules)
6. Response -> forcePasswordChangeMiddleware
7. -> csrf.Protect (add CSRF cookie)
8. -> sessionManager.LoadAndSave (save session)
9. -> loggingMiddleware (log response)

### Session Management Middleware

**Implementation**:

```go
import (
    "github.com/alexedwards/scs/v2"
    "github.com/alexedwards/scs/v2/memstore"
    "time"
)

// Create session manager with database store
sessionManager := scs.New()
sessionManager.Store = memstore.New() // Replace with database store for production
sessionManager.Lifetime = 24 * time.Hour // 24 hours default
sessionManager.Cookie.HttpOnly = true
sessionManager.Cookie.Secure = true // Set dynamically based on X-Forwarded-Proto
sessionManager.Cookie.SameSite = http.SameSiteLaxMode
sessionManager.Cookie.Name = "tk_session"

// Wrap handler with session middleware
handler := sessionManager.LoadAndSave(mux)
```

**Session Configuration**:

- **Storage**: Database-backed (sessions table)
- **Expiry**: Sliding expiration (24 hours default, 30 days with "remember me")
- **Cookie Name**: `tk_session` (configurable)
- **Cookie Security**: Secure, HttpOnly, SameSite=Lax (production)

**Cross-Reference**: See [Security: Authentication (Web UI)](../06-security/authentication-web-ui.md) for complete cookie security configuration.

### CSRF Protection Middleware

**Implementation**:

```go
import (
    "github.com/gorilla/csrf"
)

// Configure CSRF protection (32-byte key for production)
csrfKey := []byte("32-byte-long-secret-for-csrf-protection!")
csrfMiddleware := csrf.Protect(
    csrfKey,
    csrf.Secure(true),           // Set dynamically based on X-Forwarded-Proto
    csrf.SameSite(csrf.SameSiteLaxMode),
    csrf.Path("/"),
)

// Wrap handler with CSRF protection
handler := sessionManager.LoadAndSave(
    csrfMiddleware(mux),
)
```

**CSRF Strategy**:

- **Pattern**: Double-submit cookie
- **Token Generation**: On session creation
- **Token Validation**: Automatic on POST/PUT/DELETE/PATCH
- **Exempt Routes**: GET, HEAD, OPTIONS, `/healthz`, `/readyz`, `/static/*`

**Template Integration**:

```html
<form method="POST" action="/rule/create">
  <input type="hidden" name="csrf_token" value="{{ .CSRFToken }}" />
  <!-- form fields -->
</form>
```

**Error Handling**:

```
403 Forbidden
CSRF token validation failed. Please refresh and try again.
```

**Cross-Reference**: See [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) for CSRF validation specifications.

### Custom Redirect Middleware

**Force Password Change Flow**:

```go
import (
    "net/http"
)

func forcePasswordChangeMiddleware(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        // Check if user authenticated and force_password_change flag set
        ctx := r.Context()
        userID := sessionManager.GetString(ctx, "user_id")
        forceChange := sessionManager.GetBool(ctx, "force_password_change")

        if userID != "" && forceChange {
            // Allow only /change-password endpoint
            if r.URL.Path != "/change-password" && r.URL.Path != "/logout" {
                http.Redirect(w, r, "/change-password", http.StatusSeeOther)
                return
            }
        }

        next.ServeHTTP(w, r)
    })
}
```

**Rationale**: Middleware-based redirect ensures force password change cannot be bypassed.

## Web UI Routes

Complete route table for the Web UI service.

### Route Table

| Method | Path               | Handler              | Required Role | Description             |
| ------ | ------------------ | -------------------- | ------------- | ----------------------- |
| GET    | `/login`           | loginPage            | -             | Login form              |
| POST   | `/login`           | loginSubmit          | -             | Process login           |
| POST   | `/logout`          | logout               | observer      | End session             |
| GET    | `/change-password` | changePasswordPage   | observer      | Password change form    |
| POST   | `/change-password` | changePasswordSubmit | observer      | Process password change |
| GET    | `/dashboard`       | dashboard            | observer      | Main dashboard          |
| GET    | `/rules`           | listRules            | observer      | List all rules          |
| GET    | `/rules/create`    | createRulePage       | operator      | Create rule form        |
| POST   | `/rules`           | createRule           | operator      | Create new rule         |
| GET    | `/rules/{id}`      | viewRule             | observer      | View rule details       |
| GET    | `/rules/{id}/edit` | editRulePage         | operator      | Edit rule form          |
| PUT    | `/rules/{id}`      | updateRule           | operator      | Update existing rule    |
| DELETE | `/rules/{id}`      | deleteRule           | operator      | Delete rule             |
| GET    | `/users`           | listUsers            | admin         | List all users          |
| GET    | `/users/create`    | createUserPage       | admin         | Create user form        |
| POST   | `/users`           | createUser           | admin         | Create new user         |
| DELETE | `/users/{id}`      | deleteUser           | admin         | Delete user             |
| PUT    | `/users/{id}/role` | updateUserRole       | admin         | Change user role        |
| GET    | `/healthz`         | healthCheck          | -             | Liveness probe          |
| GET    | `/readyz`          | readinessCheck       | -             | Readiness probe         |
| GET    | `/static/*`        | staticFiles          | -             | Static assets           |

**Role Enforcement**: Routes with Required Role are protected by `RequireRole` middleware. See [Web UI Authentication](../06-security/authentication-web-ui.md) for role definitions (admin, operator, observer).

**Cross-Reference**: See [Health Endpoints](health-endpoints.md) for `/healthz` and `/readyz` specifications.

## Server-Side Rendering with html/template

### Template Engine Configuration

**Template Loading with embed.FS**:

```go
import (
    "embed"
    "html/template"
)

//go:embed templates/*
var templateFS embed.FS

// Parse templates at startup
templates := template.Must(template.ParseFS(templateFS, "templates/*.html"))
```

**Template Structure**:

```
templates/
├── base.html           # Base layout with header/footer
├── login.html          # Login form
├── rules-list.html     # Rules listing
├── rules-create.html   # Create rule form
├── rules-edit.html     # Edit rule form
├── users-list.html     # Users listing
└── users-create.html   # Create user form
```

### Template Example

**Base Template** (`templates/base.html`):

```html
<!DOCTYPE html>
<html>
  <head>
    <title>{{block "title" .}}TrapperKeeper{{end}}</title>
    <link rel="stylesheet" href="/static/css/style.css?v={{ .AssetHash }}" />
  </head>
  <body>
    {{block "content" .}}{{end}}
  </body>
</html>
```

**Child Template** (`templates/rules-list.html`):

```html
{{define "title"}}Rules - TrapperKeeper{{end}} {{define "content"}}
<h1>Rules</h1>
<ul>
  {{range .Rules}}
  <li data-testid="rule-{{.RuleID}}">{{.Name}}</li>
  {{end}}
</ul>
{{end}}
```

### Handler Integration

```go
import (
    "net/http"
    "html/template"
)

type RulesListData struct {
    Rules     []Rule
    CSRFToken string
}

func listRules(w http.ResponseWriter, r *http.Request) {
    ctx := r.Context()
    rules := fetchRules(ctx) // Fetch from database

    csrfToken := csrf.Token(r)
    data := RulesListData{
        Rules:     rules,
        CSRFToken: csrfToken,
    }

    err := templates.ExecuteTemplate(w, "rules-list.html", data)
    if err != nil {
        http.Error(w, "Template rendering failed", http.StatusInternalServerError)
        return
    }
}
```

**HTML Escaping**: html/template automatically escapes HTML by default (prevents XSS).

**Cross-Reference**: See [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) for HTML escaping specifications.

## Form Validation

### Validation Library Integration

```go
import (
    "github.com/go-playground/validator/v10"
)

var validate = validator.New()
```

### Struct-Level Validation

```go
type CreateRuleForm struct {
    Name         string `validate:"required,min=1,max=128"`
    Description  string `validate:"max=1024"`
    ContactEmail string `validate:"omitempty,email"`
}

// Validate method
func (f *CreateRuleForm) Validate() error {
    return validate.Struct(f)
}
```

### Form Parsing and Validation Helper

```go
import (
    "net/http"
    "encoding/json"
)

func parseAndValidateForm(r *http.Request, form interface{}) error {
    if err := r.ParseForm(); err != nil {
        return err
    }

    // Manual form field mapping or use schema decoder
    // Example: form.Name = r.FormValue("name")

    // Validate using validator
    if validatable, ok := form.(interface{ Validate() error }); ok {
        return validatable.Validate()
    }

    return nil
}
```

### Handler Pattern

```go
func createRule(w http.ResponseWriter, r *http.Request) {
    var form CreateRuleForm

    // Parse and validate form
    if err := parseAndValidateForm(r, &form); err != nil {
        // Render form with validation errors
        renderFormWithErrors(w, form, err)
        return
    }

    // Validation passed, process form
    ruleID, err := insertRule(r.Context(), &form)
    if err != nil {
        http.Error(w, "Failed to create rule", http.StatusInternalServerError)
        return
    }

    http.Redirect(w, r, "/rule/"+ruleID, http.StatusSeeOther)
}
```

**HTTP Status Codes**:

- **400 Bad Request**: Validation failure (return rendered form with errors)
- **422 Unprocessable Entity**: Semantic validation failure (duplicate rule name)

**Cross-Reference**: See [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) for form validation specifications.

## Static Asset Serving

### Embedded Assets Strategy

**Implementation** (using `embed.FS`):

```go
import (
    "embed"
    "net/http"
    "io/fs"
)

//go:embed static/*
var staticFS embed.FS

// Create http.FileServer with embedded filesystem
func serveStatic() http.Handler {
    // Strip "static/" prefix from embedded paths
    staticSub, _ := fs.Sub(staticFS, "static")
    fileServer := http.FileServer(http.FS(staticSub))

    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        // Set cache headers
        w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
        fileServer.ServeHTTP(w, r)
    })
}
```

**Route Configuration**:

```go
mux := http.NewServeMux()
mux.Handle("/static/", http.StripPrefix("/static/", serveStatic()))
// other routes
```

### Cache Busting

**Build-Time Hash Injection** (generate at build time):

```go
package main

import (
    "crypto/sha256"
    "fmt"
    "io"
    "os"
    "path/filepath"
)

// assetHashes maps filenames to content hashes (generated at build time)
var assetHashes = map[string]string{
    // Populated by build script or init()
}

func init() {
    // Walk static directory and compute hashes
    filepath.Walk("static", func(path string, info os.FileInfo, err error) error {
        if err != nil || info.IsDir() {
            return err
        }

        f, _ := os.Open(path)
        defer f.Close()

        h := sha256.New()
        io.Copy(h, f)
        hash := fmt.Sprintf("%x", h.Sum(nil))[:8]

        relPath, _ := filepath.Rel("static", path)
        assetHashes[relPath] = hash
        return nil
    })
}

func assetHash(filename string) string {
    if hash, ok := assetHashes[filename]; ok {
        return hash
    }
    return ""
}
```

**Template Usage**:

```html
<link rel="stylesheet" href="/static/css/style.css?v={{ .AssetHash }}" />
<img src="/static/images/logo.png?v={{ .LogoHash }}" alt="Logo" />
```

**Cache Headers**:

- **With versioning**: `Cache-Control: public, max-age=31536000, immutable`
- **Without versioning** (dev mode): `Cache-Control: no-cache`

### Directory Structure

```
static/
├── css/
│   ├── style.css
│   └── forms.css
├── images/
│   └── logo.png
└── fonts/
    └── inter.woff2
```

## Testing Integration

### HTTP Test Utilities

```go
import (
    "net/http"
    "net/http/httptest"
    "strings"
    "testing"
)

func TestCreateRuleSuccess(t *testing.T) {
    mux := createApp()

    formData := strings.NewReader("name=TestRule&description=Test")
    req := httptest.NewRequest("POST", "/rule/create", formData)
    req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

    rr := httptest.NewRecorder()
    mux.ServeHTTP(rr, req)

    if rr.Code != http.StatusSeeOther {
        t.Errorf("Expected status 303, got %d", rr.Code)
    }
}
```

### HTML Parsing with goquery

```go
import (
    "github.com/PuerkitoBio/goquery"
    "net/http/httptest"
    "strings"
    "testing"
)

func TestRulesListRendering(t *testing.T) {
    mux := createApp()

    req := httptest.NewRequest("GET", "/rule", nil)
    rr := httptest.NewRecorder()
    mux.ServeHTTP(rr, req)

    doc, err := goquery.NewDocumentFromReader(rr.Body)
    if err != nil {
        t.Fatal(err)
    }

    rules := doc.Find("[data-testid^='rule-']").Length()
    if rules == 0 {
        t.Error("Expected at least one rule in listing")
    }
}
```

**data-testid Attributes**: Use for reliable element selection in tests:

```html
<input type="text" name="name" data-testid="rule-name-input" />
<div class="error" data-testid="rule-name-error">{{ .Error }}</div>
```

**Cross-Reference**: See [Principles: Testing Philosophy](../01-principles/testing-philosophy.md) for complete web UI testing patterns.

## Related Documents

**Dependencies** (read these first):

- [Operations Overview](README.md): Strategic context for web framework selection
- [Architecture: Service Architecture](../02-architecture/README.md): Web UI service definition

**Related Spokes** (siblings in this hub):

- [CLI Design](cli-design.md): `web-ui` subcommand configuration

**Security References**:

- [Security: Authentication (Web UI)](../06-security/authentication-web-ui.md): Session management, cookie security, force password change flow
- [Security: TLS/HTTPS Strategy](../06-security/tls-https-strategy.md): Middleware for TLS configuration

**Validation References**:

- [Validation: Unified Validation and Input Sanitization](../07-validation/README.md): CSRF validation (Section 3.6), HTML escaping (Section 4.2), error handling (Section 5.1)

**Testing References**:

- [Principles: Testing Philosophy](../01-principles/testing-philosophy.md): Web UI testing with net/http test utilities and goquery
