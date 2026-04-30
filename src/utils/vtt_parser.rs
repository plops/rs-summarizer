use regex::Regex;

/// Parse a VTT subtitle file content into plain text with second-granularity timestamps.
/// Performs deduplication of repeated caption lines (common in auto-generated subs).
///
/// This is a direct port of the Python `parse_vtt_file` function from `s02_parse_vtt_file.py`.
pub fn parse_vtt(vtt_content: &str) -> String {
    let timestamp_re = Regex::new(r"^(\d{2}:\d{2}:\d{2}\.\d+)\s+-->").unwrap();
    // Regex to strip VTT inline tags like <00:00:01.350><c> word</c>
    let tag_re = Regex::new(r"<[^>]+>").unwrap();

    // Collect cues: (start_time, text) where text is the full cue text joined by newlines
    // This mirrors webvtt.read() which gives c.text as the full cue text (with tags stripped)
    let mut cues: Vec<(String, String)> = Vec::new();
    let mut current_start: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();
    let mut in_cue = false;

    for line in vtt_content.lines() {
        if let Some(caps) = timestamp_re.captures(line) {
            // If we had a previous cue, save it
            if let Some(start) = current_start.take() {
                // Strip trailing whitespace-only lines (matches webvtt library behavior)
                while current_lines.last().map_or(false, |l| l.trim().is_empty()) {
                    current_lines.pop();
                }
                let text = current_lines.join("\n");
                cues.push((start, text));
                current_lines.clear();
            }
            current_start = Some(caps[1].to_string());
            in_cue = true;
        } else if in_cue {
            if line.is_empty() {
                // Empty line ends the cue
                in_cue = false;
            } else {
                // Strip VTT tags from the line (but preserve whitespace)
                let clean = tag_re.replace_all(line, "").to_string();
                current_lines.push(clean);
            }
        }
    }
    // Don't forget the last cue
    if let Some(start) = current_start.take() {
        // Strip trailing whitespace-only lines
        while current_lines.last().map_or(false, |l| l.trim().is_empty()) {
            current_lines.pop();
        }
        let text = current_lines.join("\n");
        cues.push((start, text));
    }

    // Now replicate the Python deduplication algorithm exactly:
    // old_text = ["__bla__"]
    // old_time = "00:00:00"
    // out = [dict(text="")]
    // for c in cues:
    //     if out[-1]["text"] != old_text[-1]:
    //         out.append(dict(text=old_text[-1], time=old_time))
    //     old_text = c.text.split("\n")  -> the text lines of the cue
    //     old_time = c.start

    let mut old_text: Vec<String> = vec!["__bla__".to_string()];
    let mut old_time = "00:00:00".to_string();

    struct Entry {
        text: String,
        time: String,
    }

    let mut out: Vec<Entry> = vec![Entry {
        text: String::new(),
        time: String::new(),
    }];

    for (start_time, cue_text) in &cues {
        let last_out_text = &out.last().unwrap().text;
        let last_old_text = old_text.last().unwrap();

        if last_out_text != last_old_text {
            out.push(Entry {
                text: last_old_text.clone(),
                time: old_time.clone(),
            });
        }

        // Split cue text by newlines (mirrors c.text.split("\n") in Python)
        old_text = cue_text.split('\n').map(|s| s.to_string()).collect();
        old_time = start_time.clone();
    }

    // Format output: skip the first two entries (initialization artifacts)
    let mut result = String::new();
    for entry in out.iter().skip(2) {
        let tstamp = truncate_timestamp(&entry.time);
        result.push_str(&format!("{} {}\n", tstamp, entry.text));
    }

    result
}

/// Truncate "HH:MM:SS.mmm" to "HH:MM:SS"
fn truncate_timestamp(ts: &str) -> String {
    ts.split('.').next().unwrap_or(ts).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vtt_matches_python_output() {
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

        assert_eq!(result, expected, "VTT parser output must match Python implementation byte-for-byte");
    }

    #[test]
    fn test_truncate_timestamp() {
        assert_eq!(truncate_timestamp("00:00:05.120"), "00:00:05");
        assert_eq!(truncate_timestamp("01:23:45.678"), "01:23:45");
        assert_eq!(truncate_timestamp("00:00:00"), "00:00:00");
    }

    #[test]
    fn test_parse_vtt_empty_input() {
        let result = parse_vtt("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_parse_vtt_minimal() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello world\n\n00:00:03.000 --> 00:00:04.000\nGoodbye world\n";
        let result = parse_vtt(vtt);
        assert_eq!(result, "00:00:01 Hello world\n");
    }
}
