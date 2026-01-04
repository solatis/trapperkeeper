---
doc_type: spoke
status: active
primary_category: architecture
hub_document: doc/03-data/README.md
tags:
  - timestamps
  - timezone
  - presentation
  - cookies
---

# Timezone Presentation

## Context

TrapperKeeper stores all timestamps in UTC (see [Timestamp Representation](timestamps.md)). This document specifies how UTC timestamps are converted to user-local time for display in the web UI and other presentation contexts.

**Hub Document**: This document is part of the Data Hub. See [Data Architecture](README.md) for strategic overview.

## Design Decision

**Approach**: Server-side timezone conversion with browser-detected timezone via cookie.

**Mechanism**: Browser JavaScript detects the user's timezone once, stores it in a cookie. Server reads the cookie and formats all timestamps in templates before rendering.

### Why Server-Side with Browser Detection

This decision was reached after systematic analysis of alternatives. The key insight: timezone is user context (like locale or authentication), not display logic. Once detected, it should be available throughout the system.

**Alternatives Considered**:

| Approach                   | Rejected Because                                                                                                                              |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Browser-only JS conversion | Requires JavaScript on every page for timestamp display; creates flash-of-UTC on every page load; blocks server-side timezone-aware reporting |
| User preference storage    | UX friction too high; users won't configure; traveling users get wrong times                                                                  |
| System timezone            | Doesn't work for multi-tenant cloud deployment                                                                                                |

**Decision Drivers**:

1. **Minimal JavaScript**: Browser-only requires JS on every page. Server-side requires ~10 lines for one-time cookie detection, then zero JS for display.

2. **No flash-of-UTC**: Browser-only shows UTC briefly on every page load until JS runs. Server-side shows UTC only on very first visit (before cookie is set).

3. **Reporting capability**: Server-side timezone context enables timezone-aware aggregations ("events per hour in local time"). Browser-only makes this impossible without fetching all data to client.

4. **Export consistency**: Server can use session timezone for exports automatically. Scheduled/API exports can use stored preference or explicit parameter.

5. **Multi-tenant cloud**: Works automatically -- each user's browser provides their timezone.

## Implementation

### Browser-Side: Timezone Detection

Inline JavaScript in HTML `<head>` detects timezone and sets cookie:

```html
<script>
  (function () {
    if (!document.cookie.includes("tz=")) {
      var tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
      document.cookie =
        "tz=" +
        encodeURIComponent(tz) +
        ";path=/;max-age=31536000;SameSite=Lax";
    }
  })();
</script>
```

**Properties**:

- Runs once per browser (cookie persists for 1 year)
- Executes before page renders (inline in `<head>`)
- Uses `Intl.DateTimeFormat` API (supported in all browsers since 2012)
- Returns IANA timezone string (e.g., "America/New_York", "Europe/London")
- No user permission required

### Server-Side: Middleware

Go middleware reads timezone cookie and adds to request context:

```go
package middleware

import (
    "context"
    "net/http"
    "time"
)

type contextKey string

const TimezoneKey contextKey = "timezone"

func TimezoneMiddleware(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        tz := "UTC" // default fallback
        if cookie, err := r.Cookie("tz"); err == nil {
            if loc, err := time.LoadLocation(cookie.Value); err == nil {
                tz = loc.String()
            }
        }
        ctx := context.WithValue(r.Context(), TimezoneKey, tz)
        next.ServeHTTP(w, r.WithContext(ctx))
    })
}

func GetTimezone(ctx context.Context) *time.Location {
    if tz, ok := ctx.Value(TimezoneKey).(string); ok {
        if loc, err := time.LoadLocation(tz); err == nil {
            return loc
        }
    }
    return time.UTC
}
```

**Error Handling**: Invalid timezone strings fall back to UTC. Malformed cookies are ignored.

**tzdata Dependency**: For minimal container images without system timezone data, embed tzdata:

```go
import _ "time/tzdata"
```

### Template Functions

Template helper formats timestamps in user's timezone:

```go
func templateFuncs(ctx context.Context) template.FuncMap {
    loc := GetTimezone(ctx)
    return template.FuncMap{
        "formatTime": func(t time.Time) string {
            return t.In(loc).Format("Jan 2, 2006, 3:04 PM")
        },
        "formatDate": func(t time.Time) string {
            return t.In(loc).Format("Jan 2, 2006")
        },
        "formatDateTime": func(t time.Time) string {
            return t.In(loc).Format("2006-01-02 15:04:05")
        },
    }
}
```

**Template Usage**:

```html
<td>{{ formatTime .CreatedAt }}</td>
<td>{{ formatDate .EventDate }}</td>
```

### First-Request Behavior

On the very first request (no cookie yet):

1. Server renders timestamps in UTC with explicit label: "2025-01-04 12:00:00 UTC"
2. Inline JS in `<head>` sets timezone cookie
3. All subsequent requests use user's local timezone

This flash-of-UTC happens only once per browser, not on every page load.

## Downstream Uses

### Reporting

Server has timezone in request context, enabling timezone-aware queries:

```go
func GetEventsPerHour(ctx context.Context, db *sql.DB) ([]HourlyCount, error) {
    loc := GetTimezone(ctx)
    // Use timezone for aggregation logic
    // ...
}
```

For database-level timezone aggregation (if ever needed), pass timezone as query parameter rather than relying on database session settings.

### Exports

**Interactive Exports** (user clicks "Export CSV"):

Server uses session timezone automatically. No additional parameters needed.

**Scheduled/API Exports**:

Require explicit timezone parameter or stored user preference:

```
GET /api/v1/export?format=csv&tz=America/New_York
```

If no timezone provided, export uses UTC with explicit labeling in output.

### Email Notifications

If email notifications are added (future), use stored user timezone preference. Fall back to UTC with explicit label if preference not set.

## Format Standards

### Display Formats

| Context       | Format                 | Example              |
| ------------- | ---------------------- | -------------------- |
| Full datetime | `Jan 2, 2006, 3:04 PM` | Jan 4, 2025, 7:00 AM |
| Date only     | `Jan 2, 2006`          | Jan 4, 2025          |
| Time only     | `3:04 PM`              | 7:00 AM              |
| ISO (exports) | `2006-01-02 15:04:05`  | 2025-01-04 07:00:00  |

### UTC Fallback Label

When displaying UTC (first request or explicit choice):

```
2025-01-04 12:00:00 UTC
```

Always include "UTC" suffix to avoid ambiguity.

## Security Considerations

**Cookie Validation**: Timezone cookie value is validated against Go's timezone database. Invalid values fall back to UTC. This prevents:

- Injection attacks via malformed timezone strings
- Crashes from invalid `time.LoadLocation` calls

**No Sensitive Data**: Timezone is not sensitive information. Cookie does not require encryption or HMAC.

## Related Documents

**Dependencies** (read these first):

- [Timestamp Representation](timestamps.md): UTC storage design, database-specific types

**Related Spokes** (siblings in this hub):

- [Event Schema and Storage](event-schema-storage.md): Timestamp field definitions

**Operations References**:

- [Web Framework](../09-operations/web-framework.md): Middleware integration
- [Configuration Management](../09-operations/configuration.md): Default timezone fallback configuration
