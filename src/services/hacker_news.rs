use std::collections::VecDeque;
use std::net::IpAddr;
use tracing;

#[derive(Debug, serde::Deserialize)]
pub struct HnItem {
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub score: Option<i64>,
    #[serde(default)]
    pub kids: Option<Vec<u64>>,
    #[serde(default)]
    pub r#type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HnFetchResult {
    pub story_id: u64,
    pub title: String,
    pub hn_url: String,
    pub article_url: Option<String>,
    pub article_text: Option<String>,
    pub article_fetch_failed: bool,
    pub article_fetch_error: Option<String>,
    pub discussion_text: String,
    pub combined_text: String,
}

pub struct HackerNewsService {
    client: reqwest::Client,
    /// Redirects disabled: external articles go through a manual hop loop
    /// that re-validates every URL (SSRF protection).
    article_client: reqwest::Client,
}

impl Default for HackerNewsService {
    fn default() -> Self {
        Self::new()
    }
}

impl HackerNewsService {
    pub fn new() -> Self {
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .user_agent(user_agent)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let article_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(user_agent)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            article_client,
        }
    }

    /// Fetch a Hacker News item by ID, its comments, and its linked article (if available).
    pub async fn fetch_hn_submission(
        &self,
        story_id: u64,
        user_pasted_content: Option<&str>,
    ) -> Result<HnFetchResult, String> {
        tracing::info!(story_id = story_id, "Fetching Hacker News story metadata");
        let story_url = format!(
            "https://hacker-news.firebaseio.com/v0/item/{}.json",
            story_id
        );
        let story_res = self
            .client
            .get(&story_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch HN item {}: {}", story_id, e))?;

        if !story_res.status().is_success() {
            return Err(format!(
                "HN API returned HTTP status {}",
                story_res.status()
            ));
        }

        let story: HnItem = story_res
            .json()
            .await
            .map_err(|e| format!("Failed to parse HN item JSON: {}", e))?;

        let title = story
            .title
            .clone()
            .unwrap_or_else(|| format!("HN Item {}", story_id));
        let hn_url = format!("https://news.ycombinator.com/item?id={}", story_id);
        let article_url = story.url.clone();
        let author = story.by.as_deref().unwrap_or("anonymous");
        let score = story.score.unwrap_or(0);

        // Fetch comments
        let discussion_text = self.fetch_comments(&story).await;

        let disc_word_count = discussion_text.split_whitespace().count();
        tracing::info!(
            story_id = story_id,
            size_bytes = discussion_text.len(),
            word_count = disc_word_count,
            "Fetched Hacker News discussion comments successfully"
        );

        // Fetch article content if external URL exists
        let mut article_text = None;
        let mut article_fetch_failed = false;
        let mut article_fetch_error = None;

        if let Some(ref ext_url) = article_url
            && !ext_url.contains("news.ycombinator.com")
        {
            match self.fetch_external_article(ext_url).await {
                Ok(text) => {
                    article_text = Some(text);
                }
                Err(e) => {
                    tracing::warn!(url = %ext_url, error = %e, "Failed to download external article");
                    article_fetch_failed = true;
                    article_fetch_error = Some(e);
                }
            }
        }

        // Build combined text buffer
        let mut combined_text = String::new();
        combined_text.push_str("=== HACKER NEWS SUBMISSION ===\n");
        combined_text.push_str(&format!("Title: {}\n", title));
        combined_text.push_str(&format!("HN Link: {}\n", hn_url));
        if let Some(ref url) = article_url {
            combined_text.push_str(&format!("Article Link: {}\n", url));
        }
        combined_text.push_str(&format!("Author: {} | Points: {}\n\n", author, score));

        if let Some(ref self_text) = story.text {
            let clean_self = clean_html_to_text(self_text);
            if !clean_self.is_empty() {
                combined_text.push_str("--- Submission Text ---\n");
                combined_text.push_str(&clean_self);
                combined_text.push_str("\n\n");
            }
        }

        combined_text.push_str("=== ARTICLE CONTENT ===\n");
        if let Some(ref art_text) = article_text {
            combined_text.push_str(art_text);
            combined_text.push_str("\n\n");
        } else if article_fetch_failed {
            let err_msg = article_fetch_error
                .as_deref()
                .unwrap_or("HTTP fetch failed");
            combined_text.push_str(&format!(
                "[Note: External article could not be downloaded via HTTP ({})].\n\n",
                err_msg
            ));
        } else {
            combined_text.push_str("[No external article link for this submission].\n\n");
        }

        if let Some(pasted) = user_pasted_content {
            let trimmed = pasted.trim();
            if !trimmed.is_empty() {
                combined_text.push_str("=== USER PASTED ARTICLE CONTENT ===\n");
                combined_text.push_str(trimmed);
                combined_text.push_str("\n\n");
            }
        }

        combined_text.push_str("=== HACKER NEWS DISCUSSION (COMMENTS) ===\n");
        if discussion_text.is_empty() {
            combined_text.push_str("(No comments found on this post)\n");
        } else {
            combined_text.push_str(&discussion_text);
        }

        Ok(HnFetchResult {
            story_id,
            title,
            hn_url,
            article_url,
            article_text,
            article_fetch_failed,
            article_fetch_error,
            discussion_text,
            combined_text,
        })
    }

    /// Fetch only a story's title (single cheap item request, no comments,
    /// no external article). Used by the title backfill worker.
    pub async fn fetch_story_title(&self, story_id: u64) -> Result<String, String> {
        let story_url = format!(
            "https://hacker-news.firebaseio.com/v0/item/{}.json",
            story_id
        );
        let story: HnItem = self
            .client
            .get(&story_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch HN item {}: {}", story_id, e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse HN item JSON: {}", e))?;
        story
            .title
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| format!("HN item {} has no title", story_id))
    }

    /// Fetch comments recursively (up to ~80 comments, max depth 4).
    async fn fetch_comments(&self, story: &HnItem) -> String {
        let mut result = String::new();
        let Some(ref kids) = story.kids else {
            return result;
        };

        let mut queue: VecDeque<(u64, usize)> = kids.iter().map(|&id| (id, 0)).collect();
        let mut fetched_count = 0;
        let max_comments = 80;

        while let Some((comment_id, depth)) = queue.pop_front() {
            if fetched_count >= max_comments || depth > 4 {
                continue;
            }

            let comment_url = format!(
                "https://hacker-news.firebaseio.com/v0/item/{}.json",
                comment_id
            );
            if let Ok(res) = self.client.get(&comment_url).send().await
                && let Ok(comment) = res.json::<HnItem>().await
            {
                if let Some(ref text) = comment.text {
                    let clean_text = clean_html_to_text(text);
                    if !clean_text.is_empty() {
                        let author = comment.by.as_deref().unwrap_or("anonymous");
                        let indent = "  ".repeat(depth);
                        result.push_str(&format!("{}[{}] (by {}):\n", indent, comment_id, author));
                        for line in clean_text.lines() {
                            result.push_str(&format!("{}  {}\n", indent, line));
                        }
                        result.push('\n');
                        fetched_count += 1;
                    }
                }

                if let Some(ref comment_kids) = comment.kids {
                    for &kid_id in comment_kids {
                        queue.push_back((kid_id, depth + 1));
                    }
                }
            }
        }

        result
    }

    /// Fetch external article and convert HTML to plain text.
    ///
    /// SSRF-hardened: only `http(s)` URLs are fetched, redirects are
    /// followed manually (max 5 hops) with every hop re-validated, resolved
    /// IPs must be public, and the body is capped at 5 MiB.
    async fn fetch_external_article(&self, url: &str) -> Result<String, String> {
        const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;
        const MAX_REDIRECTS: usize = 5;
        tracing::info!(url = %url, "Downloading external article for Hacker News submission");
        let mut current = validate_fetch_url(url).await?;
        let mut hops = 0;
        let html = loop {
            let res = self
                .article_client
                .get(current.clone())
                .send()
                .await
                .map_err(|e| format!("HTTP GET error: {}", e))?;

            if res.status().is_redirection()
                && let Some(location) = res
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
            {
                hops += 1;
                if hops > MAX_REDIRECTS {
                    return Err("Too many redirects".to_string());
                }
                let next = current
                    .join(location)
                    .map_err(|e| format!("Invalid redirect target: {}", e))?;
                current = validate_fetch_url(next.as_str()).await?;
                continue;
            }

            if !res.status().is_success() {
                return Err(format!("HTTP status {}", res.status()));
            }

            if let Some(len) = res.content_length()
                && len > MAX_BODY_BYTES as u64
            {
                return Err(format!(
                    "Response body too large ({} bytes, max {} bytes)",
                    len, MAX_BODY_BYTES
                ));
            }

            let mut body: Vec<u8> = Vec::new();
            let mut res = res;
            while let Some(chunk) = res
                .chunk()
                .await
                .map_err(|e| format!("Failed to read response body: {}", e))?
            {
                body.extend_from_slice(&chunk);
                if body.len() > MAX_BODY_BYTES {
                    return Err(format!(
                        "Response body too large (over {} bytes)",
                        MAX_BODY_BYTES
                    ));
                }
            }
            break String::from_utf8_lossy(&body).into_owned();
        };

        let clean_text = clean_html_to_text(&html);
        if clean_text.trim().is_empty() {
            return Err("Extracted text content was empty".to_string());
        }

        let size_bytes = clean_text.len();
        let word_count = clean_text.split_whitespace().count();
        tracing::info!(
            url = %url,
            size_bytes = size_bytes,
            word_count = word_count,
            "Downloaded and parsed external article successfully"
        );

        // Limit to first 25,000 words (~100KB)
        let words: Vec<&str> = clean_text.split_whitespace().take(25_000).collect();
        Ok(words.join(" "))
    }
}

