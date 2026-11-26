use crate::{
    benchmark::dataset::require_workspace,
    cloud::vault::WindowsVault,
    commands::{AppState, blocking, spawn_index_maintenance},
    http_transport::model_client,
    models::{AppResult, now},
};
use quick_xml::{Reader, escape::resolve_predefined_entity, events::Event};
use reqwest::{
    RequestBuilder, StatusCode, Url,
    dns::{Addrs, Name, Resolve, Resolving},
    header::HeaderValue,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    net::{IpAddr, ToSocketAddrs},
    path::Path,
    time::Duration,
};
use tauri::State;
use zeroize::Zeroizing;

const ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";
const NEWS_ENDPOINT: &str = "https://www.bing.com/news/search";
const FIXTURE_KEY: &str = "cortexos-brave-fixture";
const MAX_RESULTS: usize = 5;
const MAX_BYTES: usize = 256 * 1024;
const MAX_PAGE_BYTES: usize = 1024 * 1024;
const MAX_PAGE_CHARS: usize = 16_000;

pub(crate) struct WebSearch {
    fixture: AppResult<Option<Url>>,
    #[cfg(debug_assertions)]
    page_fixture: AppResult<Option<Url>>,
}

impl WebSearch {
    pub(crate) fn new(_default_directory: &Path) -> Self {
        #[cfg(debug_assertions)]
        let fixture = fixture_endpoint(_default_directory);
        #[cfg(not(debug_assertions))]
        let fixture = Ok(None);
        Self {
            fixture,
            #[cfg(debug_assertions)]
            page_fixture: loopback_fixture_endpoint(
                _default_directory,
                "CORTEXOS_TEST_WEB_PAGE_URL",
                "网页抓取替身需要独立测试数据目录及 127.0.0.1 回环地址。",
            ),
        }
    }
}

#[cfg(debug_assertions)]
fn fixture_endpoint(default_directory: &Path) -> AppResult<Option<Url>> {
    let message = "Brave 搜索替身需要独立测试数据目录及 127.0.0.1 回环地址。";
    let Some(mut endpoint) =
        loopback_fixture_endpoint(default_directory, "CORTEXOS_TEST_BRAVE_URL", message)?
    else {
        return Ok(None);
    };
    if endpoint.port().is_none()
        || !matches!(endpoint.path(), "/" | "/res/v1/web/search")
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(message.into());
    }
    endpoint.set_path("/res/v1/web/search");
    Ok(Some(endpoint))
}

#[cfg(debug_assertions)]
fn loopback_fixture_endpoint(
    default_directory: &Path,
    variable: &str,
    message: &str,
) -> AppResult<Option<Url>> {
    let invalid = || message.to_string();
    let Some(endpoint) = std::env::var_os(variable) else {
        return Ok(None);
    };
    let directory = std::env::var("CORTEXOS_TEST_DATA_DIR").map_err(|_| invalid())?;
    let directory = Path::new(&directory);
    if !directory.is_absolute() {
        return Err(invalid());
    }
    let actual = directory.canonicalize().map_err(|_| invalid())?;
    // Compare canonical paths even when the default data directory has yet to be created.
    let mut ancestor = default_directory;
    let normal = loop {
        match ancestor.canonicalize() {
            Ok(path) => {
                let suffix = default_directory
                    .strip_prefix(ancestor)
                    .map_err(|_| invalid())?;
                break path.join(suffix);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or_else(invalid)?;
            }
            Err(_) => return Err(invalid()),
        }
    };
    if actual == normal || actual.starts_with(&normal) || normal.starts_with(&actual) {
        return Err(invalid());
    }
    let raw = endpoint.to_str().ok_or_else(invalid)?;
    let endpoint = Url::parse(raw).map_err(|_| invalid())?;
    // Url normalizes an explicit HTTP port 80 to the implicit default.
    let explicit_default_port = || {
        raw.strip_prefix("http://127.0.0.1:")
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .and_then(|port| port.parse::<u16>().ok())
            == Some(80)
    };
    if endpoint.scheme() != "http"
        || endpoint.host_str() != Some("127.0.0.1")
        || (endpoint.port().is_none() && !explicit_default_port())
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err(invalid());
    }
    Ok(Some(endpoint))
}

