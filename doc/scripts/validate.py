#!/usr/bin/env python3
"""Documentation validation tool for Trapperkeeper."""

import argparse
import sys
import re
import yaml
from pathlib import Path
from typing import List, Dict, Optional, Any
from rule_evaluator import RuleEvaluator, ValidationError


# Global template cache for performance optimization
_template_cache: Dict[Path, Dict[str, Any]] = {}


# Complexity limits for validation blocks
COMPLEXITY_LIMITS = {
    'max_frontmatter_lines': 40,
    'max_conditions': 5,
    'max_required_sections': 10,
    'max_forbidden_patterns': 8,
}


def discover_docs() -> List[Path]:
    """Find all documentation files, excluding meta-docs and CLAUDE.md."""
    doc_root = Path(__file__).parent.parent
    all_docs = list(doc_root.glob('**/*.md'))

    # Exclude patterns
    excluded = []
    for doc in all_docs:
        rel_path = doc.relative_to(doc_root)

        # Exclude _meta/ directory
        if '_meta' in rel_path.parts:
            continue

        # Exclude scripts/ directory (contains fixtures and tooling)
        if 'scripts' in rel_path.parts:
            continue

        # Exclude doc/CLAUDE.md (but allow subdirectory CLAUDE.md files)
        if doc.name == 'CLAUDE.md' and len(rel_path.parts) == 1:
            continue

        excluded.append(doc)

    return sorted(excluded)


def extract_frontmatter(file_path: Path) -> Optional[Dict[str, any]]:
    """Extract YAML frontmatter from markdown file using stdlib only.

    Args:
        file_path: Path to the markdown file

    Returns:
        Dictionary of frontmatter fields, or None if no frontmatter found

    Examples:
        For a file with frontmatter like:
        ---
        status: active
        tags:
          - validation
          - testing
        ---

        Returns: {'status': 'active', 'tags': ['validation', 'testing']}
    """
    content = file_path.read_text(encoding='utf-8')

    # Match frontmatter block between --- markers
    pattern = r'^---\s*\n(.*?)\n---\s*\n'
    match = re.match(pattern, content, re.DOTALL)

    if not match:
        return None

    yaml_block = match.group(1)
    frontmatter = {}

    current_key = None
    current_list = []

    for line in yaml_block.split('\n'):
        line = line.rstrip()

        # Skip empty lines
        if not line:
            continue

        # List item
        if line.startswith('  - '):
            if current_key:
                current_list.append(line[4:].strip())
            continue

        # Key-value pair
        if ':' in line and not line.startswith(' '):
            # Save previous list if exists
            if current_key and current_list:
                frontmatter[current_key] = current_list
                current_list = []

            key, _, value = line.partition(':')
            key = key.strip()
            value = value.strip()

            if value:
                frontmatter[key] = value
                current_key = None
            else:
                # Start of list
                current_key = key

    # Save final list
    if current_key and current_list:
        frontmatter[current_key] = current_list

    return frontmatter


def validate_frontmatter_fields(doc: Path, fm: Dict) -> List[str]:
    """Validate frontmatter fields against schema.

    Args:
        doc: Path to the document being validated
        fm: Frontmatter dictionary extracted from the document

    Returns:
        List of error messages (empty if validation passes)
    """
    errors = []

    # Required fields (all doc types)
    required = ['doc_type', 'status', 'date_created', 'primary_category']
    for field in required:
        if field not in fm:
            errors.append(f"[ERROR] {doc}: Missing required field: {field}")

    # Validate enums
    if 'doc_type' in fm:
        valid_types = ['spoke', 'hub', 'index', 'guide', 'reference', 'redirect-stub']
        if fm['doc_type'] not in valid_types:
            errors.append(f"[ERROR] {doc}: Invalid doc_type: {fm['doc_type']}")

    if 'status' in fm:
        valid_statuses = ['draft', 'active', 'deprecated', 'superseded']
        if fm['status'] not in valid_statuses:
            errors.append(f"[ERROR] {doc}: Invalid status: {fm['status']}")

    # Validate date format
    date_pattern = r'^\d{4}-\d{2}-\d{2}$'
    for date_field in ['date_created', 'date_updated']:
        if date_field in fm and not re.match(date_pattern, fm[date_field]):
            errors.append(f"[ERROR] {doc}: Invalid {date_field} format: {fm[date_field]}")

    # Conditional validations
    if fm.get('status') == 'superseded' and 'superseded_by' not in fm:
        errors.append(f"[ERROR] {doc}: status=superseded requires superseded_by field")

    if fm.get('doc_type') == 'spoke' and 'hub_document' not in fm:
        errors.append(f"[ERROR] {doc}: doc_type=spoke requires hub_document field")

    if fm.get('doc_type') == 'hub':
        spokes = fm.get('consolidated_spokes', [])
        if not isinstance(spokes, list):
            errors.append(f"[ERROR] {doc}: consolidated_spokes must be a list")
        elif len(spokes) < 3:
            errors.append(f"[ERROR] {doc}: Hub requires minimum 3 consolidated_spokes (found {len(spokes)})")

    # Forbidden fields
    forbidden = ['version', 'revision', 'changelog', 'history', 'versions', 'revisions']
    for field in forbidden:
        if field in fm:
            errors.append(f"[ERROR] {doc}: Forbidden field: {field} (use git for history)")

    return errors


