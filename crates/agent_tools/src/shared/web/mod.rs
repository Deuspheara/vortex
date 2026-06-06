use std::collections::HashSet;
use std::net::IpAddr;

use agent_protocol::ToolContext;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::shared::provenance::{Confidence, SourceProvenance};

pub const DEFAULT_MAX_FETCH_BYTES: usize = 100_000;
pub const MAX_FETCH_BYTES_LIMIT: usize = 500_000;
pub const DEFAULT_SEARCH_RESULTS: usize = 8;
pub const MAX_SEARCH_RESULTS: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub published_at: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuredToolOutput {
    pub summary: String,
    pub data: Value,
}

pub fn structured_output(summary: impl Into<String>, data: Value) -> Result<String, String> {
    serde_json::to_string_pretty(&StructuredToolOutput {
        summary: summary.into(),
        data,
    })
    .map_err(|e| e.to_string())
}

pub fn public_http_url(input: &str) -> Result<Url, String> {
    let url = Url::parse(input).map_err(|e| format!("invalid URL: {e}"))?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("only absolute http(s) URLs are allowed".into()),
    }
    let host = url
        .host_str()
        .ok_or_else(|| "URL must include a host".to_string())?;
    if is_blocked_host(host) {
        return Err("local, private, and internal network hosts are not allowed".into());
    }
    Ok(url)
}

pub fn public_http_url_from_tool_args(args: &Value, key: &str) -> Result<Url, String> {
    let url = args
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing {key}"))?;
    public_http_url(url)
}

pub fn is_blocked_host(host: &str) -> bool {
    let lower = host.trim_matches('.').to_ascii_lowercase();
    if lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
    {
        return true;
    }
    if let Ok(ip) = lower.parse::<IpAddr>() {
        return is_private_ip(ip);
    }
    false
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || matches!(v6.segments()[0] & 0xfe00, 0xfc00)
                || matches!(v6.segments()[0] & 0xffc0, 0xfe80)
        }
    }
}

pub fn clamp_max_bytes(args: &Value) -> usize {
    args.get("max_bytes")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_MAX_FETCH_BYTES)
        .clamp(1_000, MAX_FETCH_BYTES_LIMIT)
}

pub fn truncate_chars(input: &str, max_chars: usize) -> (String, bool) {
    if input.chars().count() <= max_chars {
        return (input.to_string(), false);
    }
    let mut out: String = input.chars().take(max_chars).collect();
    out.push_str("\n\n[truncated]");
    (out, true)
}

pub fn normalize_provider_list(args: &Value) -> Vec<String> {
    let raw = args.get("providers");
    let mut providers = match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_ascii_lowercase())
            .collect::<Vec<_>>(),
        Some(Value::String(s)) => vec![s.to_ascii_lowercase()],
        _ => vec!["auto".into()],
    };
    if providers.is_empty() || providers.iter().any(|p| p == "auto") {
        providers = vec![
            "exa".into(),
            "tavily".into(),
            "jina".into(),
            "firecrawl".into(),
            "openai".into(),
            "anthropic".into(),
        ];
    }
    providers
}

pub fn dedupe_results(results: Vec<WebSearchResult>, max_results: usize) -> Vec<WebSearchResult> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for result in results {
        let key = normalize_url_key(&result.url);
        if seen.insert(key) {
            deduped.push(result);
        }
        if deduped.len() >= max_results {
            break;
        }
    }
    deduped
}

fn normalize_url_key(url: &str) -> String {
    Url::parse(url)
        .map(|mut parsed| {
            parsed.set_fragment(None);
            parsed
                .to_string()
                .trim_end_matches('/')
                .to_ascii_lowercase()
        })
        .unwrap_or_else(|_| url.trim_end_matches('/').to_ascii_lowercase())
}

