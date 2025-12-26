#!/usr/bin/env python3
"""
Test suite for markdown_parser module.

Tests AST parsing, section extraction, paragraph counting, sentence counting,
and list item extraction with 20+ test cases covering edge cases.
"""

import pytest
from pathlib import Path
from markdown_parser import (
    MarkdownParser,
    SectionInfo,
    parse_markdown,
    extract_sections,
    get_section,
    count_paragraphs,
    count_sentences,
    extract_list_items,
    section_matches_pattern,
    section_present,
)


# ============================================================================
# AST Parser Tests
# ============================================================================

def test_parse_markdown_simple():
    """Test parsing simple markdown document."""
    text = "# Title\n\nParagraph text."
    tokens = parse_markdown(text)
    assert len(tokens) > 0
    assert tokens[0].type == "heading_open"


def test_parse_markdown_empty():
    """Test parsing empty document."""
    text = ""
    tokens = parse_markdown(text)
    assert len(tokens) == 0


def test_parse_markdown_caching():
    """Test that AST caching works."""
    text = "# Title\n\nParagraph."
    parser = MarkdownParser()

    # Parse with cache key
    tokens1 = parser.parse_markdown(text, cache_key="test")
    tokens2 = parser.parse_markdown(text, cache_key="test")

    # Should return same cached object
    assert tokens1 is tokens2


def test_parse_markdown_cache_clear():
    """Test clearing AST cache."""
    text = "# Title\n\nParagraph."
    parser = MarkdownParser()

    tokens1 = parser.parse_markdown(text, cache_key="test")
    parser.clear_cache()
    tokens2 = parser.parse_markdown(text, cache_key="test")

    # Should be different objects after cache clear
    assert tokens1 is not tokens2


# ============================================================================
# Section Indexer Tests
# ============================================================================

def test_extract_sections_basic():
    """Test extracting basic H2 sections."""
    text = """# Document Title

## Section One

This is content for section one.

## Section Two

This is content for section two.
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    assert "Section One" in sections
    assert "Section Two" in sections
    assert len(sections) == 2


def test_extract_sections_nested():
    """Test extracting nested H2 > H3 > H4 sections."""
    text = """# Document Title

## Section One

Content for section one.

### Subsection 1.1

Content for subsection.

#### Subsubsection 1.1.1

Deep content.

## Section Two

Content for section two.
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    # Should extract all H2+ headings
    assert "Section One" in sections
    assert "Subsection 1.1" in sections
    assert "Subsubsection 1.1.1" in sections
    assert "Section Two" in sections
    assert len(sections) == 4


def test_extract_sections_h1_excluded():
    """Test that H1 headings are excluded by default."""
    text = """# Main Title

## Section One

Content.

# Another H1

## Section Two

More content.
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    # Should only extract H2 sections
    assert "Main Title" not in sections
    assert "Another H1" not in sections
    assert "Section One" in sections
    assert "Section Two" in sections


def test_extract_sections_custom_min_level():
    """Test extracting sections with custom minimum level."""
    text = """# Document

## H2 Section

### H3 Section

#### H4 Section
"""
    tokens = parse_markdown(text)

    # Extract H3+ only
    sections = extract_sections(tokens, min_level=3)

    assert "H2 Section" not in sections
    assert "H3 Section" in sections
    assert "H4 Section" in sections


def test_extract_sections_empty_sections():
    """Test extracting empty sections with no content."""
    text = """## Empty Section One

## Empty Section Two

## Section With Content

Some content here.
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    assert "Empty Section One" in sections
    assert "Empty Section Two" in sections
    assert "Section With Content" in sections
    assert sections["Empty Section One"].content == ""
    assert sections["Section With Content"].content.strip() == "Some content here."


def test_extract_sections_special_characters():
    """Test extracting sections with special characters in headings."""
    text = """## Section: With Colon

## Section (With Parens)

## Section - With Dash

## Section & Ampersand
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    assert "Section: With Colon" in sections
    assert "Section (With Parens)" in sections
    assert "Section - With Dash" in sections
    assert "Section & Ampersand" in sections


def test_extract_sections_line_numbers():
    """Test that line numbers are correctly tracked."""
    text = """## Section One
