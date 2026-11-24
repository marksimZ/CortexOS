use crate::{
    benchmark::dataset::require_workspace,
    cloud::vault::WindowsVault,
    commands::{AppState, blocking},
    http_transport::model_client,
    models::{AppResult, now},
};
use quick_xml::{Reader, escape::resolve_predefined_entity, events::Event};
use reqwest::{RequestBuilder, StatusCode, Url, header::HeaderValue};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, net::IpAddr, path::Path, time::Duration};
use tauri::State;
use zeroize::Zeroizing;

const ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";
const NEWS_ENDPOINT: &str = "https://www.bing.com/news/search";
const FIXTURE_KEY: &str = "cortexos-brave-fixture";
const MAX_RESULTS: usize = 5;
const MAX_BYTES: usize = 256 * 1024;

pub(crate) struct WebSearch {
    fixture: AppResult<Option<Url>>,
}

impl WebSearch {
    pub(crate) fn new(_default_directory: &Path) -> Self {
        #[cfg(debug_assertions)]
        let fixture = fixture_endpoint(_default_directory);
        #[cfg(not(debug_assertions))]
        let fixture = Ok(None);
        Self { fixture }
    }
}

#[cfg(debug_assertions)]
fn fixture_endpoint(default_directory: &Path) -> AppResult<Option<Url>> {
    let invalid = || "Brave 搜索替身需要独立测试数据目录及 127.0.0.1 回环地址。".to_string();
    let Some(endpoint) = std::env::var_os("CORTEXOS_TEST_BRAVE_URL") else {
        return Ok(None);
    };
    let directory = std::env::var_os("CORTEXOS_TEST_DATA_DIR").ok_or_else(invalid)?;
    let directory = Path::new(&directory);
    if !directory.is_absolute() {
        return Err(invalid());
    }
    let actual = directory.canonicalize().map_err(|_| invalid())?;
    let normal = default_directory
        .canonicalize()
        .unwrap_or_else(|_| default_directory.to_owned());
    if actual == normal || actual.starts_with(&normal) || normal.starts_with(&actual) {
        return Err(invalid());
    }
    let mut endpoint = Url::parse(endpoint.to_str().ok_or_else(invalid)?).map_err(|_| invalid())?;
    if endpoint.scheme() != "http"
        || endpoint.host_str() != Some("127.0.0.1")
        || endpoint.port().is_none()
        || !matches!(endpoint.path(), "/" | "/res/v1/web/search")
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(invalid());
    }
    endpoint.set_path("/res/v1/web/search");
    Ok(Some(endpoint))
}

