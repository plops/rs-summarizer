---
inclusion: manual
---

# VTT Parsing Algorithm

## Overview

The VTT parser converts WebVTT subtitle files into plain text with second-granularity timestamps. It matches the Python `webvtt` library behavior byte-for-byte.

## Algorithm (ported from Python `s02_parse_vtt_file.py`)

```python
# Python original:
old_text = ["__bla__"]
old_time = "00:00:00"
out = [dict(text="")]
for c in webvtt.read(filename):
    if out[-1]["text"] != old_text[-1]:
        out.append(dict(text=old_text[-1], time=old_time))
    old_text = c.text.split("\n")
    old_time = c.start
# Skip first two entries, format as "HH:MM:SS text\n"
```

## Key Behaviors

1. **Multi-line cues**: Only the last line of each cue's payload is used (`c.text.split("\n")[-1]`)
2. **Consecutive deduplication**: A new entry is only appended when the current last entry's text differs from the previous cue's last line
3. **Initialization skip**: The first two entries in the output list are initialization artifacts and are skipped
4. **Timestamp truncation**: Timestamps are truncated to second granularity by splitting on "." and taking the first part

## VTT Tag Stripping

Auto-generated YouTube captions contain inline VTT tags like `<00:00:01.350>`, `<c>`, `</c>`. These are stripped before processing:

```rust
fn strip_vtt_tags(text: &str) -> String {
    // Remove anything between < and >
}
```

## Custom Parser (not using `vtt` crate)

Although the `vtt` crate is in Cargo.toml, a custom parser is used because the `vtt` crate handles whitespace-only lines differently from Python's `webvtt` library. The custom parser:
- Splits content into blocks using trim-empty lines as separators
- Finds timing lines (containing `-->`)
- Skips blocks without payload lines after the timing line
- Extracts payload as lines after the timing line, joined with newlines

## Output Format

Each line: `HH:MM:SS caption_text\n`

No milliseconds, no VTT tags, no consecutive duplicate lines.

## Ground Truth Test

The test fixture `tests/fixtures/cW3tzRzTHKI.en.vtt` produces a known 37-line output that matches the Python implementation byte-for-byte.

## Relevant Files

- `src/utils/vtt_parser.rs` — Parser implementation
- `tests/fixtures/cW3tzRzTHKI.en.vtt` — Test fixture
- `source04/tsum/s02_parse_vtt_file.py` — Python original
- `source04/tsum/t02_parse_vtt_file.py` — Python ground truth test
