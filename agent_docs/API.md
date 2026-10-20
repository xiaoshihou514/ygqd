# Tauri command 接口

前端统一通过 `src/services/api.ts` 调用 Rust commands，不存在 HTTP Base URL 或 REST
响应包装。成功值直接返回，Rust 的 `Err(String)` 会使 `invoke` Promise reject。

| Command | 参数 | 返回值 |
|---|---|---|
| `fetch_category` | `source`, `category`, `page` | `{ items, pagination }` |
| `fetch_bika_categories` | — | `string[]` |
| `fetch_bika_leaderboard` | `tt`（`H24`/`D7`/`D30`） | `{ items, pagination }`（无分页，一次返回整个榜） |
| `bika_switch_like` | `id` | `void`（toggle） |
| `bika_switch_favourite` | `id` | `void`（toggle） |
| `fetch_favorites` | — | `FavoriteEntry[]` |
| `is_favorite` | `source`, `comicId` | `bool` |
| `add_favorite` | `entry` | `void`（bika 源同时同步服务器） |
| `remove_favorite` | `source`, `comicId` | `void`（bika 源同时同步服务器） |
| `sync_bika_favorites` | — | `number`（本次同步新增数） |
| `search_comics` | `source`, `keyword`, `classid`, `show`, `page`, `cacheBuster?` | `SearchResult` |
| `fetch_comic` | `source`, `categoryId`, `id` | `ComicDetail` |
| `fetch_comic_images` | `source`, `categoryId`, `id`, `epOrder`, `page` | `ComicImagesResult` |
| `fetch_image` | `url`, `source` | `ArrayBuffer` |
| `bika_login` | `email`, `password` | `BikaAccountStatus` |
| `bika_logout` | — | `void` |
| `bika_status` | — | `BikaAccountStatus` |
| `fetch_bika_addresses` | — | `string[]`（从官方 init 端点同步分流 IP） |
| `get_bika_address` | — | `string`（空串表示不分流） |
| `set_bika_address` | `address` | `void`（重建哔咔客户端并清缓存） |
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
