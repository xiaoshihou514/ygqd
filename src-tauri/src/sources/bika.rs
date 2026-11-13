use super::shared::{dedup_in_order, http_status_error, snippet};
use crate::models::*;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest::{Method, StatusCode};
use serde::de::{DeserializeOwned, IgnoredAny};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::net::{IpAddr, SocketAddr};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BASE: &str = "https://picaapi.picacomic.com";
const HOST: &str = "picaapi.picacomic.com";
/// 用户可见错误里的站点名
const SITE: &str = "哔咔";
/// 建连超时:分流 IP 不通时尽快失败,提示用户切换分流
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 单次读空闲超时(客户端级,同时约束经此客户端下载的大图)
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// API 请求整体超时(JSON 接口,不含图片下载)
const API_TIMEOUT: Duration = Duration::from_secs(30);
/// 章节列表最多翻页数(每页 40 章),防服务端分页异常时无限翻页
const MAX_EPISODE_PAGES: i32 = 100;
/// 登录影子响应(200 无 data/空 token)的提示
const LOGIN_REJECTED: &str =
    "哔咔拒绝了本次登录（风控/防爆破），请切换网络（Wi-Fi 与流量互换）后重试，或稍后再试";
/// 哔咔官方分流地址列表端点(返回 Cloudflare 边缘 IP,用于绕过
/// DNS 污染与部分节点的风控,参考 pikapika 的"分流地址"设置)
const ADDRESS_INIT: &str = "http://68.183.234.72/init";
const API_KEY: &str = "C69BAF41DA5ABD1FFEDC6D2FEA56B";
const SECRET: &str = "~d}$Q7$eIni=V)9\\RK/P.RM4;9[7|@/CA}b~OW!3?EV`:<>M7pddUBL5n|0/*Cn";

const HEX: &[u8; 16] = b"0123456789abcdef";

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// PicaAPI 请求签名:`path(含 query) + time + nonce + METHOD + api-key`
/// 直接拼接后整体转小写,再以固定密钥做 HMAC-SHA256(小写 hex)
pub fn signature(path_and_query: &str, time: &str, nonce: &str, method: &str) -> String {
    // 密钥固定:预先完成 key schedule,每次签名只克隆状态
    static SIGNER: LazyLock<Hmac<Sha256>> = LazyLock::new(|| {
        Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).expect("static hmac key")
    });
    let raw = format!("{path_and_query}{time}{nonce}{method}{API_KEY}").to_lowercase();
    let mut mac = SIGNER.clone();
    mac.update(raw.as_bytes());
    hex_lower(&mac.finalize().into_bytes())
}

fn now_secs() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn nonce() -> String {
    // 官方客户端使用 UUID v1(时间戳序,去掉连字符的 32 位 hex)。
    // 手工构造 v1 布局以免引入 uuid crate 的 v1 特性依赖;
    // 服务器只校验签名与 nonce 头一致,不解析其语义
    let ticks = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
        / 100
        + 0x01B2_1DD2_1381_4000; // 100ns 计数,含 1582-10-15 起始偏移
    const NODE: [u8; 6] = [0x6b, 0x69, 0x61, 0x6b, 0x61, 0x61]; // "bikaaa"
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&(ticks as u32).to_be_bytes()); // time_low
    bytes.extend_from_slice(&((ticks >> 32) as u16).to_be_bytes()); // time_mid
    bytes.extend_from_slice(&(((ticks >> 48) as u16) | 0x1000).to_be_bytes()); // time_hi + version 1
    bytes.extend_from_slice(&[0xb3, 0x2a]); // clock_seq(variant 10x)
    bytes.extend_from_slice(&NODE);
    hex_lower(&bytes)
}

// ---- PicaAPI 响应 DTO ----

// 注:泛型容器上的字段不要用 #[serde(default)],serde 会给 T 追加 Default 约束
#[derive(Deserialize)]
struct Envelope<T> {
    code: i64,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

/// 内部错误分类;对外(IPC)经 `From<ApiError> for String` 统一转为简短中文。
///
/// 契约:`Unauthorized` 转成的字符串以 "401" 开头——lib.rs 的 `bika_call`
/// 据此自动重登并重试一次;其余错误不得以 "401" 开头。
#[derive(Debug)]
enum ApiError {
    /// token 失效/未授权(HTTP 401 或 envelope.code 401)
    Unauthorized,
    /// 成功响应却无 data(登录风控的影子响应即此形态)
    MissingData,
    /// 传输层失败(DNS/连接/超时/读响应体)
    Network(reqwest::Error),
    /// 响应体不是预期 JSON(Cloudflare 拦截页、接口结构变更等)
    Decode { status: StatusCode },
    /// 业务失败:HTTP 非 2xx 或 envelope.code != 200
    Api { status: StatusCode, message: String },
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(err)
    }
}

