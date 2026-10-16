use crate::models::*;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE: &str = "https://picaapi.picacomic.com";
const API_KEY: &str = "C69BAF41DA5ABD1FFEDC6D2FEA56B";
const SECRET: &str = "~d}$Q7$eIni=V)9\\RK/P.RM4;9[7|@/CA}b~OW!3?EV`:<>M7pddUBL5n|0/*Cn";

/// PicaAPI 请求签名:`path(含 query) + time + nonce + METHOD + api-key`
/// 直接拼接后整体转小写,再以固定密钥做 HMAC-SHA256(小写 hex)
pub fn signature(path_and_query: &str, time: &str, nonce: &str, method: &str) -> String {
    let raw = format!("{path_and_query}{time}{nonce}{method}{API_KEY}").to_lowercase();
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).expect("static hmac key");
    mac.update(raw.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now_secs() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub struct BikaApi {
    pub client: reqwest::Client,
}

fn jstr(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn jnum(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn jarr_str(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn image_url(value: &Value) -> String {
    format!("{}{}", jstr(value, "fileServer"), jstr(value, "path"))
}

fn comic_item(doc: &Value) -> ComicItem {
    let categories = jarr_str(doc, "categories");
    ComicItem {
        source: "bika".into(),
        id: jstr(doc, "_id"),
        title: jstr(doc, "title"),
        thumbnail: doc.get("thumb").map(image_url).unwrap_or_default(),
        category: categories.first().cloned().unwrap_or_default(),
        category_id: 0,
        tags: categories,
        likes: jnum(doc, "likesCount").to_string(),
        link: String::new(),
    }
}

/// PicaAPI 分页为 1-based,统一转换为前端使用的 0-based current
fn pagination(data: &Value) -> PaginationInfo {
    let page = data.get("page").and_then(Value::as_i64).unwrap_or(1).max(1);
    let pages = data
        .get("pages")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1);
    PaginationInfo {
        current: (page - 1) as i32,
        total: pages as i32,
        has_next: page < pages,
        has_prev: page > 1,
    }
}

fn docs(data: &Value) -> Vec<Value> {
    data.get("docs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
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
        headers.insert("app-channel", HeaderValue::from_static("2"));
        headers.insert("app-version", HeaderValue::from_static("2.2.1.3.3.4"));
        headers.insert("app-uuid", HeaderValue::from_static("defaultUuid"));
        headers.insert("app-platform", HeaderValue::from_static("android"));
        headers.insert("app-build-version", HeaderValue::from_static("45"));
        headers.insert("image-quality", HeaderValue::from_static("original"));
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

    /// 返回 Err("401") 表示 token 失效,调用方应重新登录后重试
    async fn request(
        &self,
        method: &str,
        path: &str,
        query: Option<String>,
        body: Option<Value>,
        token: Option<&str>,
    ) -> Result<Value, String> {
        let url = match &query {
            Some(query) => format!("{BASE}{path}?{query}"),
            None => format!("{BASE}{path}"),
        };
        let time = now_secs();
        let nonce = nonce();
        let path_and_query = match &query {
            Some(query) => format!("{path}?{query}"),
            None => path.to_string(),
        };
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
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response.json().await.map_err(|e| e.to_string())?;
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("bika request failed");
        if status.as_u16() == 401 {
            return Err(format!("401 {message}"));
        }
        let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
        if !status.is_success() || code != 200 {
            return Err(format!("{message} ({})", status.as_u16()));
        }
        Ok(value.get("data").cloned().unwrap_or(Value::Null))
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<String, String> {
        let data = self
            .request(
                "POST",
                "/auth/sign-in",
                None,
                Some(json!({ "email": email, "password": password })),
                None,
            )
            .await?;
        data.get("token")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "bika sign-in response missing token".into())
    }

    pub async fn categories(&self, token: String) -> Result<Vec<String>, String> {
        let data = self
            .request("GET", "/categories", None, None, Some(token.as_str()))
            .await?;
        Ok(data
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|v| jbool(v, "active") && !jbool(v, "isWeb"))
                    .map(|v| jstr(v, "title"))
                    .filter(|v| !v.is_empty())
                    .collect()
            })
            .unwrap_or_default())
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
        let data = self
            .request("GET", "/comics", Some(query), None, Some(token.as_str()))
            .await?;
        Ok(CategoryListResult {
            items: docs(&data).iter().map(comic_item).collect(),
            pagination: pagination(&data),
        })
    }

    pub async fn search(
        &self,
        token: String,
        keyword: &str,
        page: i32,
    ) -> Result<CategoryListResult, String> {
        let query = encode_query(&[("page", &page.to_string())]);
        let data = self
            .request(
                "POST",
                "/comics/search",
                Some(query),
                Some(json!({ "keyword": keyword, "sort": "dd", "categories": [] })),
                Some(token.as_str()),
            )
            .await?;
        Ok(CategoryListResult {
            items: docs(&data).iter().map(comic_item).collect(),
            pagination: pagination(&data),
        })
    }

    pub async fn detail(&self, token: String, id: &str) -> Result<ComicDetail, String> {
        let path = format!("/comics/{id}");
        let data = self
            .request("GET", &path, None, None, Some(token.as_str()))
            .await?;
        let categories = jarr_str(&data, "categories");
        let tags = jarr_str(&data, "tags");
        let mut all_tags = categories.clone();
        for tag in tags {
            if !all_tags.contains(&tag) {
                all_tags.push(tag);
            }
        }
        let author = jstr(&data, "author");
        Ok(ComicDetail {
            source: "bika".into(),
            id: jstr(&data, "_id"),
            title: jstr(&data, "title"),
            thumbnail: data.get("thumb").map(image_url).unwrap_or_default(),
            category: categories.first().cloned().unwrap_or_default(),
            category_id: 0,
            author,
            works: vec![],
            characters: vec![],
            tags: all_tags,
            likes: jnum(&data, "likesCount").to_string(),
            episodes: vec![],
            images: vec![],
        })
    }

    /// 循环拉取全部章节,order 升序
    pub async fn episodes(&self, token: String, id: &str) -> Result<Vec<Episode>, String> {
        let mut result = Vec::new();
        let mut page = 1;
        loop {
            let query = encode_query(&[("page", &page.to_string())]);
            let data = self
                .request(
                    "GET",
                    &format!("/comics/{id}/eps"),
                    Some(query),
                    None,
                    Some(token.as_str()),
                )
                .await?;
            for doc in docs(&data) {
                result.push(Episode {
                    order: jnum(&doc, "order") as i32,
                    title: jstr(&doc, "title"),
                });
            }
            if !pagination(&data).has_next {
                break;
            }
            page += 1;
        }
        result.sort_by_key(|ep| ep.order);
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
        let data = self
            .request(
                "GET",
                &format!("/comics/{id}/order/{order}/pages"),
                Some(query),
                None,
                Some(token.as_str()),
            )
            .await?;
        let images = docs(&data)
            .iter()
            .map(|doc| doc.get("media").map(image_url).unwrap_or_default())
            .filter(|v| !v.is_empty())
            .collect();
        Ok(ComicImagesResult {
            images,
            pagination: pagination(&data),
        })
    }
}

fn jbool(value: &Value, key: &str) -> bool {
    match value.get(key) {
        Some(Value::Bool(b)) => *b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn maps_comic_item_from_api_doc() {
        let doc: Value = serde_json::from_str(
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
        let item = comic_item(&doc);
        assert_eq!(item.id, "5822a40b24fcb814c3ed542d");
        assert_eq!(item.thumbnail, "https://storage.example/static/a.jpg");
        assert_eq!(item.category, "短篇");
        assert_eq!(item.tags, vec!["短篇", "全彩"]);
        assert_eq!(item.likes, "1200");
        assert_eq!(item.source, "bika");
    }

    #[test]
    fn maps_pagination_from_api_page() {
        let data: Value = serde_json::from_str(
            r#"{ "docs": [], "page": 2, "pages": 5, "total": 40, "limit": 8 }"#,
        )
        .unwrap();
        let p = pagination(&data);
        assert_eq!(p.current, 1);
        assert_eq!(p.total, 5);
        assert!(p.has_next && p.has_prev);
    }
}
