---
doc_type: spoke
status: active
primary_category: architecture
hub_document: doc/10-integration/README.md
tags:
  - dependencies
  - versioning
  - vendoring
  - go-modules
---

# Dependency Management

## Context

TrapperKeeper requires consistent dependency management across Go, Python, and Java
ecosystems. This document specifies versioning policies, vendoring strategy, and
rationale for language-specific approaches.

**Hub Document**: This spoke is part of [Integration Overview](README.md). See the
hub for strategic context on module architecture.

## Go Version Policy

**Policy**: Always use the latest stable Go release.

The go.mod `go` directive reflects the current stable Go version at project
inception or last major update. Upgrade to new stable releases as part of regular
maintenance.

**Rationale**: Go maintains excellent backwards compatibility. Staying current
provides security patches, performance improvements, and language features without
migration burden.

## Go Dependency Versioning

### Initial Selection

**Policy**: Use the latest stable version when first adding a dependency.

When adding a new dependency to go.mod, select the current stable release. This
ensures access to recent bug fixes and features while avoiding pre-release
instability.

### Ongoing Maintenance

**Policy**: Upgrade minor versions freely; require expert decision for major
versions.

Versioning follows semantic versioning conventions:

```
v1.2.3 -> v1.2.4   (patch)   -> upgrade automatically, no review required
v1.2.3 -> v1.3.0   (minor)   -> upgrade automatically, no review required
v1.2.3 -> v2.0.0   (major)   -> expert decision required
```

**Minor/Patch Upgrades**: Apply when available. These contain bug fixes, security
patches, and backwards-compatible features. No justification required.

**Major Upgrades**: Require explicit justification:

- Functionality needed that only exists in new major version
- Security vulnerability in current major version without backported fix
- Upstream end-of-life or abandonment of current major version
- Significant performance or correctness improvements

**Decision Authority**: Senior engineer or team lead makes major version upgrade
decisions on a case-by-case basis.

### Rationale

Minor versions within a major release maintain API compatibility per semver
conventions. Staying current within a major version minimizes security exposure and
bug accumulation. Major version upgrades may introduce breaking changes requiring
code modifications, hence the gated approval.

## Go Vendoring

**Policy**: Vendor all Go dependencies.

The repository includes a `vendor/` directory containing all Go dependencies,
committed to source control.

**Commands**:

```bash
# Add/update dependencies
go get github.com/example/package@v1.2.3

# Sync vendor directory
go mod vendor

# Verify vendor matches go.sum
go mod verify
```

**Repository Structure**:

```
trapperkeeper/
  go.mod
  go.sum
  vendor/           # Committed to source control
    modules.txt
    github.com/
    google.golang.org/
    ...
```

**Rationale**:

- **Reproducible builds**: Build succeeds without network access to module proxies
- **Audit trail**: Dependency changes visible in version control diffs
- **Supply chain security**: Immune to upstream repository deletion or modification
- **Build speed**: No network fetches during CI/CD builds
- **Offline development**: Full development capability without internet

**Trade-offs**:

- Larger repository size (typically 50-200MB for vendor/)
- Dependency updates require committing vendor/ changes
- Merge conflicts possible in vendor/ during concurrent updates

## Python Dependency Management

**Policy**: Version pinning via requirements files; no vendoring.

Python dependencies are specified with exact versions in requirements files:

```
# requirements.txt (production)
grpcio==1.62.1
protobuf==4.25.3
pandas==2.2.0

# requirements-dev.txt (development)
pytest==8.0.0
mypy==1.8.0
black==24.1.0
```

**Version Updates**: Follow the same policy as Go -- minor/patch versions upgrade
freely, major versions require expert decision.

**Why No Vendoring**:

Vendoring Python dependencies is impractical:

- **Binary wheels**: Many packages (numpy, pandas, grpcio) distribute
  platform-specific compiled binaries. Vendoring would require bundling binaries
  for all target platforms (linux/x86_64, linux/arm64, darwin/x86_64,
  darwin/arm64, windows/x86_64).

- **C extensions**: Packages with C extensions require compilation against system
  libraries. Vendored source would need build toolchains on every developer
  machine.

- **Virtual environments**: Python's package isolation model expects dependencies
  installed into virtualenvs, not imported from local directories.

- **Tooling assumptions**: pip, poetry, and other tools assume network-based
  dependency resolution. Fighting this creates maintenance burden.

**Mitigation**:

- Pin exact versions for reproducibility
- Use `pip-compile` (pip-tools) to generate locked requirements from looser
  constraints
- Consider private PyPI mirror for supply chain concerns
- CI caches downloaded wheels for build speed

## Java Dependency Management

**Policy**: Version pinning via Gradle/Maven configuration; no vendoring.

Java dependencies are specified with exact versions in build configuration:

```groovy
// build.gradle
dependencies {
    implementation 'io.grpc:grpc-netty:1.62.1'
    implementation 'io.grpc:grpc-protobuf:1.62.1'
    implementation 'com.google.protobuf:protobuf-java:3.25.3'
}
```

**Version Updates**: Follow the same policy as Go -- minor/patch versions upgrade
freely, major versions require expert decision.

**Why No Vendoring**:

Vendoring Java dependencies is impractical:

- **Transitive dependencies**: Java libraries have deep transitive dependency
  trees. A single library may pull hundreds of JARs. Vendoring requires
  flattening and maintaining this entire tree manually.

- **Repository conventions**: Maven Central and Gradle's dependency resolution
  assume repository-based fetching. Build tools have no first-class vendoring
  support.

- **Enterprise tooling**: Organizations typically operate Nexus/Artifactory
  mirrors. Vendoring conflicts with this established pattern.

- **Dependency conflicts**: Java's classpath model requires careful version
  resolution (dependency mediation). Vendored JARs bypass this, risking runtime
  NoSuchMethodError and similar failures.

**Mitigation**:

- Pin exact versions for reproducibility
- Use Gradle's dependency locking (`gradle dependencies --write-locks`)
- Consider Nexus/Artifactory mirror for supply chain concerns
- Gradle caches downloaded JARs for build speed

## Summary Matrix

| Aspect            | Go                   | Python               | Java                 |
| ----------------- | -------------------- | -------------------- | -------------------- |
| Version selection | Latest stable on add | Latest stable on add | Latest stable on add |
| Patch/minor       | Upgrade freely       | Upgrade freely       | Upgrade freely       |
| Major version     | Expert decision      | Expert decision      | Expert decision      |
| Vendoring         | Yes (vendor/)        | No (not feasible)    | No (not feasible)    |
| Pinning mechanism | go.mod + go.sum      | requirements.txt     | build.gradle         |
| Lock file         | go.sum               | requirements.txt     | gradle.lockfile      |

## Related Documents

**Dependencies** (read these first):

- [Integration Overview](README.md): Strategic context for module architecture
- [Monorepo Structure](monorepo-structure.md): Repository layout including vendor/

**Related Spokes** (siblings in this hub):

- [Package Separation](package-separation.md): How dependencies flow between
  internal packages

**Implements**:

- [Architecture: Binary Distribution](../02-architecture/binary-distribution.md):
  Vendoring enables reproducible binary builds