def validate_frontmatter(args) -> int:
    """Validate frontmatter against schema requirements.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors
    """
    docs = discover_docs()
    errors = []

    for doc in docs:
        fm = extract_frontmatter(doc)

        if not fm:
            errors.append(f"[ERROR] {doc}: No frontmatter found")
            continue

        doc_errors = validate_frontmatter_fields(doc, fm)
        errors.extend(doc_errors)

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"Frontmatter validation passed: {len(docs)} documents")
    return 0


def resolve_spoke_path(from_doc: Path, reference: str) -> Path:
    """Resolve a spoke reference to absolute path.

    Args:
        from_doc: Path to the document containing the reference
        reference: String reference to the spoke (may be relative or doc/ prefixed)

    Returns:
        Absolute Path to the referenced spoke

    Examples:
        resolve_spoke_path(/path/to/doc/06-security/README.md, "authentication-web-ui.md")
        -> /path/to/doc/06-security/authentication-web-ui.md

        resolve_spoke_path(/path/to/doc/06-security/README.md, "doc/06-security/authentication-web-ui.md")
        -> /path/to/doc/06-security/authentication-web-ui.md

        resolve_spoke_path(/path/to/doc/01-principles/README.md, "testing-philosophy")
        -> /path/to/doc/01-principles/testing-philosophy.md
    """
    doc_root = Path(__file__).parent.parent

    # Add .md extension if not present
    if not reference.endswith('.md'):
        reference = reference + '.md'

    # If reference starts with doc/, it's an absolute reference from project root
    if reference.startswith('doc/'):
        # Remove doc/ prefix and resolve from project root
        rel_reference = reference[4:]  # Remove "doc/"
        return (doc_root / rel_reference).resolve()

    # Otherwise it's relative to the document containing the reference
    return (from_doc.parent / reference).resolve()


def paths_match(path1: Path, path_str: str, relative_to: Path) -> bool:
    """Check if path1 matches path_str reference.

    Args:
        path1: Absolute path to compare
        path_str: String reference (may be relative or doc/ prefixed)
        relative_to: Path from which path_str is relative

    Returns:
        True if path1 matches the resolved path_str reference

    Examples:
        paths_match(Path("/path/to/doc/06-security/tls.md"), "tls.md", Path("/path/to/doc/06-security/README.md"))
        -> True
    """
    resolved = resolve_spoke_path(relative_to, path_str)
    return path1.resolve() == resolved


