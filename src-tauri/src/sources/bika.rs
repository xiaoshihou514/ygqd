use crate::models::*;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE: &str = "https://picaapi.picacomic.com";
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
    let raw = format!("{path_and_query}{time}{nonce}{method}{API_KEY}").to_lowercase();
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).expect("static hmac key");
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page<T> {
    docs: Vec<T>,
    page: i64,
    pages: i64,
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
        format!("{}{}", self.file_server, self.path)
    }
}

#[derive(Deserialize)]
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
}

impl From<ComicDoc> for ComicItem {
    fn from(doc: ComicDoc) -> Self {
        ComicItem {
            source: "bika".into(),
            category: doc.categories.first().cloned().unwrap_or_default(),
            thumbnail: doc.thumb.map(|thumb| thumb.url()).unwrap_or_default(),
            id: doc.id,
            title: doc.title,
            category_id: 0,
            tags: doc.categories,
            likes: doc.likes_count.to_string(),
            link: String::new(),
        }
    }
}

impl From<ComicDoc> for ComicDetail {
    fn from(doc: ComicDoc) -> Self {
        let mut tags = doc.categories.clone();
        for tag in &doc.tags {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
        ComicDetail {
            source: "bika".into(),
            category: doc.categories.first().cloned().unwrap_or_default(),
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

#[derive(Deserialize)]
struct TokenDoc {
    #[serde(default)]
    token: String,
}

pub struct BikaApi {
    pub client: reqwest::Client,
}

fn encode_query(pairs: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

impl BikaApi {
    pub fn new() -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert("api-key", HeaderValue::from_static(API_KEY));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.picacomic.com.v1+json"),
        );
        headers.insert("app-channel", HeaderValue::from_static("3"));
        headers.insert("app-version", HeaderValue::from_static("2.2.1.3.3.4"));
        headers.insert("app-uuid", HeaderValue::from_static("defaultUuid"));
        headers.insert("app-platform", HeaderValue::from_static("android"));
        headers.insert("app-build-version", HeaderValue::from_static("45"));
        headers.insert("image-quality", HeaderValue::from_static("original"));
        headers.insert("version", HeaderValue::from_static("v1.4.1"));
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=UTF-8"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("okhttp/3.8.1"));
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    /// 返回 Err 以 "401" 开头表示 token 失效,调用方应重新登录后重试
    async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        query: Option<String>,
        body: Option<Value>,
        token: Option<&str>,
    ) -> Result<T, String> {
        let url = match &query {
            Some(query) => format!("{BASE}{path}?{query}"),
            None => format!("{BASE}{path}"),
        };
        let path_and_query = match &query {
            Some(query) => format!("{path}?{query}"),
            None => path.to_string(),
        };
        let time = now_secs();
        let nonce = nonce();
        let signature = signature(&path_and_query, &time, &nonce, method);
        let method = method
            .parse::<reqwest::Method>()
            .map_err(|e| e.to_string())?;
        let mut request = self
            .client
            .request(method, &url)
            .header("time", &time)
            .header("nonce", &nonce)
            .header("signature", signature);
        if let Some(token) = token {
            // 官方客户端 authorization 为裸 JWT,不带 "Bearer " 前缀;
            // 带 Bearer 会被哔咔当作无效凭据返回 401
            request = request.header("authorization", token);
        }
        if let Some(body) = body {
            // 不能用 .json():它会把 Content-Type 覆盖为不带 charset 的版本,
            // 哔咔风控会以 "too many requests" 拒绝(实测,见对照测试)
            let serialized = serde_json::to_string(&body).map_err(|e| e.to_string())?;
            request = request
                .header(CONTENT_TYPE, "application/json; charset=UTF-8")
                .body(serialized);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let envelope: Envelope<T> = response.json().await.map_err(|e| e.to_string())?;
        if status.as_u16() == 401 {
            return Err(format!("401 {}", envelope.message));
        }
        if !status.is_success() || envelope.code != 200 {
            return Err(format!("{} ({})", envelope.message, status.as_u16()));
        }
        envelope
            .data
            .ok_or_else(|| "bika response missing data".to_string())
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<String, String> {
        let data: TokenDoc = match self
            .request(
                "POST",
                "/auth/sign-in",
                None,
                Some(json!({ "email": email, "password": password })),
                None,
            )
            .await
        {
            Ok(data) => data,
            // 登录接口返回 200 但不带 data:哔咔在风控/防爆破时返回这种影子响应,
            // 错误密码同样如此,无法区分具体原因;社区经验是切换网络后重试
            Err(e) if e.contains("missing data") => {
                return Err("哔咔拒绝了本次登录（风控/防爆破），请切换网络（Wi-Fi 与流量互换）后重试，或稍后再试".into())
            }
            Err(e) => return Err(e),
        };
        if data.token.is_empty() {
            return Err("哔咔拒绝了本次登录（风控/防爆破），请切换网络（Wi-Fi 与流量互换）后重试，或稍后再试".into());
        }
        Ok(data.token)
    }

    pub async fn categories(&self, token: String) -> Result<Vec<String>, String> {
        let docs: Vec<CategoryDoc> = self
            .request("GET", "/categories", None, None, Some(token.as_str()))
            .await?;
        Ok(docs
            .into_iter()
            .filter(|doc| doc.active && !doc.is_web && !doc.title.is_empty())
            .map(|doc| doc.title)
            .collect())
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
        let page_docs: Page<ComicDoc> = self
            .request("GET", "/comics", Some(query), None, Some(token.as_str()))
            .await?;
        Ok(CategoryListResult {
            pagination: page_docs.pagination(),
            items: page_docs.docs.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn search(
        &self,
        token: String,
        keyword: &str,
        page: i32,
    ) -> Result<CategoryListResult, String> {
        let query = encode_query(&[("page", &page.to_string())]);
        let page_docs: Page<ComicDoc> = self
            .request(
                "POST",
                "/comics/search",
                Some(query),
                Some(json!({ "keyword": keyword, "sort": "dd", "categories": [] })),
                Some(token.as_str()),
            )
            .await?;
        Ok(CategoryListResult {
            pagination: page_docs.pagination(),
            items: page_docs.docs.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn detail(&self, token: String, id: &str) -> Result<ComicDetail, String> {
        self.request::<ComicDoc>(
            "GET",
            &format!("/comics/{id}"),
            None,
            None,
            Some(token.as_str()),
        )
        .await
        .map(Into::into)
    }

    /// 循环拉取全部章节,order 升序
    pub async fn episodes(&self, token: String, id: &str) -> Result<Vec<Episode>, String> {
        let mut result = Vec::new();
        let mut page = 1;
        loop {
            let query = encode_query(&[("page", &page.to_string())]);
            let page_docs: Page<EpisodeDoc> = self
                .request(
                    "GET",
                    &format!("/comics/{id}/eps"),
                    Some(query),
                    None,
                    Some(token.as_str()),
                )
                .await?;
            let has_next = page_docs.pagination().has_next;
            result.extend(page_docs.docs.into_iter().map(|doc| Episode {
                order: doc.order as i32,
                title: doc.title,
            }));
            if !has_next {
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
        let page_docs: Page<PictureDoc> = self
            .request(
                "GET",
                &format!("/comics/{id}/order/{order}/pages"),
                Some(query),
                None,
                Some(token.as_str()),
            )
            .await?;
        Ok(ComicImagesResult {
            pagination: page_docs.pagination(),
            images: page_docs
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
        let sig = signature(
            "/comics?c=短篇&page=1",
            "1792177753",
            "ffb366c1e5e448069bcdfbcbfb12e051",
            "GET",
        );
        assert_eq!(
            sig,
            "e1eba013d81de92db7ab7372b271861f2e928c893899eb727424451f8f4e1eb6"
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
                "thumb": { "fileServer": "https://storage.example", "path": "/static/a.jpg" },
                "likesCount": 1200
            }"#,
        )
        .unwrap();
        let item = ComicItem::from(doc);
        assert_eq!(item.id, "5822a40b24fcb814c3ed542d");
        assert_eq!(item.thumbnail, "https://storage.example/static/a.jpg");
        assert_eq!(item.category, "短篇");
        assert_eq!(item.tags, vec!["短篇", "全彩"]);
        assert_eq!(item.likes, "1200");
        assert_eq!(item.source, "bika");
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

    #[test]
    #[ignore = "真实 API 联调,需 BIKA_EMAIL/BIKA_PASSWORD 或 BIKA_TOKEN 环境变量"]
    fn bika_live_endpoints() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                let api = BikaApi::new().expect("bika client");
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
                    "detail: title={} author={} tags={:?}",
                    detail.title, detail.author, detail.tags
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
            });
    }
}