pub async fn search_with_providers(
    client: &reqwest::Client,
    args: &Value,
) -> (Vec<WebSearchResult>, Vec<String>) {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_SEARCH_RESULTS)
        .clamp(1, MAX_SEARCH_RESULTS);
    let domains = args
        .get("domains")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let freshness = args
        .get("freshness")
        .and_then(|v| v.as_str())
        .unwrap_or("any");
    let mut warnings = Vec::new();
    let mut results = Vec::new();
    for provider in normalize_provider_list(args) {
        match provider.as_str() {
            "exa" => match search_exa(client, &query, max_results, &domains).await {
                Ok(mut items) => results.append(&mut items),
                Err(reason) => warnings.push(format!("exa: {reason}")),
            },
            "tavily" => match search_tavily(client, &query, max_results, freshness, &domains).await
            {
                Ok(mut items) => results.append(&mut items),
                Err(reason) => warnings.push(format!("tavily: {reason}")),
            },
            "jina" => match search_jina(client, &query, max_results).await {
                Ok(mut items) => results.append(&mut items),
                Err(reason) => warnings.push(format!("jina: {reason}")),
            },
            "firecrawl" => match search_firecrawl(client, &query, max_results).await {
                Ok(mut items) => results.append(&mut items),
                Err(reason) => warnings.push(format!("firecrawl: {reason}")),
            },
            "openai" | "anthropic" => {
                warnings.push(format!(
                    "{provider}: model-native web search adapter is reserved; no direct client is configured"
                ));
            }
            other => warnings.push(format!("{other}: unknown provider")),
        }
        if results.len() >= max_results {
            break;
        }
    }
    (dedupe_results(results, max_results), warnings)
}

async fn search_exa(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    domains: &[String],
) -> Result<Vec<WebSearchResult>, String> {
    let key = std::env::var("EXA_API_KEY").map_err(|_| "EXA_API_KEY is not configured")?;
    let mut body = json!({
        "query": query,
        "numResults": max_results,
        "contents": { "text": { "maxCharacters": 500 } }
    });
    if !domains.is_empty() {
        body["includeDomains"] = json!(domains);
    }
    let response = client
        .post("https://api.exa.ai/search")
        .header("x-api-key", key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    provider_results_from_response(response, "exa").await
}

async fn search_tavily(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    freshness: &str,
    domains: &[String],
) -> Result<Vec<WebSearchResult>, String> {
    let key = std::env::var("TAVILY_API_KEY").map_err(|_| "TAVILY_API_KEY is not configured")?;
    let mut body = json!({
        "query": query,
        "max_results": max_results,
        "include_answer": false,
        "include_raw_content": false
    });
    if freshness != "any" {
        body["time_range"] = json!(freshness);
    }
    if !domains.is_empty() {
        body["include_domains"] = json!(domains);
    }
    let response = client
        .post("https://api.tavily.com/search")
        .header("Authorization", format!("Bearer {key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    provider_results_from_response(response, "tavily").await
}

async fn search_jina(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
) -> Result<Vec<WebSearchResult>, String> {
    let url =
        Url::parse_with_params("https://s.jina.ai/", &[("q", query)]).map_err(|e| e.to_string())?;
    let mut request = client.get(url).header("Accept", "application/json");
    if let Ok(key) = std::env::var("JINA_API_KEY") {
        request = request.header("Authorization", format!("Bearer {key}"));
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    let mut items = provider_results_from_response(response, "jina").await?;
    items.truncate(max_results);
    Ok(items)
}

async fn search_firecrawl(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
) -> Result<Vec<WebSearchResult>, String> {
    let key =
        std::env::var("FIRECRAWL_API_KEY").map_err(|_| "FIRECRAWL_API_KEY is not configured")?;
    let response = client
        .post("https://api.firecrawl.dev/v1/search")
        .header("Authorization", format!("Bearer {key}"))
        .json(&json!({ "query": query, "limit": max_results }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    provider_results_from_response(response, "firecrawl").await
}

async fn provider_results_from_response(
    response: reqwest::Response,
    provider: &str,
) -> Result<Vec<WebSearchResult>, String> {
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "HTTP {status}: {}",
            text.chars().take(240).collect::<String>()
        ));
    }
    let value: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "text": text }));
    let candidates = value
        .get("results")
        .or_else(|| value.get("data"))
        .or_else(|| value.get("items"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut results = Vec::new();
    for item in candidates {
        let title = first_string(&item, &["title", "name"]).unwrap_or_else(|| "Untitled".into());
        let url = first_string(&item, &["url", "link", "href"]).unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        let snippet =
            first_string(&item, &["snippet", "content", "text", "description"]).unwrap_or_default();
        let published_at = first_string(&item, &["published_date", "publishedAt", "date"]);
        results.push(WebSearchResult {
            title,
            url,
            snippet,
            published_at,
            source: provider.to_string(),
        });
    }
    Ok(results)
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|v| v.as_str()))
        .map(ToString::to_string)
}