def validate_hub_spoke(args) -> int:
    """Validate bidirectional hub-spoke relationships.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Validation checks:
    - All hubs have consolidated_spokes list
    - All spokes in consolidated_spokes exist
    - All spokes have hub_document back-reference
    - 100% back-reference compliance required
    - No orphaned spokes (spoke points to hub but hub doesn't list spoke)
    """
    docs = discover_docs()
    errors = []

    # Load all documents with frontmatter
    doc_metadata = {}
    for doc in docs:
        fm = extract_frontmatter(doc)
        if fm:
            doc_metadata[doc] = fm

    # Find all hubs
    hubs = {doc: fm for doc, fm in doc_metadata.items() if fm.get('doc_type') == 'hub'}

    # Validate each hub
    for hub_path, hub_fm in hubs.items():
        spokes = hub_fm.get('consolidated_spokes', [])

        if not spokes:
            errors.append(f"[ERROR] Hub {hub_path}: No consolidated_spokes listed")
            continue

        # Check each spoke
        missing_refs = 0
        total_spokes = len(spokes)

        for spoke_ref in spokes:
            # Resolve spoke path (may be relative)
            spoke_path = resolve_spoke_path(hub_path, spoke_ref)

            if not spoke_path.exists():
                errors.append(f"[ERROR] Hub {hub_path}: References non-existent spoke: {spoke_ref}")
                missing_refs += 1
                continue

            # Check back-reference
            spoke_fm = extract_frontmatter(spoke_path)
            if not spoke_fm:
                errors.append(f"[ERROR] Spoke {spoke_path}: No frontmatter found")
                missing_refs += 1
                continue

            hub_doc_field = spoke_fm.get('hub_document')
            if not hub_doc_field:
                errors.append(f"[ERROR] Spoke {spoke_path}: Missing hub_document back-reference")
                missing_refs += 1
            elif not paths_match(hub_path, hub_doc_field, spoke_path):
                errors.append(f"[ERROR] Spoke {spoke_path}: hub_document points to {hub_doc_field}, expected reference to {hub_path}")
                missing_refs += 1

        # Check 100% back-reference compliance
        compliance = ((total_spokes - missing_refs) / total_spokes * 100) if total_spokes > 0 else 0
        if compliance < 100:
            errors.append(f"[ERROR] Hub {hub_path}: Back-reference compliance {compliance:.0f}% (required: 100%)")

    # Check for orphaned spokes
    all_spokes = {doc: fm for doc, fm in doc_metadata.items() if fm.get('doc_type') == 'spoke'}
    for spoke_path, spoke_fm in all_spokes.items():
        hub_ref = spoke_fm.get('hub_document')
        if not hub_ref:
            continue

        # Find the hub
        hub_path = resolve_spoke_path(spoke_path, hub_ref)
        if hub_path not in hubs:
            errors.append(f"[ERROR] Spoke {spoke_path}: References non-existent hub: {hub_ref}")
            continue

        # Check if hub lists this spoke
        hub_spokes = hubs[hub_path].get('consolidated_spokes', [])
        spoke_listed = any(paths_match(spoke_path, s, hub_path) for s in hub_spokes)

        if not spoke_listed:
            errors.append(f"[ERROR] Orphaned spoke {spoke_path}: Not listed in hub {hub_path}")

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"Hub-spoke validation passed: {len(hubs)} hubs, {len(all_spokes)} spokes")
    return 0