impl From<ApiError> for String {
    fn from(err: ApiError) -> String {
        match err {
            ApiError::Unauthorized => "401 哔咔登录已失效,请重新登录".into(),
            ApiError::MissingData => "哔咔返回了空数据,请稍后重试".into(),
            ApiError::Network(e) => network_error(e),
            ApiError::Decode { status } if status.is_success() => {
                "哔咔返回了无法识别的数据,请稍后重试".into()
            }
            ApiError::Decode { status } => http_status_error(SITE, status),
            ApiError::Api { status, message } => api_message(status, &message),
        }
    }
}

/// 传输层错误:复用全局 `crate::reqwest_error` 文案并冠以站点名;
/// 超时/连不上时追加分流提示(国内最常见原因是线路/DNS 污染)。完整错误链写 stderr
fn network_error(e: reqwest::Error) -> String {
    eprintln!("[bika] request error: {e:?}");
    let hint = e.is_timeout() || e.is_connect();
    let msg = format!("{SITE}{}", crate::reqwest_error(e));
    if hint {
        format!("{msg},或在设置中切换分流")
    } else {
        msg
    }
}

/// 业务错误文案:常见英文 message 译为中文,未知 message 原样附带便于反馈
fn api_message(status: StatusCode, message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("too many requests") {
        "哔咔请求过于频繁,请稍后再试".into()
    } else if lower.contains("invalid email or password") {
        "哔咔账号或密码错误".into()
    } else if lower.contains("under review") {
        "该漫画正在审核中,暂时无法查看".into()
    } else if lower.contains("not found") {
        "哔咔内容不存在或已被删除".into()
    } else if message.is_empty() {
        http_status_error(SITE, status)
    } else {
        format!("哔咔请求失败:{message}(HTTP {})", status.as_u16())
    }
}

/// 解析 PicaAPI 响应信封(纯函数,便于单测)。
/// 401 先于 JSON 解码判定:401 响应体不一定是合法信封
fn parse_envelope<T: DeserializeOwned>(
    status: StatusCode,
    text: &str,
) -> Result<Option<T>, ApiError> {
    if status == StatusCode::UNAUTHORIZED {
        return Err(ApiError::Unauthorized);
    }
    let envelope: Envelope<T> = serde_json::from_str(text).map_err(|e| {
        eprintln!(
            "[bika] decode error ({status}): {e}; body: {}",
            snippet(text, 200)
        );
        ApiError::Decode { status }
    })?;
    if envelope.code == 401 {
        return Err(ApiError::Unauthorized);
    }
    if !status.is_success() || envelope.code != 200 {
        eprintln!(
            "[bika] api error: http {status}, code {}, message {:?}",
            envelope.code, envelope.message
        );
        return Err(ApiError::Api {
            status,
            message: envelope.message,
        });
    }
    Ok(envelope.data)
}

/// 分流地址 → TCP 目标:支持 "IP"、"IP:PORT"、"[IPv6]:PORT",缺端口补 443
fn parse_address(address: &str) -> Result<SocketAddr, String> {
    let address = address.trim();
    address
        .parse::<SocketAddr>()
        .or_else(|_| address.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, 443)))
        .map_err(|_| format!("分流地址格式无效:{address}(应为 IP 或 IP:端口)"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page<T> {
    docs: Vec<T>,
    page: i64,
    pages: i64,
}

// 手动实现以避免 derive 给 T 追加 Default 约束(Vec<T> 的 Default 不需要 T: Default)
impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            docs: Vec::new(),
            page: 1,
            pages: 1,
        }
    }
}

impl<T> Page<T> {
    /// PicaAPI 分页为 1-based,统一转换为前端使用的 0-based current
    fn pagination(&self) -> PaginationInfo {
        let page = self.page.max(1);
        let pages = self.pages.max(1);
        PaginationInfo {
            current: (page - 1) as i32,
            total: pages as i32,
            has_next: page < pages,
            has_prev: page > 1,
        }
    }
}

