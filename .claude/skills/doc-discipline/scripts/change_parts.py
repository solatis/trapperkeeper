#!/usr/bin/env python3
"""Split the changed lines of a git range into parts that one cold reader can read whole.

The script reads the diff of a range BASE...REV and lists, per part, the line
spans of the files at REV that the part covers. It joins spans that lie close
together, fills parts in the order of the diff up to a target size, and cuts a
longer span at a blank line where one is near. It knows no language grammar, so
a cut can fall inside a function when the file has no blank line near the size.
It does not read deleted files, binary files or names that git quotes.
"""

import argparse
import re
import subprocess
import sys
from collections import namedtuple

# A span of lines start..end, both included, of a file at REV.
Span = namedtuple("Span", "path start end")

# Measured on one file of 1070 lines: readers that got about 350 lines each
# stopped at 15 of 16 known hard places, and a reader that got the whole file
# stopped at 9 to 11 of them.
DEFAULT_SIZE = 300
# Spans of one file with fewer unchanged lines than this between them are joined.
# The value is not measured.
JOIN_GAP = 5
# How far, in lines either way, a cut looks for a blank line. The value is not
# measured.
CUT_WINDOW = 40
# Directories whose files a reader does not review: version control data, build
# output and vendored code.
SKIP_DIRS = {".git", "target", "node_modules", "vendor", "thirdparty",
             "third_party", "build", "dist"}

# The new side of a hunk header, such as "@@ -10,2 +12,3 @@ fn f()" with groups 12 and 3.
HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")


def length(s):
    return s.end - s.start + 1


def parse_range(text):
    """Return (base, rev) of a range "<base>...<rev>", or None when it has another shape."""
    base, sep, rev = text.partition("...")
    # A side that starts with "-" would reach git as an option. A side next to
    # a fourth "." means the range had more than three dots.
    if not sep or not base or not rev or "..." in rev:
        return None
    if base.startswith("-") or rev.startswith(("-", ".")) or base.endswith("."):
        return None
    return base, rev


def new_side(header):
    """Return (start, count) of the new side of a hunk header, or None for another line."""
    m = HUNK.match(header)
    if m is None:
        return None
    # git leaves out the count when it is 1.
    return int(m.group(1)), int(m.group(2) or "1")


def is_skipped(path):
    """Return True for a lock file or a file under a vendored, built or git directory."""
    return path.endswith(".lock") or bool(SKIP_DIRS.intersection(path.split("/")[:-1]))


def diff_spans(text):
    """Return (spans, changed lines) of a diff made with -U0, in the order of the diff.

    A hunk that adds no line on the new side gives no span.
    """
    spans, changed, path, in_header = [], 0, None, False
    for line in text.split("\n"):
        # A body line starts with "+", "-", " " or "\", so these two prefixes
        # cannot come from file content.
        if line.startswith("diff --git "):
            path, in_header = None, True
        elif in_header and line.startswith("+++ b/"):
            # git ends the name with a tab when it holds a space.
            path = line[len("+++ b/"):].rstrip("\t")
        elif line.startswith("@@"):
            in_header = False
            hunk = new_side(line)
            if path is None or hunk is None or hunk[1] == 0 or is_skipped(path):
                continue
            spans.append(Span(path, hunk[0], hunk[0] + hunk[1] - 1))
            changed += hunk[1]
    return spans, changed


def join_close(spans):
    """Return the spans with each run of close spans of one file joined into one span."""
    joined = []
    for s in spans:
        if joined and joined[-1].path == s.path and s.start - joined[-1].end - 1 < JOIN_GAP:
            joined[-1] = joined[-1]._replace(end=max(joined[-1].end, s.end))
        else:
            joined.append(s)
    return joined


def cut_line(lines, target, lo, hi):
    """Return the blank line in lo..hi nearest to target, or target when none is near."""
    window = range(max(lo, target - CUT_WINDOW), min(hi, target + CUT_WINDOW) + 1)
    blanks = [n for n in window if n <= len(lines) and not lines[n - 1].strip()]
    # Of two blank lines at the same distance, the earlier one keeps the piece within the size.
    return min(blanks, key=lambda n: (abs(n - target), n)) if blanks else target


def cut_span(s, lines, size):
    """Return the span cut into pieces of about `size` lines, each ending at a blank line
    of `lines` where one is near."""
    pieces, start = [], s.start
    while s.end - start + 1 > size:
        # The upper bound s.end - 1 leaves at least one line for the next piece.
        end = cut_line(lines, start + size - 1, start, s.end - 1)
        pieces.append(Span(s.path, start, end))
        start = end + 1
    return pieces + [Span(s.path, start, s.end)]