def validate_meta_directories(args) -> int:
    """Validate _meta/ subdirectories have README.md hubs.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Validation checks:
    - All numbered subdirectories in _meta/ have README.md
    - README.md files have proper hub frontmatter
    - Hub frontmatter includes consolidated_spokes list
    - Spokes listed in hub exist and have proper back-references
    """
    doc_root = Path(__file__).parent.parent
    meta_dir = doc_root / '_meta'
    errors = []

    if not meta_dir.exists():
        errors.append(f"[ERROR] _meta directory not found at {meta_dir}")
        print('\n'.join(errors), file=sys.stderr)
        return 1

    # Find numbered subdirectories (e.g., 01-standards, 02-templates)
    subdirs = [d for d in meta_dir.iterdir()
               if d.is_dir()
               and not d.name.startswith('.')
               and not d.name.startswith('__')
               and d.name[0].isdigit()]

    if not subdirs:
        print("No numbered _meta subdirectories found")
        return 0

    for subdir in sorted(subdirs):
        readme_path = subdir / 'README.md'

        # Check README.md exists
        if not readme_path.exists():
            errors.append(f"[ERROR] {subdir}: Missing README.md hub document")
            continue

        # Validate README.md has proper frontmatter
        fm = extract_frontmatter(readme_path)
        if not fm:
            errors.append(f"[ERROR] {readme_path}: No frontmatter found")
            continue

        # Check doc_type is hub
        if fm.get('doc_type') != 'hub':
            errors.append(f"[ERROR] {readme_path}: doc_type must be 'hub' (found: {fm.get('doc_type')})")

        # Check consolidated_spokes exists (even if empty list is allowed for _meta)
        if 'consolidated_spokes' not in fm:
            errors.append(f"[ERROR] {readme_path}: Missing consolidated_spokes field")
            continue

        spokes = fm.get('consolidated_spokes', [])
        if not isinstance(spokes, list):
            errors.append(f"[ERROR] {readme_path}: consolidated_spokes must be a list")
            continue

        # Validate each spoke exists and has back-reference
        for spoke_ref in spokes:
            spoke_path = resolve_spoke_path(readme_path, spoke_ref)

            if not spoke_path.exists():
                errors.append(f"[ERROR] {readme_path}: References non-existent spoke: {spoke_ref}")
                continue

            # Check spoke has hub_document back-reference
            spoke_fm = extract_frontmatter(spoke_path)
            if not spoke_fm:
                errors.append(f"[ERROR] {spoke_path}: No frontmatter found")
                continue

            hub_doc_field = spoke_fm.get('hub_document')
            if not hub_doc_field:
                errors.append(f"[ERROR] {spoke_path}: Missing hub_document back-reference")
            elif not paths_match(readme_path, hub_doc_field, spoke_path):
                errors.append(f"[ERROR] {spoke_path}: hub_document points to {hub_doc_field}, expected reference to README.md")

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"Meta-directories validation passed: {len(subdirs)} subdirectories validated")
    return 0


def validate_indexes(args) -> int:
    """Placeholder: Cross-cutting index validation."""
    print("validate-indexes: Not yet implemented (placeholder)")
    return 0


def validate_links(args) -> int:
    """Placeholder: Link integrity validation."""
    print("validate-links: Not yet implemented (placeholder)")
    return 0


def check_hub_freshness(args) -> int:
    """Placeholder: Hub freshness check."""
    print("check-hub-freshness: Not yet implemented (placeholder)")
    return 0


def load_template_validation(template_path: Path, use_cache: bool = True) -> Dict[str, Any]:
    """Load validation rules from template frontmatter with caching.

    Args:
        template_path: Path to template markdown file
        use_cache: Whether to use cached templates (default: True)

    Returns:
        Dictionary containing validation rules from template frontmatter

    Raises:
        ValueError: If template has no frontmatter or no validation section
    """
    # Check cache first
    if use_cache and template_path in _template_cache:
        return _template_cache[template_path]

    content = template_path.read_text(encoding='utf-8')

    # Extract frontmatter between --- markers
    pattern = r'^---\s*\n(.*?)\n---\s*\n'
    match = re.match(pattern, content, re.DOTALL)

    if not match:
        raise ValueError(f"Template {template_path} has no frontmatter")

    yaml_block = match.group(1)

    # Parse YAML using yaml module for full support
    try:
        frontmatter = yaml.safe_load(yaml_block)
    except yaml.YAMLError as e:
        raise ValueError(f"Failed to parse YAML in {template_path}: {e}")

    if 'validation' not in frontmatter:
        raise ValueError(f"Template {template_path} has no 'validation' section in frontmatter")

    validation_rules = frontmatter['validation']

    # Cache the result
    if use_cache:
        _template_cache[template_path] = validation_rules

    return validation_rules


def discover_claude_md() -> List[Path]:
    """Find all CLAUDE.md files in documentation.

    Returns:
        List of paths to CLAUDE.md files
    """
    doc_root = Path(__file__).parent.parent
    return sorted(doc_root.glob('**/CLAUDE.md'))


