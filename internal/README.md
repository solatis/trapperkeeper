# Internal Packages

## Overview

TrapperKeeper uses Go's internal/ package mechanism to enforce clean
architectural boundaries between client-side (SDK) and server-side concerns. The
package split enables SDKs to share rule evaluation logic without bundling
server dependencies.

## Architecture

```
internal/
|-- types/      Zero-dependency domain models (encoding/json only)
|-- rules/      Rule compilation and evaluation (depends on types/)
|-- protobuf/   Generated proto code (DO NOT EDIT, depends on grpc/protobuf)
|-- core/       Server-side code: db, auth, config (depends on rules/, proto/)
```

## Design Decisions

Dependency direction is strictly acyclic with types/ at the bottom:

- types/ has zero external dependencies to minimize SDK binary size (~10KB
  impact)
- rules/ depends only on types/, not proto/ -- conversion happens at boundary
  layer
- core/ depends on rules/ and proto/, consolidating server concerns
- protobuf/ is generated code with no handwritten dependencies

The boundary layer pattern: Proto types live in internal/protobuf, domain types
in internal/types. Conversion between them happens in the calling layer (SDK or
API service), not within internal/rules. This keeps rule evaluation logic
portable across proto versions.

## Invariants

- No circular dependencies between internal/ packages
- types/ must remain zero-dependency (encoding/json only)
- rules/ must not import internal/protobuf (boundary layer separation)
- Generated code in protobuf/ must never be manually edited