Line 1
Line 2

## Section Two
Line 3
Line 4
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    # Line numbers should be tracked (0-based)
    assert sections["Section One"].line_start == 0
    assert sections["Section Two"].line_start >= 3


def test_get_section_exists():
    """Test retrieving section by name when it exists."""
    text = """## Files

Some file content.

## Summary

Summary content.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Files")

    assert section is not None
    assert section.name == "Files"
    assert "file content" in section.content


def test_get_section_not_exists():
    """Test retrieving section by name when it doesn't exist."""
    text = """## Files

Content.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "NonExistent")

    assert section is None


def test_section_info_attributes():
    """Test that SectionInfo contains all expected attributes."""
    text = """## Test Section

Paragraph content.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Test Section")

    assert hasattr(section, 'name')
    assert hasattr(section, 'level')
    assert hasattr(section, 'content')
    assert hasattr(section, 'line_start')
    assert hasattr(section, 'line_end')
    assert hasattr(section, 'tokens')
    assert section.level == 2


# ============================================================================
# Paragraph Counting Tests
# ============================================================================

def test_count_paragraphs_single():
    """Test counting single paragraph."""
    text = """## Section

This is one paragraph.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert count_paragraphs(section) == 1


def test_count_paragraphs_multiple():
    """Test counting multiple paragraphs."""
    text = """## Section

First paragraph.

Second paragraph.

Third paragraph.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert count_paragraphs(section) == 3


def test_count_paragraphs_empty_section():
    """Test counting paragraphs in empty section."""
    text = """## Empty Section

## Next Section
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Empty Section")

    assert count_paragraphs(section) == 0


def test_count_paragraphs_with_lists():
    """Test counting paragraphs with lists (lists are not paragraphs)."""
    text = """## Section

Paragraph one.

- List item 1
- List item 2

Paragraph two.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    # Should count only the paragraphs, not list items
    assert count_paragraphs(section) == 2


def test_count_paragraphs_with_code_blocks():
    """Test counting paragraphs with code blocks."""
    text = """## Section

Paragraph one.

```
code block
```

Paragraph two.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    # Code blocks are not paragraphs
    assert count_paragraphs(section) == 2


# ============================================================================
# Sentence Counting Tests
# ============================================================================

def test_count_sentences_single():
    """Test counting single sentence."""
    text = """## Section

This is one sentence.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert count_sentences(section) == 1


def test_count_sentences_multiple():
    """Test counting multiple sentences."""
    text = """## Section

First sentence. Second sentence! Third sentence?
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert count_sentences(section) == 3


def test_count_sentences_abbreviations():
    """Test counting sentences with abbreviations."""
    text = """## Section

Dr. Smith works at the clinic. Mr. Jones is his colleague.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    # Should not split on abbreviations
    assert count_sentences(section) == 2


def test_count_sentences_decimal_numbers():
    """Test counting sentences with decimal numbers."""
    text = """## Section

The value is 3.14 approximately. Another value is 2.71 roughly.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    # Should not split on decimal points
    assert count_sentences(section) == 2


def test_count_sentences_mixed_punctuation():
    """Test counting sentences with mixed punctuation."""
    text = """## Section

Question? Answer! Statement.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert count_sentences(section) == 3


def test_count_sentences_empty_section():
    """Test counting sentences in empty section."""
    text = """## Empty

## Next
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Empty")

    assert count_sentences(section) == 0


def test_count_sentences_multiline():
    """Test counting sentences across multiple lines."""
    text = """## Section

This is sentence one.
This is sentence two.

This is sentence three.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert count_sentences(section) == 3


# ============================================================================
# List Item Extraction Tests
# ============================================================================

def test_extract_list_items_bulleted():
    """Test extracting bulleted list items."""
    text = """## Section

- Item 1
- Item 2
- Item 3
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")
    items = extract_list_items(section)

    assert len(items) == 3
    assert "Item 1" in items
    assert "Item 2" in items
    assert "Item 3" in items


def test_extract_list_items_numbered():
    """Test extracting numbered list items."""
    text = """## Section

1. First item
2. Second item
3. Third item
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")
    items = extract_list_items(section)

    assert len(items) == 3
    assert "First item" in items
    assert "Second item" in items
    assert "Third item" in items