def validate_claude_md(args) -> int:
    """Validate CLAUDE.md files against template-driven rules.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Uses template-driven validation from doc/_meta/02-templates/claude-md.md
    """
    # Load validation rules from template
    doc_root = Path(__file__).parent.parent
    template_path = doc_root / "_meta" / "02-templates" / "claude-md.md"

    try:
        validation_rules = load_template_validation(template_path)
    except ValueError as e:
        print(f"[ERROR] Failed to load template validation rules: {e}", file=sys.stderr)
        return 1

    # Show effective rules if debug flag is set
    if getattr(args, 'show_effective_rules', False):
        print(f"=== Effective Validation Rules for CLAUDE.md ===")
        print(f"Template: {template_path}")
        print(f"\nParsed Validation Block:")
        print(yaml.dump(validation_rules, default_flow_style=False, sort_keys=False))
        print("=" * 60)
        return 0

    # Discover all CLAUDE.md files
    claude_files = discover_claude_md()
    errors = []

    for claude_path in claude_files:
        content = claude_path.read_text(encoding='utf-8')
        lines = content.split('\n')

        # Check no frontmatter (CLAUDE.md must not have frontmatter)
        if lines and lines[0].strip() == '---':
            errors.append(f"[ERROR] {claude_path}: CLAUDE.md must NOT have YAML frontmatter")
            continue

        # Use RuleEvaluator with template rules
        evaluator = RuleEvaluator(validation_rules)
        validation_errors = evaluator.evaluate(claude_path, content)

        # Convert ValidationError objects to string format
        for ve in validation_errors:
            errors.append(ve.format_error())

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"CLAUDE.md validation passed: {len(claude_files)} files")
    return 0


def validate_hub(args) -> int:
    """Validate hub documents against template-driven rules.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Uses template-driven validation from doc/_meta/02-templates/hub.md
    """
    # Load validation rules from hub template
    doc_root = Path(__file__).parent.parent
    template_path = doc_root / "_meta" / "02-templates" / "hub.md"

    try:
        validation_rules = load_template_validation(template_path)
    except ValueError as e:
        print(f"[ERROR] Failed to load template validation rules: {e}", file=sys.stderr)
        return 1

    # Show effective rules if debug flag is set
    if getattr(args, 'show_effective_rules', False):
        print(f"=== Effective Validation Rules for Hub Documents ===")
        print(f"Template: {template_path}")
        print(f"\nParsed Validation Block:")
        print(yaml.dump(validation_rules, default_flow_style=False, sort_keys=False))
        print("=" * 60)
        return 0

    # Discover all hub documents (README.md files in numbered directories)
    docs = discover_docs()
    hub_docs = [doc for doc in docs if doc.name == "README.md"]

    errors = []

    for hub_path in hub_docs:
        content = hub_path.read_text(encoding='utf-8')

        # Extract frontmatter
        frontmatter = extract_frontmatter(hub_path)

        if not frontmatter:
            errors.append(f"[ERROR] {hub_path}: No frontmatter found")
            continue

        # Check if this is actually a hub document
        if frontmatter.get('doc_type') != 'hub':
            # Skip non-hub README.md files
            continue

        # Use RuleEvaluator with template rules
        from rule_evaluator import RuleEvaluator
        evaluator = RuleEvaluator(validation_rules)
        validation_errors = evaluator.evaluate(hub_path, content, frontmatter)

        # Convert ValidationError objects to string format
        for ve in validation_errors:
            errors.append(ve.format_error())

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"Hub validation passed: {len([d for d in hub_docs if extract_frontmatter(d) and extract_frontmatter(d).get('doc_type') == 'hub'])} documents")
    return 0


def validate_spoke(args) -> int:
    """Validate spoke documents against template-driven rules.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Uses template-driven validation from doc/_meta/02-templates/spoke.md
    """
    # Load validation rules from spoke template
    doc_root = Path(__file__).parent.parent
    template_path = doc_root / "_meta" / "02-templates" / "spoke.md"

    try:
        validation_rules = load_template_validation(template_path)
    except ValueError as e:
        print(f"[ERROR] Failed to load template validation rules: {e}", file=sys.stderr)
        return 1

    # Show effective rules if debug flag is set
    if getattr(args, 'show_effective_rules', False):
        print(f"=== Effective Validation Rules for Spoke Documents ===")
        print(f"Template: {template_path}")
        print(f"\nParsed Validation Block:")
        print(yaml.dump(validation_rules, default_flow_style=False, sort_keys=False))
        print("=" * 60)
        return 0

    # Discover all spoke documents
    docs = discover_docs()
    errors = []

    for spoke_path in docs:
        # Extract frontmatter
        frontmatter = extract_frontmatter(spoke_path)

        if not frontmatter:
            # Skip documents without frontmatter
            continue

        # Check if this is a spoke document
        if frontmatter.get('doc_type') != 'spoke':
            continue

        content = spoke_path.read_text(encoding='utf-8')

        # Use RuleEvaluator with template rules
        from rule_evaluator import RuleEvaluator
        evaluator = RuleEvaluator(validation_rules)
        validation_errors = evaluator.evaluate(spoke_path, content, frontmatter)

        # Convert ValidationError objects to string format
        for ve in validation_errors:
            errors.append(ve.format_error())

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    spoke_count = len([d for d in docs if extract_frontmatter(d) and extract_frontmatter(d).get('doc_type') == 'spoke'])
    print(f"Spoke validation passed: {spoke_count} documents")
    return 0