pub async fn fetch_http(client: &reqwest::Client, args: &Value) -> Result<Value, String> {
    let url = public_http_url_from_tool_args(args, "url")?;
    let extract = args
        .get("extract")
        .and_then(|v| v.as_str())
        .unwrap_or("markdown");
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("auto");
    if matches!(mode, "browser") {
        return Err(
            "web_fetch mode=browser is not implemented; use browser_snapshot or browser_screenshot"
                .into(),
        );
    }
    let max_bytes = clamp_max_bytes(args);
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(ToString::to_string);
    let body = response.text().await.map_err(|e| e.to_string())?;
    let (truncated_body, truncated) = truncate_chars(&body, max_bytes);
    let text = html_to_textish(&truncated_body);
    let links = if matches!(extract, "links" | "metadata" | "markdown" | "text" | "html") {
        extract_links(&truncated_body, &final_url)
    } else {
        Vec::new()
    };
    let confidence = if status.is_success() {
        Confidence::High
    } else {
        Confidence::Low
    };
    let provenance = SourceProvenance::new(
        Some(url.to_string()),
        Some(final_url.clone()),
        "http",
        extract,
    );
    let mut warnings = Vec::new();
    if truncated {
        warnings.push(format!("response truncated to {max_bytes} characters"));
    }
    if !status.is_success() {
        warnings.push(format!("HTTP status {status}"));
    }
    Ok(json!({
        "url": url.to_string(),
        "final_url": final_url,
        "title": extract_title(&truncated_body),
        "status": status.as_u16(),
        "content_type": content_type,
        "markdown": if matches!(extract, "markdown") { Some(text.clone()) } else { None::<String> },
        "text": if matches!(extract, "text" | "markdown") { Some(text) } else { None::<String> },
        "html": if matches!(extract, "html") { Some(truncated_body) } else { None::<String> },
        "links": links,
        "metadata": {
            "status": status.as_u16(),
            "content_type": content_type,
            "provider": "http"
        },
        "confidence": confidence,
        "warnings": warnings,
        "provenance": provenance
    }))
}

fn html_to_textish(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(32_000));
    let mut in_tag = false;
    let mut last_space = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                if !last_space {
                    out.push(' ');
                    last_space = true;
                }
            }
            '>' => in_tag = false,
            _ if in_tag => {}
            ch if ch.is_whitespace() => {
                if !last_space {
                    out.push(' ');
                    last_space = true;
                }
            }
            ch => {
                out.push(ch);
                last_space = false;
            }
        }
    }
    decode_basic_entities(out.trim())
}

fn decode_basic_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    Some(decode_basic_entities(html[start..end].trim()))
}

fn extract_links(html: &str, base: &str) -> Vec<Value> {
    let Ok(base_url) = Url::parse(base) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for part in html.split("<a ").skip(1).take(200) {
        let Some(href_pos) = part.find("href=") else {
            continue;
        };
        let raw = &part[href_pos + 5..];
        let Some(quote) = raw.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(end) = raw[1..].find(quote) else {
            continue;
        };
        let href = &raw[1..1 + end];
        let resolved = base_url
            .join(href)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| href.to_string());
        let text = part
            .find('>')
            .and_then(|s| part[s + 1..].find("</a>").map(|e| &part[s + 1..s + 1 + e]))
            .map(html_to_textish)
            .unwrap_or_default();
        links.push(json!({ "text": text, "href": resolved }));
    }
    links
}

pub fn tool_mode_allows_network(ctx: &ToolContext) -> bool {
    ctx.mode.can_run_real_commands()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_private_urls() {
        assert!(public_http_url("http://127.0.0.1:3000").is_err());
        assert!(public_http_url("http://10.0.0.4").is_err());
        assert!(public_http_url("http://localhost").is_err());
        assert!(public_http_url("https://example.com").is_ok());
    }

    #[test]
    fn dedupes_urls() {
        let results = dedupe_results(
            vec![
                WebSearchResult {
                    title: "A".into(),
                    url: "https://example.com/path#x".into(),
                    snippet: String::new(),
                    published_at: None,
                    source: "test".into(),
                },
                WebSearchResult {
                    title: "B".into(),
                    url: "https://example.com/path".into(),
                    snippet: String::new(),
                    published_at: None,
                    source: "test".into(),
                },
            ],
            10,
        );
        assert_eq!(results.len(), 1);
    }
}