/// Validates an external article URL before any request is made: only
/// `http`/`https` schemes, and every resolved IP must be public (no
/// loopback, private, link-local, unspecified, multicast, CGNAT, or
/// IPv6 unique-local addresses).
async fn validate_fetch_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| format!("Invalid article URL: {}", e))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Blocked article URL scheme: {}", other)),
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Article URL has no host".to_string())?;
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("Failed to resolve article host: {}", e))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(&addr.ip()) {
            return Err(format!("Blocked article host address: {}", addr.ip()));
        }
    }
    if !any {
        return Err("Article host resolves to no address".to_string());
    }
    Ok(parsed)
}

/// Whether an IP address must never be fetched (SSRF blocklist).
fn is_blocked_ip(ip: &IpAddr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return true;
    }
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_link_local() || is_cgnat(v),
        IpAddr::V6(v) => v.is_unique_local() || v.is_unicast_link_local(),
    }
}

/// Carrier-Grade NAT range 100.64.0.0/10 (not covered by `is_private`).
fn is_cgnat(ip: &std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

/// Helper to strip HTML tags, convert block elements to newlines, and unescape HTML entities.
pub fn clean_html_to_text(html: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static TAG_BLOCKS: OnceLock<Vec<Regex>> = OnceLock::new();
    let tag_blocks = TAG_BLOCKS.get_or_init(|| {
        vec![
            Regex::new(r"(?is)<script\b[^>]*>.*?</script>").unwrap(),
            Regex::new(r"(?is)<style\b[^>]*>.*?</style>").unwrap(),
            Regex::new(r"(?is)<header\b[^>]*>.*?</header>").unwrap(),
            Regex::new(r"(?is)<footer\b[^>]*>.*?</footer>").unwrap(),
            Regex::new(r"(?is)<nav\b[^>]*>.*?</nav>").unwrap(),
            Regex::new(r"(?is)<noscript\b[^>]*>.*?</noscript>").unwrap(),
        ]
    });

    static LINE_BREAKS: OnceLock<Regex> = OnceLock::new();
    let line_breaks =
        LINE_BREAKS.get_or_init(|| Regex::new(r"(?i)<(?:p|br|h[1-6]|li|div|tr)\b[^>]*>").unwrap());

    static LINK_RE: OnceLock<Regex> = OnceLock::new();
    let link_re = LINK_RE.get_or_init(|| {
        Regex::new(r#"(?i)<a\b[^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>"#).unwrap()
    });

    static STRIP_TAGS: OnceLock<Regex> = OnceLock::new();
    let strip_tags = STRIP_TAGS.get_or_init(|| Regex::new(r"<[^>]+>").unwrap());

    let mut current = html.to_string();
    for re in tag_blocks {
        current = re.replace_all(&current, "").to_string();
    }

    current = line_breaks.replace_all(&current, "\n").to_string();
    current = link_re.replace_all(&current, "$2 ($1)").to_string();
    current = strip_tags.replace_all(&current, "").to_string();

    let text = decode_html_entities(&current);

    let mut cleaned_lines = Vec::new();
    let mut empty_count = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            empty_count += 1;
            if empty_count <= 1 {
                cleaned_lines.push("");
            }
        } else {
            empty_count = 0;
            cleaned_lines.push(trimmed);
        }
    }

    cleaned_lines.join("\n")
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_blocked_ip_ranges() {
        // Loopback, private, link-local, unspecified, multicast, CGNAT.
        for ip in [
            "127.0.0.1",
            "10.0.0.5",
            "172.16.4.2",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.10.20",
            "0.0.0.0",
            "224.0.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
        ] {
            assert!(
                is_blocked_ip(&IpAddr::from_str(ip).unwrap()),
                "expected blocked: {ip}"
            );
        }
        // Public and documentation ranges pass the IP check.
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "192.0.2.1",
            "203.0.113.7",
        ] {
            assert!(
                !is_blocked_ip(&IpAddr::from_str(ip).unwrap()),
                "expected allowed: {ip}"
            );
        }
    }

    #[tokio::test]
    async fn test_validate_fetch_url_rejects_wrong_scheme() {
        assert!(validate_fetch_url("file:///etc/passwd").await.is_err());
        assert!(validate_fetch_url("ftp://example.com/x").await.is_err());
    }

    #[tokio::test]
    async fn test_validate_fetch_url_rejects_private_ips() {
        assert!(
            validate_fetch_url("http://127.0.0.1/article")
                .await
                .is_err()
        );
        assert!(validate_fetch_url("http://10.1.2.3:8080/x").await.is_err());
        assert!(validate_fetch_url("http://[::1]/x").await.is_err());
    }

    #[tokio::test]
    async fn test_validate_fetch_url_accepts_public_ip_literal() {
        // Documentation range (no DNS, no fetch): passes validation.
        assert!(validate_fetch_url("http://192.0.2.1/article").await.is_ok());
    }

    #[test]
    fn test_clean_html_to_text_basic() {
        let html = "<p>Hello <b>World</b></p><p>Second paragraph</p>";
        let cleaned = clean_html_to_text(html);
        assert!(cleaned.contains("Hello World"));
        assert!(cleaned.contains("Second paragraph"));
    }

    #[test]
    fn test_clean_html_to_text_links_and_entities() {
        let html = "<a href=\"https://example.com\">Example &amp; Link</a>";
        let cleaned = clean_html_to_text(html);
        assert_eq!(cleaned, "Example & Link (https://example.com)");
    }
}
