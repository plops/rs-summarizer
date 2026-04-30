/// Strips WebVTT cue tags (e.g. `<00:00:01.350>`, `<c>`, `</c>`) from text.
fn strip_vtt_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

/// Represents a parsed cue from a VTT file.
struct Cue {
    start_timestamp: String,
    payload: String,
}

/// Parses VTT content into cues, matching Python webvtt behavior:
/// - Lines where `trim()` is empty are block separators
/// - Blocks without a `-->` timing line are skipped
/// - Payload is the lines after the timing line, joined with newlines
fn parse_cues(vtt_content: &str) -> Vec<Cue> {
    let lines: Vec<&str> = vtt_content.lines().collect();

    // Split into blocks using trim-empty lines as separators
    // (matching Python's `line.strip()` check)
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut current_block: Vec<&str> = Vec::new();

    for line in &lines {
        if line.trim().is_empty() {
            if !current_block.is_empty() {
                blocks.push(current_block.clone());
                current_block.clear();
            }
        } else {
            current_block.push(line);
        }
    }
    if !current_block.is_empty() {
        blocks.push(current_block);
    }

    // Process blocks into cues
    let mut cues = Vec::new();
    for block in &blocks {
        // Find the timing line (contains "-->")
        let timing_idx = block.iter().position(|line| line.contains("-->"));
        let timing_idx = match timing_idx {
            Some(idx) => idx,
            None => continue, // Skip blocks without timing
        };

        // Skip blocks that don't have payload lines after the timing line
        // (matching Python webvtt's is_valid check which requires len >= 2)
        if timing_idx + 1 >= block.len() {
            continue;
        }

        let timing_line = block[timing_idx];
        let start_str = timing_line.split("-->").next().unwrap().trim();
        let start_timestamp = parse_timestamp_to_hms(start_str);

        // Payload is everything after the timing line
        let payload_lines: Vec<&str> = block[timing_idx + 1..].to_vec();
        let payload = payload_lines.join("\n");

        cues.push(Cue {
            start_timestamp,
            payload,
        });
    }

    cues
}