def validate_cross_cutting_index(args) -> int:
    """Validate cross-cutting index documents against template-driven rules.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Uses template-driven validation from doc/_meta/02-templates/cross-cutting-index.md
    """
    # Load validation rules from cross-cutting-index template
    doc_root = Path(__file__).parent.parent
    template_path = doc_root / "_meta" / "02-templates" / "cross-cutting-index.md"

    try:
        validation_rules = load_template_validation(template_path)
    except ValueError as e:
        print(f"[ERROR] Failed to load template validation rules: {e}", file=sys.stderr)
        return 1

    # Discover all cross-cutting index documents
    docs = discover_docs()
    errors = []

    for doc_path in docs:
        # Extract frontmatter
        frontmatter = extract_frontmatter(doc_path)

        if not frontmatter:
            # Skip documents without frontmatter
            continue

        # Check if this is a cross-cutting index document
        if frontmatter.get('doc_type') != 'index':
            continue

        # Further check if it has cross_cutting_concern field
        if 'cross_cutting_concern' not in frontmatter:
            continue

        content = doc_path.read_text(encoding='utf-8')

        # Use RuleEvaluator with template rules
        evaluator = RuleEvaluator(validation_rules)
        validation_errors = evaluator.evaluate(doc_path, content, frontmatter)

        # Convert ValidationError objects to string format
        for ve in validation_errors:
            errors.append(ve.format_error())

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    index_count = len([d for d in docs if extract_frontmatter(d) and extract_frontmatter(d).get('doc_type') == 'index' and 'cross_cutting_concern' in extract_frontmatter(d)])
    print(f"Cross-cutting index validation passed: {index_count} documents")
    return 0


def validate_redirect_stub(args) -> int:
    """Validate redirect stub documents against template-driven rules.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on errors

    Uses template-driven validation from doc/_meta/02-templates/redirect-stub.md
    """
    # Load validation rules from redirect-stub template
    doc_root = Path(__file__).parent.parent
    template_path = doc_root / "_meta" / "02-templates" / "redirect-stub.md"

    try:
        validation_rules = load_template_validation(template_path)
    except ValueError as e:
        print(f"[ERROR] Failed to load template validation rules: {e}", file=sys.stderr)
        return 1

    # Discover all redirect stub documents
    docs = discover_docs()
    errors = []

    for doc_path in docs:
        # Extract frontmatter
        frontmatter = extract_frontmatter(doc_path)

        if not frontmatter:
            # Skip documents without frontmatter
            continue

        # Check if this is a redirect stub document
        if frontmatter.get('doc_type') != 'redirect-stub':
            continue

        content = doc_path.read_text(encoding='utf-8')

        # Use RuleEvaluator with template rules
        evaluator = RuleEvaluator(validation_rules)
        validation_errors = evaluator.evaluate(doc_path, content, frontmatter)

        # Convert ValidationError objects to string format
        for ve in validation_errors:
            errors.append(ve.format_error())

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    stub_count = len([d for d in docs if extract_frontmatter(d) and extract_frontmatter(d).get('doc_type') == 'redirect-stub'])
    print(f"Redirect stub validation passed: {stub_count} documents")
    return 0