impl Page<ComicDoc> {
    /// 漫画分页 → 前端列表结果
    fn into_list(self) -> CategoryListResult {
        CategoryListResult {
            pagination: self.pagination(),
            items: self.docs.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RemoteImage {
    #[serde(default)]
    file_server: String,
    #[serde(default)]
    path: String,
}

impl RemoteImage {
    fn url(&self) -> String {
        // 图片需加 /static/ 前缀;path 可能带 tobs 等存储桶段,服务器会 301 归一化
        format!(
            "{}/static/{}",
            self.file_server.trim_end_matches('/'),
            self.path.trim_start_matches('/')
        )
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ComicDoc {
    #[serde(rename = "_id", default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    thumb: Option<RemoteImage>,
    #[serde(default)]
    likes_count: i64,
    #[serde(default)]
    is_liked: bool,
    #[serde(default)]
    is_favourite: bool,
}

impl From<ComicDoc> for ComicItem {
    fn from(doc: ComicDoc) -> Self {
        ComicItem {
            source: Source::Bika,
            category: doc.categories.first().cloned().unwrap_or_default(),
            thumbnail: doc.thumb.map(|thumb| thumb.url()).unwrap_or_default(),
            id: doc.id,
            title: doc.title,
            author: doc.author,
            category_id: 0,
            tags: doc.categories,
            likes: doc.likes_count.to_string(),
            link: String::new(),
        }
    }
}

impl From<ComicDoc> for ComicDetail {
    fn from(doc: ComicDoc) -> Self {
        let category = doc.categories.first().cloned().unwrap_or_default();
        // 分类在前、标签在后,合并去重
        let tags = dedup_in_order(doc.categories.into_iter().chain(doc.tags));
        ComicDetail {
            source: Source::Bika,
            category,
            thumbnail: doc.thumb.map(|thumb| thumb.url()).unwrap_or_default(),
            id: doc.id,
            title: doc.title,
            category_id: 0,
            author: doc.author,
            works: vec![],
            characters: vec![],
            tags,
            likes: doc.likes_count.to_string(),
            episodes: vec![],
            images: vec![],
            is_liked: doc.is_liked,
            is_favourite: doc.is_favourite,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeDoc {
    #[serde(default)]
    order: i64,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PictureDoc {
    #[serde(default)]
    media: Option<RemoteImage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoryDoc {
    #[serde(default)]
    title: String,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    is_web: bool,
}

// PicaAPI 的 data 字段是包一层的对象(如 data.categories / data.comics / data.eps),
// 按接口分别定义包装结构

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoriesDoc {
    #[serde(default)]
    categories: Vec<CategoryDoc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComicsPageDoc {
    #[serde(default)]
    comics: Page<ComicDoc>,
}

/// 排行榜的 data.comics 为数组(无分页)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaderboardDoc {
    #[serde(default)]
    comics: Vec<ComicDoc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpsPageDoc {
    #[serde(default)]
    eps: Page<EpisodeDoc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PicturesPageDoc {
    #[serde(default)]
    pages: Page<PictureDoc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComicDetailDoc {
    #[serde(default)]
    comic: ComicDoc,
}

#[derive(Deserialize)]
struct TokenDoc {
    #[serde(default)]
    token: String,
}

#[derive(Clone)]
pub struct BikaApi {
    pub client: reqwest::Client,
}

fn encode_query(pairs: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        // 空值参数(如 c=&t=&a=)会被服务端以 validation error 拒绝,跳过
        if !value.is_empty() {
            serializer.append_pair(key, value);
        }
    }
    serializer.finish()
}

impl BikaApi {
    /// `address` 为分流地址("IP" 或 "IP:PORT"),None 时用系统 DNS。
    /// 分流原理:SNI 仍是 picaapi.picacomic.com,只替换 TCP 连接的边缘 IP
    pub fn new(address: Option<&str>) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert("api-key", HeaderValue::from_static(API_KEY));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.picacomic.com.v1+json"),
        );
        headers.insert("app-channel", HeaderValue::from_static("3"));
        headers.insert("app-version", HeaderValue::from_static("2.2.1.2.3.3"));
        headers.insert("app-uuid", HeaderValue::from_static("defaultUuid"));
        headers.insert("app-platform", HeaderValue::from_static("android"));
        headers.insert("app-build-version", HeaderValue::from_static("44"));
        headers.insert("image-quality", HeaderValue::from_static("original"));
        headers.insert("version", HeaderValue::from_static("v1.4.1"));
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=UTF-8"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("okhttp/3.8.1"));
        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT);
        if let Some(address) = address {
            // resolve 只影响 TCP 层目标,TLS 的 SNI 仍为域名
            builder = builder.resolve(HOST, parse_address(address)?);
        }
        let client = builder.cookie_store(true).build().map_err(|e| {
            eprintln!("[bika] client build failed: {e:?}");
            "哔咔客户端初始化失败".to_string()
        })?;
        Ok(Self { client })
    }

    /// 从哔咔官方 init 端点同步可用分流地址(裸 IP,不带端口)
    pub async fn fetch_addresses(&self) -> Result<Vec<String>, String> {
        #[derive(Deserialize)]
        struct InitDoc {
            #[serde(default)]
            addresses: Vec<String>,
        }
        let fail = |e: reqwest::Error| format!("获取分流地址失败:{}", network_error(e));
        let doc: InitDoc = self
            .client
            .get(ADDRESS_INIT)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(fail)?
            .json()
            .await
            .map_err(fail)?;
        Ok(doc
            .addresses
            .into_iter()
            .filter(|addr| !addr.is_empty())
            .collect())
    }

    /// 发送签名请求并返回 (HTTP 状态, 响应体文本)
    async fn send(
        &self,
        method: Method,
        path: &str,
        query: Option<String>,
        body: Option<Value>,
        token: Option<&str>,
    ) -> Result<(StatusCode, String), ApiError> {
        // 签名的路径不带前导斜杠(实测:带斜杠的签名会被服务端以
        // 200 假 success 静默吞掉;与 PicaComic url.replaceAll(apiUrl+"/","") 行为一致)
        let mut path_and_query = path.strip_prefix('/').unwrap_or(path).to_owned();
        if let Some(query) = query.as_deref().filter(|q| !q.is_empty()) {
            path_and_query.push('?');
            path_and_query.push_str(query);
        }
        let url = format!("{BASE}/{path_and_query}");
        let time = now_secs();
        let nonce = nonce();
        let signature = signature(&path_and_query, &time, &nonce, method.as_str());
        let mut request = self
            .client
            .request(method, url)
            .timeout(API_TIMEOUT)
            .header("time", time)
            .header("nonce", nonce)
            .header("signature", signature);
        if let Some(token) = token {
            // 官方客户端 authorization 为裸 JWT,不带 "Bearer " 前缀;
            // 带 Bearer 会被哔咔当作无效凭据返回 401
            request = request.header("authorization", token);
        }
        if let Some(body) = body {
            // 不能用 .json():它会把 Content-Type 覆盖为不带 charset 的版本,
            // 哔咔风控会以 "too many requests" 拒绝(实测,见对照测试)
            request = request
                .header(CONTENT_TYPE, "application/json; charset=UTF-8")
                .body(body.to_string());
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        Ok((status, text))
    }

    /// 签名请求 + 信封解析,要求 data 存在。
    /// 401 归为 `ApiError::Unauthorized`(转字符串后以 "401" 开头,见 ApiError)
    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<String>,
        body: Option<Value>,
        token: Option<&str>,
    ) -> Result<T, ApiError> {
        let (status, text) = self.send(method, path, query, body, token).await?;
        parse_envelope::<T>(status, &text)?.ok_or(ApiError::MissingData)
    }

    /// toggle 类端点(点赞/收藏):code 200 即成功,不要求 data 存在
    async fn request_toggle(&self, path: &str, token: &str) -> Result<(), String> {
        let (status, text) = self
            .send(Method::POST, path, None, Some(json!({})), Some(token))
            .await?;
        parse_envelope::<IgnoredAny>(status, &text)?;
        Ok(())
    }

    /// 带 token 的 GET 并解析 data
    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<String>,
        token: &str,
    ) -> Result<T, String> {
        Ok(self
            .request(Method::GET, path, query, None, Some(token))
            .await?)
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<String, String> {
        let result = self
            .request::<TokenDoc>(
                Method::POST,
                "/auth/sign-in",
                None,
                Some(json!({ "email": email, "password": password })),
                None,
            )
            .await;
        match result {
            Ok(data) if !data.token.is_empty() => Ok(data.token),
            // 登录接口返回 200 但不带 data/空 token:哔咔在风控/防爆破时返回这种
            // 影子响应,错误密码同样如此,无法区分具体原因;社区经验是切换网络后重试
            Ok(_) | Err(ApiError::MissingData) => Err(LOGIN_REJECTED.into()),
            // 登录阶段的 401 是凭据被拒,不能以 "401" 开头(否则会被当作 token 失效)
            Err(ApiError::Unauthorized) => Err("哔咔登录被拒绝,请检查邮箱和密码".into()),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn categories(&self, token: String) -> Result<Vec<String>, String> {
        let docs: CategoriesDoc = self.get("/categories", None, &token).await?;
        Ok(docs
            .categories
            .into_iter()
            .filter(|doc| doc.active && !doc.is_web && !doc.title.is_empty())
            .map(|doc| doc.title)
            .collect())
    }

    /// 排行榜,tt 为 "H24"/"D7"/"D30"(过去 24 小时/7 天/30 天);
    /// 参考 pikapika 的 RankingsScreen,ct 固定 "VC"(按观看数),无分页
    pub async fn leaderboard(&self, token: String, tt: &str) -> Result<CategoryListResult, String> {
        let query = encode_query(&[("tt", tt), ("ct", "VC")]);
        let doc: LeaderboardDoc = self.get("/comics/leaderboard", Some(query), &token).await?;
        Ok(CategoryListResult {
            items: doc.comics.into_iter().map(Into::into).collect(),
            pagination: PaginationInfo {
                current: 0,
                total: 1,
                has_next: false,
                has_prev: false,
            },
        })
    }

    /// 点赞/取消点赞(toggle,服务器端按当前状态翻转)
    pub async fn switch_like(&self, token: String, id: &str) -> Result<(), String> {
        self.request_toggle(&format!("/comics/{id}/like"), &token)
            .await
    }

    /// 收藏/取消收藏(toggle)
    pub async fn switch_favourite(&self, token: String, id: &str) -> Result<(), String> {
        self.request_toggle(&format!("/comics/{id}/favourite"), &token)
            .await
    }

    /// 服务器端收藏列表(用于同步到本地),page 为 1-based
    pub async fn favourite_list(
        &self,
        token: String,
        page: i32,
    ) -> Result<CategoryListResult, String> {
        let query = encode_query(&[("s", "dd"), ("page", &page.to_string())]);
        let doc: ComicsPageDoc = self.get("/users/favourite", Some(query), &token).await?;
        Ok(doc.comics.into_list())
    }

    /// 分类/标签/作者浏览,page 为 1-based
    pub async fn comics(
        &self,
        token: String,
        category: &str,
        tag: &str,
        author: &str,
        page: i32,
    ) -> Result<CategoryListResult, String> {
        let query = encode_query(&[
            ("c", category),
            ("t", tag),
            ("a", author),
            ("s", "dd"),
            ("page", &page.to_string()),
        ]);
        let doc: ComicsPageDoc = self.get("/comics", Some(query), &token).await?;
        Ok(doc.comics.into_list())
    }

    pub async fn search(
        &self,
        token: String,
        keyword: &str,
        page: i32,
    ) -> Result<CategoryListResult, String> {
        let query = encode_query(&[("page", &page.to_string())]);
        let doc: ComicsPageDoc = self
            .request(
                Method::POST,
                // 旧的 /comics/search 已下线(Method Not Allowed),改用 advanced-search
                "/comics/advanced-search",
                Some(query),
                Some(json!({ "keyword": keyword, "sort": "dd" })),
                Some(token.as_str()),
            )
            .await?;
        Ok(doc.comics.into_list())
    }

    pub async fn detail(&self, token: String, id: &str) -> Result<ComicDetail, String> {
        let doc: ComicDetailDoc = self.get(&format!("/comics/{id}"), None, &token).await?;
        Ok(doc.comic.into())
    }

    /// 循环拉取全部章节,order 升序
    pub async fn episodes(&self, token: String, id: &str) -> Result<Vec<Episode>, String> {
        let path = format!("/comics/{id}/eps");
        let mut result = Vec::new();
        let mut page = 1;
        loop {
            let query = encode_query(&[("page", &page.to_string())]);
            let eps = self
                .get::<EpsPageDoc>(&path, Some(query), &token)
                .await?
                .eps;
            let pages = eps.pages;
            let empty = eps.docs.is_empty();
            result.extend(eps.docs.into_iter().map(|doc| Episode {
                order: doc.order as i32,
                title: doc.title,
            }));
            // 以「请求的页号」对比总页数(而非响应回显的 page),并以空页与
            // 页数上限兜底:服务端忽略 page 参数或分页字段异常时不会死循环
            if empty || i64::from(page) >= pages || page >= MAX_EPISODE_PAGES {
                break;
            }
            page += 1;
        }
        result.sort_by_key(|episode| episode.order);
        Ok(result)
    }

    /// 某章节第 page 页(1-based)的图片
    pub async fn pictures(
        &self,
        token: String,
        id: &str,
        order: i32,
        page: i32,
    ) -> Result<ComicImagesResult, String> {
        let query = encode_query(&[("page", &page.to_string())]);
        let pages = self
            .get::<PicturesPageDoc>(
                &format!("/comics/{id}/order/{order}/pages"),
                Some(query),
                &token,
            )
            .await?
            .pages;
        Ok(ComicImagesResult {
            pagination: pages.pagination(),
            images: pages
                .docs
                .into_iter()
                .filter_map(|doc| doc.media.map(|media| media.url()))
                .filter(|url| !url.is_empty())
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_reference_vector() {
        // 向量由 openssl dgst -sha256 -hmac 独立计算得到
        // 注意:签名路径不带前导斜杠(服务端校验)
        let sig = signature(
            "auth/sign-in",
            "1792177753",
            "ffb366c1e5e448069bcdfbcbfb12e051",
            "POST",
        );
        assert_eq!(
            sig,
            "84d3ea336863cf5c16e08038939150f14ff0b23240886eb96777e665cbc03244"
        );
    }

    #[test]
    fn nonce_is_uuid_v1_shaped_hex() {
        let n = nonce();
        assert_eq!(n.len(), 32, "去连字符后应为 32 位 hex: {n}");
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()), "hex: {n}");
        let version = n.as_bytes()[12] - b'0';
        assert!(version <= 9, "版本位为 hex 数字");
        assert_eq!(u32::from(version), 1, "版本应为 1: {n}");
        let variant = u8::from_str_radix(&n[16..17], 16).expect("hex");
        assert!(
            (0x8..=0xB).contains(&variant),
            "变体位应为 RFC 4122 (10x): {n}"
        );
    }

    #[test]
    fn maps_comic_item_from_api_doc() {
        let doc: ComicDoc = serde_json::from_str(
            r#"{
                "_id": "5822a40b24fcb814c3ed542d",
                "title": "Example",
                "author": "Author",
                "categories": ["短篇", "全彩"],
                "thumb": { "fileServer": "https://storage.example", "path": "tobs/sub/a.jpg" },
                "likesCount": 1200
            }"#,
        )
        .unwrap();
        let item = ComicItem::from(doc);
        assert_eq!(item.id, "5822a40b24fcb814c3ed542d");
        assert_eq!(
            item.thumbnail,
            "https://storage.example/static/tobs/sub/a.jpg"
        );
        assert_eq!(item.category, "短篇");
        assert_eq!(item.tags, vec!["短篇", "全彩"]);
        assert_eq!(item.likes, "1200");
        assert_eq!(item.source, Source::Bika);
    }

    #[test]
    fn missing_fields_default_instead_of_failing() {
        let doc: ComicDoc = serde_json::from_str(r#"{ "_id": "x" }"#).unwrap();
        let item = ComicItem::from(doc);
        assert_eq!(item.title, "");
        assert!(item.tags.is_empty());
        assert_eq!(item.likes, "0");
    }

    #[test]
    fn maps_pagination_from_api_page() {
        let page: Page<ComicDoc> =
            serde_json::from_str(r#"{ "docs": [], "page": 2, "pages": 5 }"#).unwrap();
        let pagination = page.pagination();
        assert_eq!(pagination.current, 1);
        assert_eq!(pagination.total, 5);
        assert!(pagination.has_next && pagination.has_prev);
    }

    fn err_text<T: std::fmt::Debug>(result: Result<T, ApiError>) -> String {
        String::from(result.expect_err("should fail"))
    }

    #[test]
    fn envelope_ok_returns_data() {
        let data: Option<TokenDoc> = parse_envelope(
            StatusCode::OK,
            r#"{"code":200,"message":"success","data":{"token":"t"}}"#,
        )
        .unwrap();
        assert_eq!(data.unwrap().token, "t");
    }

    #[test]
    fn unauthorized_maps_to_401_prefix_even_with_non_json_body() {
        // 401 先于解码判定:HTML/空响应体也要触发自动重登
        let msg = err_text(parse_envelope::<IgnoredAny>(
            StatusCode::UNAUTHORIZED,
            "<html>401</html>",
        ));
        assert!(msg.starts_with("401"), "{msg}");
        assert!(msg.contains("登录"), "前端据此提示登录: {msg}");
        // HTTP 200 但信封 code 401 同样视为 token 失效
        let msg = err_text(parse_envelope::<IgnoredAny>(
            StatusCode::OK,
            r#"{"code":401,"message":"unauthorized"}"#,
        ));
        assert!(msg.starts_with("401"), "{msg}");
    }

    #[test]
    fn non_401_errors_never_start_with_401() {
        let cases = [
            (StatusCode::BAD_GATEWAY, "<html>bad gateway</html>"),
            (StatusCode::OK, "not json"),
            (
                StatusCode::BAD_REQUEST,
                r#"{"code":400,"message":"invalid email or password"}"#,
            ),
            (
                StatusCode::OK,
                r#"{"code":400,"message":"too many requests"}"#,
            ),
        ];
        for (status, body) in cases {
            let msg = err_text(parse_envelope::<IgnoredAny>(status, body));
            assert!(!msg.starts_with("401"), "{msg}");
            assert!(!msg.is_empty());
        }
    }

    #[test]
    fn api_errors_are_translated() {
        let msg = err_text(parse_envelope::<IgnoredAny>(
            StatusCode::BAD_REQUEST,
            r#"{"code":400,"message":"invalid email or password"}"#,
        ));
        assert_eq!(msg, "哔咔账号或密码错误");
        let msg = err_text(parse_envelope::<IgnoredAny>(
            StatusCode::SERVICE_UNAVAILABLE,
            "<html>cf</html>",
        ));
        assert_eq!(msg, "哔咔服务器暂时不可用(HTTP 503),请稍后重试");
        // 长中文响应体截断不得 panic(旧实现按字节切片会在字符中间 panic)
        let body = "错".repeat(300);
        let msg = err_text(parse_envelope::<IgnoredAny>(StatusCode::OK, &body));
        assert_eq!(msg, "哔咔返回了无法识别的数据,请稍后重试");
    }

    #[test]
    fn missing_data_is_reported() {
        let data: Option<TokenDoc> =
            parse_envelope(StatusCode::OK, r#"{"code":200,"message":"success"}"#).unwrap();
        assert!(data.is_none());
    }

    #[test]
    fn parses_address_variants() {
        assert_eq!(
            parse_address("104.21.1.1").unwrap(),
            "104.21.1.1:443".parse().unwrap()
        );
        assert_eq!(
            parse_address(" 104.21.1.1:8443 ").unwrap(),
            "104.21.1.1:8443".parse().unwrap()
        );
        assert_eq!(
            parse_address("2606:4700::1").unwrap(),
            "[2606:4700::1]:443".parse().unwrap()
        );
        assert!(parse_address("not-an-ip").is_err());
    }

    #[test]
    fn detail_merges_categories_and_tags_without_duplicates() {
        let doc: ComicDoc = serde_json::from_str(
            r#"{ "_id": "x", "categories": ["短篇", "全彩"], "tags": ["全彩", "纯爱"] }"#,
        )
        .unwrap();
        let detail = ComicDetail::from(doc);
        assert_eq!(detail.category, "短篇");
        assert_eq!(detail.tags, vec!["短篇", "全彩", "纯爱"]);
    }

    #[test]
    #[ignore = "真实 API 联调,需 BIKA_EMAIL/BIKA_PASSWORD 或 BIKA_TOKEN 环境变量"]
    fn bika_live_endpoints() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                let api = BikaApi::new(std::env::var("BIKA_ADDRESS").ok().as_deref())
                    .expect("bika client");
                let token = match std::env::var("BIKA_TOKEN") {
                    Ok(token) if !token.is_empty() => {
                        println!("using BIKA_TOKEN from env");
                        token
                    }
                    _ => {
                        let email = std::env::var("BIKA_EMAIL").expect("BIKA_EMAIL not set");
                        let password =
                            std::env::var("BIKA_PASSWORD").expect("BIKA_PASSWORD not set");
                        let token = api
                            .sign_in(&email, &password)
                            .await
                            .expect("sign-in failed");
                        println!("sign-in ok, token: {token}");
                        token
                    }
                };

                let categories = api.categories(token.clone()).await.expect("categories");
                println!("categories({}): {:?}", categories.len(), categories);
                assert!(!categories.is_empty(), "categories should not be empty");

                for tt in ["H24", "D7", "D30"] {
                    let board = api
                        .leaderboard(token.clone(), tt)
                        .await
                        .expect("leaderboard");
                    println!(
                        "leaderboard[{tt}]: {} items, top1 = {}",
                        board.items.len(),
                        board.items.first().map(|i| i.title.as_str()).unwrap_or("?")
                    );
                    assert!(
                        !board.items.is_empty(),
                        "leaderboard {tt} should not be empty"
                    );
                }

                let list = api
                    .comics(token.clone(), "", "", "", 1)
                    .await
                    .expect("comics");
                println!(
                    "comics page1: {} items, pagination current={} total={} hasNext={}",
                    list.items.len(),
                    list.pagination.current,
                    list.pagination.total,
                    list.pagination.has_next
                );
                assert!(!list.items.is_empty(), "comics should not be empty");

                if let Some(category) = categories.first() {
                    let by_category = api
                        .comics(token.clone(), category, "", "", 1)
                        .await
                        .expect("comics by category");
                    println!(
                        "comics category[{category}] page1: {} items",
                        by_category.items.len()
                    );
                }

                let search = api.search(token.clone(), "纯爱", 1).await.expect("search");
                println!("search page1: {} items", search.items.len());

                let id = list.items[0].id.clone();
                let detail = api.detail(token.clone(), &id).await.expect("detail");
                println!(
                    "detail: title={} author={} isLiked={} isFavourite={}",
                    detail.title, detail.author, detail.is_liked, detail.is_favourite
                );

                // 点赞 toggle 两次还原;收藏 toggle 两次还原(不污染账号状态)
                let liked_before = detail.is_liked;
                let fav_before = detail.is_favourite;
                api.switch_like(token.clone(), &id)
                    .await
                    .expect("switch_like");
                api.switch_favourite(token.clone(), &id)
                    .await
                    .expect("switch_favourite");
                let after = api
                    .detail(token.clone(), &id)
                    .await
                    .expect("detail after toggle");
                println!(
                    "after toggle: isLiked={} isFavourite={}",
                    after.is_liked, after.is_favourite
                );
                assert_eq!(after.is_liked, !liked_before, "点赞应翻转");
                assert_eq!(after.is_favourite, !fav_before, "收藏应翻转");
                api.switch_like(token.clone(), &id)
                    .await
                    .expect("switch_like restore");
                api.switch_favourite(token.clone(), &id)
                    .await
                    .expect("switch_favourite restore");
                let restored = api
                    .detail(token.clone(), &id)
                    .await
                    .expect("detail restored");
                assert_eq!(restored.is_liked, liked_before, "点赞应还原");
                assert_eq!(restored.is_favourite, fav_before, "收藏应还原");
                println!("like/favourite toggled and restored ok");

                let favs = api
                    .favourite_list(token.clone(), 1)
                    .await
                    .expect("favourite_list");
                println!(
                    "favourite_list page1: {} items, hasNext={}",
                    favs.items.len(),
                    favs.pagination.has_next
                );

                let episodes = api.episodes(token.clone(), &id).await.expect("episodes");
                println!(
                    "episodes({}): {:?}",
                    episodes.len(),
                    episodes
                        .iter()
                        .map(|ep| (ep.order, ep.title.clone()))
                        .collect::<Vec<_>>()
                );
                assert!(!episodes.is_empty(), "episodes should not be empty");

                let order = episodes[0].order;
                let pictures = api
                    .pictures(token.clone(), &id, order, 1)
                    .await
                    .expect("pictures");
                println!(
                    "pictures ep{order} page1: {} images, hasNext={}",
                    pictures.images.len(),
                    pictures.pagination.has_next
                );
                assert!(!pictures.images.is_empty(), "pictures should not be empty");
                println!("first image: {}", pictures.images[0]);

                // 图片 URL 需真实可下载(验证 fileServer/path 拼接)
                let image_bytes = api
                    .client
                    .get(&pictures.images[0])
                    .send()
                    .await
                    .expect("image request")
                    .error_for_status()
                    .expect("image status")
                    .bytes()
                    .await
                    .expect("image bytes");
                println!("image download: {} bytes", image_bytes.len());
                assert!(image_bytes.len() > 10_000, "image too small to be real");
            });
    }
}
