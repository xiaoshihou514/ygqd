# Tauri command 接口

前端统一通过 `src/services/api.ts` 调用 Rust commands，不存在 HTTP Base URL 或 REST
响应包装。成功值直接返回，Rust 的 `Err(String)` 会使 `invoke` Promise reject。

| Command | 参数 | 返回值 |
|---|---|---|
| `fetch_home` | — | `HomeSection[]` |
| `fetch_category` | `category`, `page` | `{ items, pagination }` |
| `search_comics` | `keyword`, `classid`, `show`, `page`, `cacheBuster?` | `SearchResult` |
| `fetch_comic` | `categoryId`, `id` | `ComicDetail` |
| `fetch_image` | `url` | `ArrayBuffer` |
| `fetch_follows` | — | `FollowedAuthor[]` |
| `follow_author` | `author` | `FollowedAuthor` |
| `unfollow_author` | `author` | `void` |
| `fetch_history` | `limit` | `ViewHistoryEntry[]` |
| `record_history` | `entry` | `void` |
| `fetch_blacklist` | — | `BlacklistEntry[]` |
| `add_blacklist` | `tag`, `mode` | `BlacklistEntry` |
| `remove_blacklist` | `tag` | `void` |
| `update_blacklist` | `tag`, `mode` | `void` |

共享的数据模型定义在 `src/types/index.ts` 和 `src-tauri/src/models.rs`。Rust 模型使用
`#[serde(rename_all = "camelCase")]` 保持与 TypeScript 字段一致。