async fn workspace_vault(
    state: &AppState,
    workspace: String,
    fixture: bool,
) -> AppResult<WindowsVault> {
    blocking(state.database.clone()?, move |db| {
        require_workspace(&*db.connection()?, &workspace)?;
        Ok(WindowsVault::for_web_search(&db.path, &workspace, fixture))
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct WebSearchStatus {
    configured: bool,
}

#[tauri::command]
pub(crate) async fn read_web_search_status(
    state: State<'_, AppState>,
    search: State<'_, WebSearch>,
    workspace_id: String,
) -> AppResult<WebSearchStatus> {
    let fixture = search.fixture.as_ref().map_err(Clone::clone)?.is_some();
    let vault = workspace_vault(&state, workspace_id, fixture).await?;
    let key = vault.read_payload::<String>()?.map(Zeroizing::new);
    Ok(WebSearchStatus {
        configured: key.is_some(),
    })
}

#[tauri::command]
pub(crate) async fn save_web_search_key(
    state: State<'_, AppState>,
    search: State<'_, WebSearch>,
    workspace_id: String,
    key: String,
) -> AppResult<()> {
    let key = Zeroizing::new(key);
    let value = key.trim();
    let fixture = search.fixture.as_ref().map_err(Clone::clone)?.is_some();
    let vault = workspace_vault(&state, workspace_id, fixture).await?;
    if value.is_empty() {
        return vault.erase();
    }
    if (fixture && value != FIXTURE_KEY) || (!fixture && value == FIXTURE_KEY) {
        return Err("Brave 搜索测试密钥仅用于本地替身。".into());
    }
    if value.len() > 256 || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("Brave Search API 密钥格式无效。".into());
    }
    vault.write_payload(&value)
}

#[derive(Deserialize)]
struct BraveResponse {
    web: Option<BraveResults>,
}

#[derive(Deserialize)]
struct BraveResults {
    results: Vec<BraveItem>,
}

#[derive(Deserialize)]
struct BraveItem {
    title: String,
    url: String,
    description: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct WebResult {
    title: String,
    url: String,
    snippet: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebResults {
    results: Vec<WebResult>,
    searched_at: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchProvider {
    #[default]
    News,
    Brave,
}

fn result_url(value: &str) -> Option<Url> {
    if value.len() > 4096 {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.');
    let public = match host.trim_matches(['[', ']']).parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            let [a, b, c, _] = address.octets();
            !address.is_private()
                && !address.is_loopback()
                && !address.is_link_local()
                && !address.is_documentation()
                && a != 0
                && a < 224
                && !matches!(
                    (a, b, c),
                    (100, 64..=127, _) | (192, 0, 0) | (192, 88, 99) | (198, 18..=19, _)
                )
        }
        Ok(IpAddr::V6(address)) => {
            let [a, b, ..] = address.segments();
            (0x2000..0x4000).contains(&a)
                && !matches!(
                    (a, b),
                    (0x2001, 0..=0x1ff) | (0x2001, 0xdb8) | (0x2002, _) | (0x3fff, _)
                )
        }
        Err(_) => host.rsplit_once('.').is_some_and(|(_, tld)| {
            !matches!(
                tld,
                "localhost"
                    | "local"
                    | "internal"
                    | "test"
                    | "invalid"
                    | "onion"
                    | "arpa"
                    | "lan"
                    | "home"
            )
        }),
    };
    public.then_some(url)
}

#[tauri::command]
pub(crate) async fn search_web(
    state: State<'_, AppState>,
    search: State<'_, WebSearch>,
    workspace_id: String,
    query: String,
    provider: Option<SearchProvider>,
) -> AppResult<WebResults> {
    let query = validate_query(&query)?;
    let results = match provider.unwrap_or_default() {
        SearchProvider::News => {
            blocking(state.database.clone()?, move |db| {
                require_workspace(&*db.connection()?, &workspace_id)
            })
            .await?;
            public_news(&search, query).await?
        }
        SearchProvider::Brave => {
            let (client, fixture) = search_client(&search)?;
            let request =
                brave_request(&state, fixture.as_ref(), workspace_id, query, &client).await?;
            read_brave(&search_body(request, "Brave 搜索", true).await?)?
        }
    };
    Ok(WebResults {
        results,
        searched_at: now(),
    })
}

pub(crate) fn validate_query(query: &str) -> AppResult<&str> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 300 || query.chars().any(char::is_control) {
        return Err("请输入 1 至 300 个字符的单行搜索词。".into());
    }
    Ok(query)
}

pub(crate) async fn public_news(search: &WebSearch, query: &str) -> AppResult<Vec<WebResult>> {
    let (client, fixture) = search_client(search)?;
    let mut endpoint = search_endpoint(fixture.as_ref(), NEWS_ENDPOINT, "/news/search")?;
    endpoint
        .query_pairs_mut()
        .extend_pairs([("q", query), ("format", "rss")]);
    let request = client
        .get(endpoint)
        .header("Accept", "application/rss+xml, application/xml, text/xml");
    read_news(&search_body(request, "Bing 新闻", false).await?)
}

fn search_client(search: &WebSearch) -> AppResult<(reqwest::Client, &Option<Url>)> {
    let fixture = search.fixture.as_ref().map_err(Clone::clone)?;
    let mut builder = model_client(Duration::from_secs(5)).timeout(Duration::from_secs(12));
    if fixture.is_some() {
        builder = builder.no_proxy();
    }
    let client = builder.build().map_err(|_| "无法初始化搜索连接。")?;
    Ok((client, fixture))
}

fn search_endpoint(fixture: Option<&Url>, default: &str, path: &str) -> AppResult<Url> {
    if let Some(fixture) = fixture {
        let mut endpoint = fixture.clone();
        endpoint.set_path(path);
        return Ok(endpoint);
    }
    Url::parse(default).map_err(|_| "搜索地址配置无效。".into())
}

async fn brave_request(
    state: &AppState,
    fixture: Option<&Url>,
    workspace_id: String,
    query: &str,
    client: &reqwest::Client,
) -> AppResult<RequestBuilder> {
    let vault = workspace_vault(&state, workspace_id, fixture.is_some()).await?;
    let key = Zeroizing::new(
        vault
            .read_payload::<String>()?
            .ok_or("请先设置当前工作区的 Brave Search API 密钥。")?,
    );
    let token = if fixture.is_some() {
        if key.as_str() != FIXTURE_KEY {
            return Err("Brave 搜索替身需要测试密钥。".into());
        }
        FIXTURE_KEY
    } else {
        key.as_str()
    };
    let mut token = HeaderValue::from_str(token).map_err(|_| "Brave Search API 密钥格式无效。")?;
    token.set_sensitive(true);
    let mut endpoint = search_endpoint(fixture, ENDPOINT, "/res/v1/web/search")?;
    {
        let mut params = endpoint.query_pairs_mut();
        params.extend_pairs([
            ("q", query),
            ("count", "5"),
            ("text_decorations", "false"),
            ("result_filter", "web"),
        ]);
        if query
            .chars()
            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        {
            params.append_pair("search_lang", "zh-hans");
            params.append_pair("country", "ALL");
        }
        if query.contains("最近一周") || query.contains("过去一周") {
            params.append_pair("freshness", "pw");
        } else if query.contains("今天") {
            params.append_pair("freshness", "pd");
        }
    }
    Ok(client
        .get(endpoint)
        .header("X-Subscription-Token", token)
        .header("Accept", "application/json"))
}

async fn search_body(request: RequestBuilder, service: &str, brave: bool) -> AppResult<Vec<u8>> {
    let mut response = request
        .send()
        .await
        .map_err(|_| format!("{service}连接失败或超时，可稍后重试。"))?;
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN if brave => {
            return Err("Brave 未接受当前密钥，请检查工作区设置。".into());
        }
        StatusCode::TOO_MANY_REQUESTS => {
            return Err(format!("{service}已达请求限制，请稍后重试。"));
        }
        status if !status.is_success() => {
            return Err(format!("{service}请求失败（HTTP {}）。", status.as_u16()));
        }
        _ => {}
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BYTES as u64)
    {
        return Err(format!("{service}响应超过大小限制。"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| format!("{service}响应中断或超时。"))?
    {
        if chunk.len() > MAX_BYTES - body.len() {
            return Err(format!("{service}响应超过大小限制。"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn read_brave(body: &[u8]) -> AppResult<Vec<WebResult>> {
    let response: BraveResponse =
        serde_json::from_slice(body).map_err(|_| "Brave 搜索响应格式异常。")?;
    let mut seen = HashSet::new();
    let results = response
        .web
        .into_iter()
        .flat_map(|web| web.results)
        .filter_map(|item| {
            let url = result_url(&item.url)?;
            make_result(
                &mut seen,
                &item.title,
                url,
                item.description.as_deref().unwrap_or_default(),
            )
        })
        .take(MAX_RESULTS)
        .collect();
    Ok(results)
}

fn make_result(seen: &mut HashSet<Url>, title: &str, url: Url, snippet: &str) -> Option<WebResult> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    let mut canonical = url.clone();
    canonical.set_fragment(None);
    if !seen.insert(canonical) {
        return None;
    }
    Some(WebResult {
        title: title.chars().take(300).collect(),
        url: url.into(),
        snippet: snippet.trim().chars().take(800).collect(),
    })
}

fn news_url(value: &str) -> Option<Url> {
    let url = result_url(value.trim())?;
    if matches!(url.host_str(), Some("bing.com" | "www.bing.com"))
        && url.path().eq_ignore_ascii_case("/news/apiclick.aspx")
    {
        let (_, original) = url.query_pairs().find(|(key, _)| key == "url")?;
        return result_url(&original);
    }
    Some(url)
}

fn read_news(body: &[u8]) -> AppResult<Vec<WebResult>> {
    let malformed = || "Bing 新闻未返回有效的 RSS 资讯。".to_owned();
    let body = std::str::from_utf8(body).map_err(|_| malformed())?;
    let mut reader = Reader::from_str(body);
    reader.config_mut().enable_all_checks(true);
    reader.config_mut().expand_empty_elements = true;
    let mut depth = 0;
    let mut root = false;
    let mut has_channel = false;
    let mut in_channel = false;
    let mut in_item = false;
    let mut fields: [String; 3] = Default::default();
    let mut field: Option<usize> = None;
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    loop {
        match reader.read_event().map_err(|_| malformed())? {
            Event::Start(start) => {
                depth += 1;
                match (depth, start.name().as_ref()) {
                    (1, "rss") if !root => root = true,
                    (1, _) | (65, _) => return Err(malformed()),
                    (2, "channel") => {
                        has_channel = true;
                        in_channel = true;
                    }
                    (3, "item") if in_channel => {
                        in_item = true;
                        fields.iter_mut().for_each(String::clear);
                    }
                    (4, name) if in_item => {
                        field = match name {
                            "title" => Some(0),
                            "link" => Some(1),
                            "description" => Some(2),
                            _ => None,
                        };
                    }
                    _ => {}
                }
            }
            Event::End(_) => {
                match depth {
                    2 => in_channel = false,
                    3 if in_item => {
                        in_item = false;
                        if results.len() < MAX_RESULTS
                            && let Some(url) = news_url(&fields[1])
                            && let Some(item) = make_result(&mut seen, &fields[0], url, &fields[2])
                        {
                            results.push(item);
                        }
                    }
                    4 => field = None,
                    _ => {}
                }
                depth -= 1;
            }
            Event::Text(event) => {
                let text = event.as_ref();
                if let Some(index) = field {
                    fields[index].push_str(text);
                } else if depth == 0 && !text.trim().is_empty() {
                    return Err(malformed());
                }
            }
            Event::CData(event) => {
                if let Some(index) = field {
                    fields[index].push_str(event.as_ref());
                } else if depth == 0 {
                    return Err(malformed());
                }
            }
            Event::GeneralRef(event) => {
                if depth == 0 {
                    return Err(malformed());
                }
                if let Some(index) = field {
                    if let Some(ch) = event.resolve_char_ref().map_err(|_| malformed())? {
                        fields[index].push(ch);
                    } else {
                        fields[index].push_str(
                            resolve_predefined_entity(event.as_ref()).ok_or_else(malformed)?,
                        );
                    }
                }
            }
            Event::DocType(_) => return Err(malformed()),
            Event::Eof if depth == 0 && has_channel => return Ok(results),
            Event::Eof => return Err(malformed()),
            _ => {}
        }
    }
}