def check_complexity(args) -> int:
    """Check validation block complexity across all templates.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 on success, 1 on warnings (informational only)

    This command checks template validation blocks for complexity issues
    and emits warnings when thresholds are exceeded. This is an informational
    command that helps template authors identify overly complex validation rules.
    """
    doc_root = Path(__file__).parent.parent
    meta_templates_dir = doc_root / "_meta" / "02-templates"

    if not meta_templates_dir.exists():
        print(f"[ERROR] Templates directory not found: {meta_templates_dir}", file=sys.stderr)
        return 1

    # Find all template files
    templates = sorted(meta_templates_dir.glob("*.md"))

    if not templates:
        print("[WARN] No template files found")
        return 0

    total_warnings = 0
    templates_checked = 0

    for template_path in templates:
        try:
            validation_block = load_template_validation(template_path, use_cache=False)
            templates_checked += 1

            warnings = check_validation_complexity(validation_block, template_path)
            total_warnings += len(warnings)

        except ValueError as e:
            # Template may not have validation section - skip it
            continue

    if total_warnings > 0:
        print(f"\n[SUMMARY] Found {total_warnings} complexity warnings across {templates_checked} templates")
        print("[INFO] Consider simplifying validation rules or splitting templates")
    else:
        print(f"[SUCCESS] Complexity check passed: {templates_checked} templates validated")

    return 0


def check_validation_complexity(validation_block: Dict[str, Any], template_path: Path) -> List[str]:
    """Check validation block complexity and emit warnings.

    Args:
        validation_block: Validation rules from template frontmatter
        template_path: Path to the template file

    Returns:
        List of warning messages (empty if no warnings)
    """
    warnings = []

    # Count lines in validation YAML
    yaml_text = yaml.dump(validation_block)
    line_count = len(yaml_text.splitlines())
    if line_count > COMPLEXITY_LIMITS['max_frontmatter_lines']:
        warnings.append(f"Validation block has {line_count} lines (limit: {COMPLEXITY_LIMITS['max_frontmatter_lines']})")

    # Count conditions
    conditions = validation_block.get('conditions', {})
    if len(conditions) > COMPLEXITY_LIMITS['max_conditions']:
        warnings.append(f"{len(conditions)} conditions (limit: {COMPLEXITY_LIMITS['max_conditions']})")

    # Count required_sections
    sections = validation_block.get('required_sections', [])
    if len(sections) > COMPLEXITY_LIMITS['max_required_sections']:
        warnings.append(f"{len(sections)} required sections (limit: {COMPLEXITY_LIMITS['max_required_sections']})")

    # Count forbidden patterns
    forbidden = validation_block.get('forbidden', [])
    if len(forbidden) > COMPLEXITY_LIMITS['max_forbidden_patterns']:
        warnings.append(f"{len(forbidden)} forbidden patterns (limit: {COMPLEXITY_LIMITS['max_forbidden_patterns']})")

    if warnings:
        print(f"[WARN] {template_path}: Validation complexity high:")
        for warning in warnings:
            print(f"  - {warning}")

    return warnings


def validate_all(args) -> int:
    """Run all validators in sequence.

    Args:
        args: Command-line arguments from argparse

    Returns:
        Exit code: 0 if all validators pass, first non-zero exit code otherwise

    Execution order:
        1. Frontmatter validation
        2. Hub-Spoke validation
        3. Meta-Directories validation
        4. CLAUDE.md validation
        5. Hub validation
        6. Spoke validation
        7. Cross-cutting index validation
        8. Redirect stub validation
        9. Indexes validation (placeholder)
        10. Links validation (placeholder)
        11. Hub freshness check (informational only)
    """
    validators = [
        ('Frontmatter', validate_frontmatter),
        ('Hub-Spoke', validate_hub_spoke),
        ('Meta-Directories', validate_meta_directories),
        ('CLAUDE.md', validate_claude_md),
        ('Hub', validate_hub),
        ('Spoke', validate_spoke),
        ('Cross-cutting Index', validate_cross_cutting_index),
        ('Redirect Stub', validate_redirect_stub),
        ('Indexes', validate_indexes),
        ('Links', validate_links),
    ]

    results = {}
    print("=" * 60)

    for name, validator in validators:
        print(f"\nRunning {name} validation...")
        print("-" * 60)
        results[name] = validator(args)

    print("\n" + "=" * 60)
    print("VALIDATION SUMMARY")
    print("=" * 60)

    for name, exit_code in results.items():
        status = "PASSED" if exit_code == 0 else "FAILED"
        print(f"{name}: {status}")

    # Hub freshness (informational only)
    print("\n" + "=" * 60)
    print("Hub Freshness Check (informational)")
    print("=" * 60)
    check_hub_freshness(args)

    # Return first non-zero exit code
    for exit_code in results.values():
        if exit_code != 0:
            return exit_code
    return 0