def fill_parts(spans, size):
    """Return the spans grouped into parts in order, each part at most `size` lines unless
    it holds a single span."""
    parts = []
    for s in spans:
        if parts and sum(map(length, parts[-1])) + length(s) <= size:
            parts[-1].append(s)
        else:
            parts.append([s])
    return parts


def git(args):
    """Return the stdout of git run with `args`, raising CalledProcessError when git fails."""
    return subprocess.run(["git"] + args, capture_output=True, check=True).stdout


def read_diff(base, rev):
    """Return the text of the diff of the range with no context lines."""
    # The prefixes and --no-ext-diff override a user configuration that would
    # change the header lines that diff_spans reads.
    return git(["diff", "-U0", "--no-color", "--no-ext-diff", "--src-prefix=a/",
                "--dst-prefix=b/", "-M", "--diff-filter=AMR", base + "..." + rev, "--"]
               ).decode("utf-8", "surrogateescape")


def file_lines(rev, path):
    return git(["show", rev + ":" + path]).decode("utf-8", "replace").split("\n")


def change_parts(base, rev, size):
    """Return (parts, changed lines, files) of the range. Raises as `git` does."""
    # The parts come from the diff in four steps.
    # 1. Read the spans of the new side of each hunk from the diff.
    # 2. Join close spans, so that a cut sees the whole region that a reader reads.
    # 3. Cut each span longer than the size, reading the file at rev only for those spans.
    # 4. Fill the parts in the order of the diff.

    # 1. read the spans
    spans, changed = diff_spans(read_diff(base, rev))
    files = len({s.path for s in spans})
    # 2. join close spans
    joined = join_close(spans)
    # 3. cut long spans
    pieces = [p for s in joined
              for p in (cut_span(s, file_lines(rev, s.path), size) if length(s) > size else [s])]
    # 4. fill the parts
    return fill_parts(pieces, size), changed, files


def format_span(s):
    return "%s:%d-%d" % s


def list_lines(parts, changed, files):
    """Return the output lines of the list form.

    A part counts the lines of its spans, which include the unchanged lines that joining took
    in. The last line counts the changed lines alone.
    """
    n = len(parts)
    rows = ["part %d/%d: %d lines: %s" % (k, n, sum(map(length, p)),
                                          ", ".join(map(format_span, p)))
            for k, p in enumerate(parts, 1)]
    return rows + ["change_parts: %d parts, %d changed lines in %d files" % (n, changed, files)]


class Parser(argparse.ArgumentParser):
    def error(self, message):
        # A usage error is one line on stderr, as every other error is.
        print("change_parts: " + message, file=sys.stderr)
        sys.exit(2)


def parse_args(argv):
    """Return the parsed arguments. Exits with status 2 on a usage error."""
    p = Parser(prog="change_parts.py", allow_abbrev=False,
               description="List the parts of a change, or print the spans of part K.")
    p.add_argument("range", metavar="BASE...REV")
    p.add_argument("part", nargs="?", type=int, metavar="K")
    p.add_argument("--size", type=int, default=DEFAULT_SIZE, metavar="N",
                   help="target number of changed lines per part (default %d)" % DEFAULT_SIZE)
    args = p.parse_args(argv)
    args.parsed = parse_range(args.range)
    if args.parsed is None:
        p.error("malformed range %r, expected BASE...REV" % args.range)
    if args.size < 1:
        p.error("--size must be at least 1")
    return args


def main(argv=None):
    """Print the parts of the range or the spans of one part and return the exit status."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        parts, changed, files = change_parts(args.parsed[0], args.parsed[1], args.size)
    except subprocess.CalledProcessError as e:
        reason = e.stderr.decode("utf-8", "replace").strip().splitlines()
        print("change_parts: git %s failed: %s" % (
            e.cmd[1], reason[0] if reason else "exit status %d" % e.returncode), file=sys.stderr)
        return 2
    except OSError as e:
        print("change_parts: cannot run git: %s" % e, file=sys.stderr)
        return 2
    if args.part is None:
        print("\n".join(list_lines(parts, changed, files)))
    elif 1 <= args.part <= len(parts):
        print("\n".join(map(format_span, parts[args.part - 1])))
    else:
        print("change_parts: no part %d, the change has %d parts" % (args.part, len(parts)),
              file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