#[cfg(debug_assertions)]
fn web_page_fixture_matches(fixture: &Url, candidate: &Url) -> bool {
    candidate.scheme() == fixture.scheme()
        && candidate.host_str() == fixture.host_str()
        && candidate.port_or_known_default() == fixture.port_or_known_default()
        && candidate.username().is_empty()
        && candidate.password().is_none()
        && candidate.as_str().len() <= 4096
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebPageContent {
    title: String,
    url: String,
    text: String,
    fetched_at: String,
    saved_node_id: Option<String>,
    pub saved_version: Option<i64>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchProvider {
    #[default]
    News,
    Brave,
}

fn public_page_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
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
        IpAddr::V6(address) => {
            let [a, b, ..] = address.segments();
            (0x2000..0x4000).contains(&a)
                && !matches!(
                    (a, b),
                    (0x2001, 0..=0x1ff) | (0x2001, 0xdb8) | (0x2002, _) | (0x3fff, _)
                )
        }
    }
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
        Ok(address) => public_page_ip(address),
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

struct PublicPageResolver;

impl Resolve for PublicPageResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addresses = tokio::task::spawn_blocking(move || {
                (name.as_str(), 0)
                    .to_socket_addrs()
                    .map(|addresses| addresses.collect::<Vec<_>>())
            })
            .await??;

            if addresses.is_empty()
                || addresses
                    .iter()
                    .any(|address| !public_page_ip(address.ip()))
            {
                return Err("网页域名必须指向公开网络地址。".into());
            }
            // The connector dials these same addresses and applies the URL's port.
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
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

#[tauri::command]
pub(crate) async fn fetch_web_page(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    _search: State<'_, WebSearch>,
    workspace_id: String,
    url: String,
    save_to_knowledge: Option<bool>,
) -> AppResult<WebPageContent> {
    let url = url.trim();
    let parsed = result_url(url);
    #[cfg(debug_assertions)]
    let fixture_url = if parsed.is_none() && url.len() <= 4096 {
        _search
            .page_fixture
            .as_ref()
            .map_err(Clone::clone)?
            .as_ref()
            .and_then(|fixture| {
                Url::parse(url)
                    .ok()
                    .filter(|candidate| web_page_fixture_matches(fixture, candidate))
            })
    } else {
        None
    };
    #[cfg(debug_assertions)]
    let parsed = parsed.or_else(|| fixture_url.clone());
    let parsed = parsed
        .ok_or_else(|| "仅支持公开的 http/https 网页地址（已拦截内网或本机地址）。".to_string())?;
    let ws = workspace_id.clone();
    blocking(state.database.clone()?, move |db| {
        require_workspace(&*db.connection()?, &ws)
    })
    .await?;

    let builder = model_client(Duration::from_secs(6))
        .timeout(Duration::from_secs(15))
        .no_proxy();
    #[cfg(debug_assertions)]
    let builder = if fixture_url.is_some() {
        builder
    } else {
        builder.dns_resolver(PublicPageResolver)
    };
    #[cfg(not(debug_assertions))]
    let builder = builder.dns_resolver(PublicPageResolver);
    let allowed = move |candidate: &Url| {
        #[cfg(debug_assertions)]
        if let Some(fixture) = &fixture_url {
            return web_page_fixture_matches(fixture, candidate);
        }
        result_url(candidate.as_str()).is_some()
    };
    let redirect_allowed = allowed.clone();
    let client = builder
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("网页重定向次数过多。");
            }
            if !redirect_allowed(attempt.url()) {
                return attempt.error("重定向目标地址不受信任。");
            }
            attempt.follow()
        }))
        .build()
        .map_err(|_| "无法初始化网页抓取连接。")?;

    let mut response = client
        .get(parsed.clone())
        .header(
            "User-Agent",
            "CortexOS/0.2 (+https://github.com/marksimZ/CortexOS)",
        )
        .header(
            "Accept",
            "text/html, application/xhtml+xml, text/plain;q=0.9, */*;q=0.1",
        )
        .send()
        .await
        .map_err(|_| "网页连接失败或超时，请检查网络后重试。".to_string())?;

    let final_url = response.url().clone();
    if !allowed(&final_url) {
        return Err("网页最终跳转地址不受信任，已拦截。".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "网页读取失败（HTTP {}）。",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PAGE_BYTES as u64)
    {
        return Err("网页体积超过 1 MiB 限制，已停止读取。".into());
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.is_empty()
        && !content_type.contains("text/")
        && !content_type.contains("html")
        && !content_type.contains("xml")
        && !content_type.contains("json")
    {
        return Err("目标链接不是文本或网页内容，无法提取正文。".into());
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "网页读取中断或超时。".to_string())?
    {
        if body.len().saturating_add(chunk.len()) > MAX_PAGE_BYTES {
            let take = MAX_PAGE_BYTES.saturating_sub(body.len());
            body.extend_from_slice(&chunk[..take]);
            break;
        }
        body.extend_from_slice(&chunk);
    }

    let raw = String::from_utf8_lossy(&body);
    let (title, text) = extract_web_text(&raw, &content_type, &final_url);
    if text.trim().is_empty() {
        return Err("未能从该网页提取到可读正文内容（可能需要登录或纯脚本渲染）。".into());
    }

    let fetched_at = now();
    let final_url_str = final_url.to_string();
    let (saved_node_id, saved_version) = if save_to_knowledge.unwrap_or(false) {
        let workspace = workspace_id.clone();
        let host = final_url.host_str().unwrap_or("网页").to_string();
        let (saved_url, saved_title, saved_text, saved_at) = (
            final_url_str.clone(),
            title.clone(),
            text.clone(),
            fetched_at.clone(),
        );
        let (node_id, version) = blocking(state.database.clone()?, move |db| {
            save_web_page_knowledge(
                db,
                &workspace,
                &saved_url,
                &host,
                &saved_title,
                &saved_text,
                &saved_at,
            )
        })
        .await?;
        spawn_index_maintenance(&app, &state, workspace_id);
        (Some(node_id), Some(version))
    } else {
        (None, None)
    };

    Ok(WebPageContent {
        title,
        url: final_url_str,
        text,
        fetched_at,
        saved_node_id,
        saved_version,
    })
}

fn save_web_page_knowledge(
    db: &crate::db::Database,
    workspace_id: &str,
    final_url_str: &str,
    host: &str,
    title: &str,
    text: &str,
    fetched_at: &str,
) -> AppResult<(String, i64)> {
    let node_title: String = title.chars().filter(|c| !c.is_control()).take(60).collect();
    let node_title = match node_title.trim() {
        "" => format!("网页资料：{host}"),
        title => title.to_owned(),
    };
    let header = format!("来源网页：{final_url_str}\n抓取时间：{fetched_at}\n\n");
    let max_body = 19_500usize.saturating_sub(header.chars().count());
    let clipped_body: String = text.chars().take(max_body).collect();
    let node_id = db.save_knowledge(crate::knowledge::SaveKnowledge {
        workspace_id: workspace_id.to_owned(),
        node_id: None,
        expected_revision: None,
        title: node_title,
        kind: "evidence".into(),
        content: format!("{header}{clipped_body}"),
        change_reason: format!("抓取网页正文：{host}"),
        source_message_id: None,
        source_excerpt: None,
    })?;
    let version = db
        .connection()?
        .query_row(
            "SELECT COALESCE(effective_version, latest_version, 1)
             FROM knowledge_nodes WHERE workspace_id = ?1 AND id = ?2",
            rusqlite::params![workspace_id, node_id],
            |row| row.get(0),
        )
        .map_err(crate::db::error)?;
    Ok((node_id, version))
}

fn extract_web_text(raw: &str, content_type: &str, url: &Url) -> (String, String) {
    let fallback_title = url.host_str().unwrap_or("网页资料").to_string();
    if content_type.contains("text/plain")
        || content_type.contains("text/markdown")
        || content_type.contains("application/json")
    {
        let cleaned = normalize_extracted_text(raw);
        return (fallback_title, cleaned);
    }

    let title = extract_html_title(raw)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_title);
    let stripped = strip_html_to_text(raw);
    let cleaned = normalize_extracted_text(&stripped);
    (title, cleaned)
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let after_open = html[start..].find('>')? + start + 1;
    let end = lower[after_open..].find("</title>")? + after_open;
    let raw_title = &html[after_open..end];
    let decoded = decode_html_entities(raw_title);
    let collapsed: String = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(160).collect())
    }
}