def test_extract_list_items_mixed():
    """Test extracting mixed bulleted and numbered lists."""
    text = """## Section

- Bulleted item

1. Numbered item
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")
    items = extract_list_items(section)

    assert len(items) == 2
    assert "Bulleted item" in items
    assert "Numbered item" in items


def test_extract_list_items_empty():
    """Test extracting list items when no lists exist."""
    text = """## Section

Just a paragraph, no lists.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")
    items = extract_list_items(section)

    assert len(items) == 0


def test_extract_list_items_nested():
    """Test extracting nested list items."""
    text = """## Section

- Parent item
  - Child item 1
  - Child item 2
- Another parent
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")
    items = extract_list_items(section)

    # Should extract all items including nested ones
    assert len(items) >= 2


# ============================================================================
# Pattern Matching Tests
# ============================================================================

def test_section_matches_pattern_match():
    """Test pattern matching when pattern matches."""
    text = """## Section

This section contains foo and bar together.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert section_matches_pattern(section, r"foo.*bar") is True


def test_section_matches_pattern_no_match():
    """Test pattern matching when pattern doesn't match."""
    text = """## Section

This section contains only foo.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert section_matches_pattern(section, r"baz") is False


def test_section_matches_pattern_case_sensitive():
    """Test pattern matching is case sensitive by default."""
    text = """## Section

This section contains FOO.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert section_matches_pattern(section, r"foo") is False
    assert section_matches_pattern(section, r"FOO") is True


# ============================================================================
# Section Present Tests
# ============================================================================

def test_section_present_exists():
    """Test section_present returns True when section exists."""
    text = """## Files

Content.

## Summary

More content.
"""
    tokens = parse_markdown(text)

    assert section_present(tokens, "Files") is True
    assert section_present(tokens, "Summary") is True


def test_section_present_not_exists():
    """Test section_present returns False when section doesn't exist."""
    text = """## Files

Content.
"""
    tokens = parse_markdown(text)

    assert section_present(tokens, "Missing") is False
    assert section_present(tokens, "NonExistent") is False


def test_section_present_case_sensitive():
    """Test section_present is case sensitive."""
    text = """## Files

Content.
"""
    tokens = parse_markdown(text)

    assert section_present(tokens, "Files") is True
    assert section_present(tokens, "files") is False
    assert section_present(tokens, "FILES") is False


# ============================================================================
# Performance Tests
# ============================================================================

def test_parse_large_document_performance():
    """Test parsing 1000-line document completes in <100ms."""
    import time

    # Generate 1000-line document
    lines = []
    for i in range(50):
        lines.append(f"## Section {i}\n")
        lines.extend([f"Paragraph {j}.\n\n" for j in range(19)])

    text = "".join(lines)
    assert len(text.split('\n')) >= 1000

    # Measure parsing time
    start = time.time()
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)
    end = time.time()

    duration_ms = (end - start) * 1000

    # Should complete in <100ms
    assert duration_ms < 100, f"Parsing took {duration_ms:.2f}ms, expected <100ms"
    assert len(sections) == 50


# ============================================================================
# Edge Cases
# ============================================================================

def test_section_with_inline_code():
    """Test section containing inline code."""
    text = """## Section

This has `inline code` in it.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert section is not None
    assert "inline code" in section.content


def test_section_with_links():
    """Test section containing markdown links."""
    text = """## Section

This has a [link](http://example.com) in it.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert section is not None
    assert count_paragraphs(section) == 1


def test_section_with_blockquotes():
    """Test section containing blockquotes."""
    text = """## Section

> This is a blockquote.
> It spans multiple lines.

Regular paragraph.
"""
    tokens = parse_markdown(text)
    section = get_section(tokens, "Section")

    assert section is not None
    # Blockquote + paragraph
    assert count_paragraphs(section) >= 1


def test_multiple_sections_same_level():
    """Test multiple consecutive sections at same level."""
    text = """## Section 1

Content 1.

## Section 2

Content 2.

## Section 3

Content 3.
"""
    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    assert len(sections) == 3
    assert all(s.level == 2 for s in sections.values())


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