/// Parses a VTT timestamp string (HH:MM:SS.mmm or MM:SS.mmm) and returns HH:MM:SS.
fn parse_timestamp_to_hms(ts_str: &str) -> String {
    let parts: Vec<&str> = ts_str.split(':').collect();
    let (hours, minutes, sec_part) = if parts.len() == 3 {
        (
            parts[0].parse::<u64>().unwrap_or(0),
            parts[1].parse::<u64>().unwrap_or(0),
            parts[2],
        )
    } else if parts.len() == 2 {
        (0u64, parts[0].parse::<u64>().unwrap_or(0), parts[1])
    } else {
        return "00:00:00".to_string();
    };

    // Strip milliseconds (everything after '.')
    let seconds: u64 = sec_part
        .split('.')
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Parses WebVTT content and returns a deduplicated transcript string
/// with second-granularity timestamps.
///
/// This is a port of the Python `parse_vtt_file` function from `s02_parse_vtt_file.py`.
/// Algorithm:
/// 1. For each cue, take the last line of the payload (after stripping VTT tags).
/// 2. Deduplicate consecutive entries with the same text.
/// 3. Skip the first two initialization entries.
/// 4. Format each entry as "HH:MM:SS text\n".
pub fn parse_vtt(vtt_content: &str) -> String {
    let cues = parse_cues(vtt_content);

    // Mirrors the Python algorithm:
    // old_text = ["__bla__"]
    // old_time = "00:00:00"
    // out = [dict(text="")]
    let mut old_text: Vec<String> = vec!["__bla__".to_string()];
    let mut old_time = "00:00:00".to_string();

    // out stores (text, time) pairs; first entry is initialization artifact
    let mut out: Vec<(String, String)> = vec![("".to_string(), String::new())];

    for cue in &cues {
        // Check if current last entry's text differs from old_text's last element
        let last_out_text = &out.last().unwrap().0;
        let old_text_last = old_text.last().unwrap();
        if last_out_text != old_text_last {
            out.push((old_text_last.clone(), old_time.clone()));
        }

        // Process payload: strip VTT tags, split by newline
        let clean_payload = strip_vtt_tags(&cue.payload);
        let lines: Vec<&str> = clean_payload.split('\n').collect();
        old_text = lines.iter().map(|s| s.to_string()).collect();
        old_time = cue.start_timestamp.clone();
    }

    // Build output string, skipping first two entries (initialization artifacts)
    let mut ostr = String::new();
    for entry in out.iter().skip(2) {
        let tstamp = &entry.1;
        let caption = &entry.0;
        ostr.push_str(&format!("{} {}\n", tstamp, caption));
    }

    ostr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vtt_fixture_matches_python_output() {
        let vtt_content =
            std::fs::read_to_string("tests/fixtures/cW3tzRzTHKI.en.vtt").unwrap();
        let result = parse_vtt(&vtt_content);

        let expected = r#"00:00:00 [Music]
00:00:00 welcome to BASF we create chemistry so
00:00:05 it makes sense that we should
00:00:06 familiarize you with the basic chemistry
00:00:08 taught in our poly urethanes Academy
00:00:11 we're going to simplify things a bit in
00:00:13 this video and at the same time cover a
00:00:16 lot of topics so let's get started first
00:00:18 let's introduce you to two of our
00:00:21 leading characters by societies or ISO
00:00:25 and resin let's talk about ISO first
00:00:29 when we make ISO we do so in very large
00:00:32 quantities for our purposes today there
00:00:35 are only a few types of eye soaps pure
00:00:38 MD eyes and TV eyes that's their
00:00:40 nicknames form long and squiggly
00:00:43 chemical structures that's because they
00:00:45 have fewer places to connect to they are
00:00:47 generally used to make flexible products
00:00:50 like seat cushions mattresses and
00:00:52 sealants polymeric MD eyes have many
00:00:55 more places to plug into which creates
00:00:58 more of the structure they are generally
00:01:00 used to make you guessed it rigid
00:01:03 products like picnic coolers foam
00:01:05 insulation and wood boards now when our
00:01:09 customers make a resin they create a
00:01:11 custom formula of additives that include
00:01:14 polygons also supplied by BASF which are
00:01:18 the backbone of the mix polyols make the
00:01:21 majority of the mix kind of like flour
00:01:24 is to a cake batter
00:01:25 polyols determine the physical
00:01:27 properties of the product like how soft
00:01:30 or hard the product is
00:01:32 catalysts they control the speed of the
00:01:35 chemical reaction and how quickly it
00:01:37 cures surfactants determine the cell
00:01:40 structure and influence the flow
00:01:42 pigments determine the color flame
00:01:45 retardants make it savory adhesion
00:01:48 promoters make it stickier and finally
00:01:50 blowing agents help determine the
00:01:53 density and foaming action
00:01:55 at BASF we're proud to supply raw
00:01:58 materials that help our customers
00:02:00 innovate and succeed on ISOs and polyols
00:02:04 combined to make custom formulas for our
00:02:06 customers custom formulas that produce
00:02:09 unique products that are flexible rigid
00:02:33 just the way end-users like them so
00:02:33 there you have it the basics of
00:02:36 polyurethanes from BASF we create
"#;

        assert_eq!(result, expected);
    }

    #[test]
    fn test_strip_vtt_tags() {
        assert_eq!(
            strip_vtt_tags("welcome<00:00:01.350><c> to</c><00:00:01.530><c> BASF</c>"),
            "welcome to BASF"
        );
        assert_eq!(strip_vtt_tags("plain text"), "plain text");
        assert_eq!(strip_vtt_tags(""), "");
    }

    #[test]
    fn test_parse_timestamp_to_hms() {
        assert_eq!(parse_timestamp_to_hms("00:00:00.040"), "00:00:00");
        assert_eq!(parse_timestamp_to_hms("00:00:05.090"), "00:00:05");
        assert_eq!(parse_timestamp_to_hms("01:01:01.000"), "01:01:01");
        assert_eq!(parse_timestamp_to_hms("23:45.678"), "00:23:45");
    }

    #[test]
    fn test_parse_vtt_empty_content() {
        let content = "WEBVTT\n\n";
        let result = parse_vtt(content);
        assert_eq!(result, "");
    }
}