def main() -> int:
    """Main entry point for the validation tool."""
    parser = argparse.ArgumentParser(
        description="Validate Trapperkeeper documentation",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s validate-frontmatter
  %(prog)s validate-hub-spoke
  %(prog)s validate-all
        """
    )

    subparsers = parser.add_subparsers(
        dest='command',
        required=True,
        help='Validation command to run'
    )

    # validate-frontmatter subcommand
    parser_frontmatter = subparsers.add_parser(
        'validate-frontmatter',
        help='Validate frontmatter against schema requirements'
    )

    # validate-hub-spoke subcommand
    parser_hub_spoke = subparsers.add_parser(
        'validate-hub-spoke',
        help='Validate hub-spoke documentation structure'
    )

    # validate-meta-directories subcommand
    parser_meta_dirs = subparsers.add_parser(
        'validate-meta-directories',
        help='Validate _meta/ subdirectories have README.md hubs'
    )

    # validate-claude-md subcommand
    parser_claude_md = subparsers.add_parser(
        'validate-claude-md',
        help='Validate CLAUDE.md navigation files'
    )
    parser_claude_md.add_argument(
        '--show-effective-rules',
        action='store_true',
        help='Show resolved validation rules and exit (debug mode)'
    )

    # validate-hub subcommand
    parser_hub = subparsers.add_parser(
        'validate-hub',
        help='Validate hub documents against template rules'
    )
    parser_hub.add_argument(
        '--show-effective-rules',
        action='store_true',
        help='Show resolved validation rules and exit (debug mode)'
    )

    # validate-spoke subcommand
    parser_spoke = subparsers.add_parser(
        'validate-spoke',
        help='Validate spoke documents against template rules'
    )
    parser_spoke.add_argument(
        '--show-effective-rules',
        action='store_true',
        help='Show resolved validation rules and exit (debug mode)'
    )

    # validate-cross-cutting-index subcommand
    parser_cross_cutting_index = subparsers.add_parser(
        'validate-cross-cutting-index',
        help='Validate cross-cutting index documents against template rules'
    )

    # validate-redirect-stub subcommand
    parser_redirect_stub = subparsers.add_parser(
        'validate-redirect-stub',
        help='Validate redirect stub documents against template rules'
    )

    # validate-indexes subcommand
    parser_indexes = subparsers.add_parser(
        'validate-indexes',
        help='Validate index file completeness and structure'
    )

    # validate-links subcommand
    parser_links = subparsers.add_parser(
        'validate-links',
        help='Validate internal documentation links'
    )

    # check-hub-freshness subcommand
    parser_freshness = subparsers.add_parser(
        'check-hub-freshness',
        help='Check hub document freshness against spokes'
    )

    # validate-all subcommand
    parser_all = subparsers.add_parser(
        'validate-all',
        help='Run all validation checks'
    )

    # check-complexity subcommand
    parser_complexity = subparsers.add_parser(
        'check-complexity',
        help='Check validation block complexity across templates'
    )

    # Parse arguments
    args = parser.parse_args()

    # Dispatch to handler functions
    handlers: Dict[str, callable] = {
        'validate-frontmatter': validate_frontmatter,
        'validate-hub-spoke': validate_hub_spoke,
        'validate-meta-directories': validate_meta_directories,
        'validate-claude-md': validate_claude_md,
        'validate-hub': validate_hub,
        'validate-spoke': validate_spoke,
        'validate-cross-cutting-index': validate_cross_cutting_index,
        'validate-redirect-stub': validate_redirect_stub,
        'validate-indexes': validate_indexes,
        'validate-links': validate_links,
        'check-hub-freshness': check_hub_freshness,
        'check-complexity': check_complexity,
        'validate-all': validate_all,
    }

    handler = handlers.get(args.command)
    if handler is None:
        print(f"Error: Unknown command '{args.command}'", file=sys.stderr)
        return 1

    return handler(args)


if __name__ == "__main__":
    sys.exit(main())