fn strip_html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(64 * 1024));
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut skip_tag: Option<&'static str> = None;

    while i < bytes.len() {
        if let Some(tag) = skip_tag {
            if bytes[i] == b'<' {
                let rest = &html[i..];
                if let Some(close_idx) = rest.find('>') {
                    let inner = rest[1..close_idx].trim();
                    if let Some(end_name) = inner.strip_prefix('/') {
                        let end_name = end_name
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        if end_name == tag {
                            skip_tag = None;
                        }
                    }
                    i += close_idx + 1;
                    continue;
                }
            }
            i += 1;
            continue;
        }

        if bytes[i] == b'<' {
            let rest = &html[i..];
            if rest.starts_with("<!--") {
                if let Some(end_comment) = rest.find("-->") {
                    i += end_comment + 3;
                    continue;
                } else {
                    break;
                }
            }
            if let Some(close_idx) = rest.find('>') {
                let tag_body = rest[1..close_idx].trim();
                let is_closing = tag_body.starts_with('/');
                let name_raw = tag_body
                    .trim_start_matches('/')
                    .split(|c: char| c.is_ascii_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !is_closing {
                    match name_raw.as_str() {
                        "script" => skip_tag = Some("script"),
                        "style" => skip_tag = Some("style"),
                        "noscript" => skip_tag = Some("noscript"),
                        "svg" => skip_tag = Some("svg"),
                        "template" => skip_tag = Some("template"),
                        "br" | "hr" | "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4"
                        | "h5" | "h6" | "section" | "article" | "blockquote" | "pre" => {
                            out.push('\n');
                        }
                        _ => {}
                    }
                } else if matches!(
                    name_raw.as_str(),
                    "p" | "div"
                        | "li"
                        | "tr"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "section"
                        | "article"
                        | "blockquote"
                        | "pre"
                ) {
                    out.push('\n');
                }
                i += close_idx + 1;
                continue;
            }
        }

        if let Some(ch) = html[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    decode_html_entities(&out)
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == '&' {
            let rest = &input[idx + 1..];
            if let Some(semi) = rest.find(';') {
                if semi > 0 && semi <= 12 {
                    let entity = &rest[..semi];
                    let decoded = match entity {
                        "amp" => Some("&".to_string()),
                        "lt" => Some("<".to_string()),
                        "gt" => Some(">".to_string()),
                        "quot" => Some("\"".to_string()),
                        "apos" | "#39" | "#x27" | "#X27" => Some("'".to_string()),
                        "nbsp" => Some(" ".to_string()),
                        "mdash" => Some("—".to_string()),
                        "ndash" => Some("–".to_string()),
                        "hellip" => Some("…".to_string()),
                        _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                            u32::from_str_radix(&entity[2..], 16)
                                .ok()
                                .and_then(char::from_u32)
                                .map(|c| c.to_string())
                        }
                        _ if entity.starts_with('#') => entity[1..]
                            .parse::<u32>()
                            .ok()
                            .and_then(char::from_u32)
                            .map(|c| c.to_string()),
                        _ => None,
                    };
                    if let Some(replacement) = decoded {
                        out.push_str(&replacement);
                        let target = idx + 1 + semi + 1;
                        while let Some(&(next_idx, _)) = chars.peek() {
                            if next_idx < target {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        continue;
                    }
                }
            }
        }
        out.push(ch);
    }
    out
}

fn normalize_extracted_text(input: &str) -> String {
    let mut lines = Vec::new();
    let mut blank = false;
    for raw_line in input.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let collapsed: String = raw_line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            if !blank && !lines.is_empty() {
                lines.push(String::new());
                blank = true;
            }
        } else {
            lines.push(collapsed);
            blank = false;
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
        .join("\n")
        .chars()
        .take(MAX_PAGE_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_page_import_rejects_local_and_private_urls() {
        for url in [
            "http://127.0.0.1/a",
            "http://localhost:8080/",
            "http://192.168.1.10/",
            "http://10.0.0.1/",
            "file:///c:/test.html",
        ] {
            assert!(result_url(url).is_none(), "{url}");
        }
        assert!(result_url("https://example.com/article").is_some());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn web_page_import_fixture_matches_only_configured_origin() {
        let fixture = Url::parse("http://127.0.0.1:8080/start").unwrap();
        let matches =
            |candidate| web_page_fixture_matches(&fixture, &Url::parse(candidate).unwrap());
        assert!(matches("http://127.0.0.1:8080/redirected"));
        for candidate in [
            "https://127.0.0.1:8080/",
            "http://127.0.0.1:8081/",
            "http://localhost:8080/",
            "http://user@127.0.0.1:8080/",
            "blob:http://127.0.0.1:8080/opaque",
        ] {
            assert!(!matches(candidate), "{candidate}");
        }
    }

    #[test]
    fn public_page_addresses_exclude_special_use_ranges() {
        for address in [
            "0.0.0.0",
            "100.64.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::ffff:192.168.1.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
        ] {
            assert!(!public_page_ip(address.parse().unwrap()), "{address}");
        }
        for address in ["93.184.215.14", "2606:4700::1111"] {
            assert!(public_page_ip(address.parse().unwrap()), "{address}");
        }
    }

    #[test]
    fn web_page_import_extracts_title_and_body_without_scripts() {
        let html = r#"<!doctype html>
        <html>
          <head>
            <title>CortexOS &amp; 个人知识库</title>
            <style>body { color: red; }</style>
            <script>console.log("ignore me");</script>
          </head>
          <body>
            <!-- comment to ignore -->
            <h1>核心设计</h1>
            <p>第一段内容 &nbsp; 包含 &lt;标签&gt;。</p>
            <p>第二段内容。 English text.</p>
          </body>
        </html>"#;
        let url = Url::parse("https://example.com/guide").unwrap();
        let (title, text) = extract_web_text(html, "text/html; charset=utf-8", &url);
        assert_eq!(title, "CortexOS & 个人知识库");
        assert!(text.contains("核心设计"));
        assert!(text.contains("第一段内容 包含 <标签>。"));
        assert!(text.contains("第二段内容。 English text."));
        assert!(!text.contains("ignore me"));
        assert!(!text.contains("color: red"));
        assert!(!text.contains("comment to ignore"));
    }

    #[test]
    fn web_page_import_persists_evidence_node_and_version() {
        let directory =
            std::env::temp_dir().join(format!("cortexos-web-page-{}", uuid::Uuid::new_v4()));
        let db = crate::db::Database::open(&directory).unwrap();
        let workspace = db.create_workspace("Web import").unwrap();
        let url = Url::parse("https://example.com/article").unwrap();
        let (title, text) = extract_web_text(
            "<title>网页资料</title><p>可读正文</p><script>discardMe()</script>",
            "text/html",
            &url,
        );
        let fetched_at = "2026-11-26T00:00:00Z";
        let (node_id, version) = save_web_page_knowledge(
            &db,
            &workspace.id,
            url.as_str(),
            url.host_str().unwrap(),
            &title,
            &text,
            fetched_at,
        )
        .unwrap();
        let detail = db.knowledge_detail(&workspace.id, &node_id).unwrap();
        drop(db);
        std::fs::remove_dir_all(directory).unwrap();

        assert_eq!(version, 1);
        assert_eq!(detail.node.effective_version, Some(1));
        assert_eq!(detail.node.kind, "evidence");
        assert_eq!(detail.node.title, "网页资料");
        assert_eq!(detail.versions.len(), 1);
        assert_eq!(detail.versions[0].version, version);
        assert_eq!(
            detail.versions[0].content,
            format!("来源网页：{url}\n抓取时间：{fetched_at}\n\n{text}")
        );
        assert!(detail.versions[0].content.contains("可读正文"));
        assert!(!detail.versions[0].content.contains("discardMe"));
    }
}
