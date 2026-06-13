# 后端接口文档

## 概述

本项目通过 Vite 开发服务器内置的中间件代理 niacg.com，提供以下 RESTful API 接口。所有接口统一返回 JSON 格式数据。

### 基础信息

- **Base URL**: `http://localhost:5173`（开发环境）
- **响应格式**: JSON
- **字符编码**: UTF-8

### 通用响应结构

```json
{
  "code": 0,
  "data": {},
  "message": "success"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `code` | `number` | 状态码，`0` 表示成功，`-1` 表示失败 |
| `data` | `T` | 响应数据，类型根据接口不同而变化 |
| `message` | `string?` | 错误信息，仅在失败时返回 |

---

## 数据模型

### ComicItem

漫画条目基础模型，用于列表和搜索结果。

```typescript
interface ComicItem {
  id: string          // 漫画唯一标识
  title: string       // 标题
  thumbnail: string   // 缩略图 URL（已代理为本地接口）
  category: string    // 分类名称（如 "COS", "CG", "本子"）
  categoryId: number  // 分类 ID
  tags: string[]      // 标签列表
  likes: string       // 点赞数
  link: string        // 原始链接
}
```

### ComicDetail

漫画详情模型，在 ComicItem 基础上扩展了详细信息。

```typescript
interface ComicDetail {
  id: string          // 漫画唯一标识
  title: string       // 标题
  thumbnail: string   // 封面图 URL（已代理）
  category: string    // 分类名称
  categoryId: number  // 分类 ID
  author: string      // 作者
  works: string[]     // 作品列表
  characters: string[] // 角色列表
  tags: string[]      // 标签列表
  likes: string       // 点赞数
  images: string[]    // 图片列表（已代理）
}
```

### PaginationInfo

分页信息。

```typescript
interface PaginationInfo {
  current: number  // 当前页码（从 0 开始）
  total: number    // 总页数
  hasNext: boolean // 是否有下一页
  hasPrev: boolean // 是否有上一页
}
```

### HomeSection

首页推荐分组。

```typescript
interface HomeSection {
  category: string     // 分类名称
  categoryId: number   // 分类 ID
  label: string        // 分组标签（如 "COS推荐"）
  items: ComicItem[]   // 该分组的漫画列表
}
```

### SearchParams

搜索请求参数。

```typescript
interface SearchParams {
  keyword: string  // 搜索关键词
  classid: number  // 搜索分类 ID
  show?: string    // 显示字段，默认 "title,text,keyboard,ftitle"
  tempid?: string  // 模板 ID，默认 "1"
}
```

---

## 接口列表

### 1. 获取首页推荐

获取首页各个分类的推荐漫画列表。

```
GET /api/home
```

**请求参数**: 无

**响应数据**: `HomeSection[]`

**响应示例**:

```json
{
  "code": 0,
  "data": [
    {
      "category": "COS",
      "categoryId": 1,
      "label": "COS推荐",
      "items": [
        {
          "id": "12345",
          "title": "示例漫画标题",
          "thumbnail": "/api/image?url=https%3A%2F%2Fexample.com%2Fthumb.jpg",
          "category": "COS",
          "categoryId": 1,
          "tags": ["标签1", "标签2"],
          "likes": "1000",
          "link": "moehome-1-12345.html"
        }
      ]
    }
  ]
}
```

**错误响应**:

```json
{
  "code": -1,
  "message": "错误描述信息"
}
```

---

### 2. 获取分类列表

根据分类 ID 和页码获取对应分类的漫画列表。

```
GET /api/list?cat={categoryId}&page={page}
```

**请求参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cat` | `string` | 是 | 分类 ID |
| `page` | `string` | 是 | 页码（从 0 开始） |

**分类 ID 对照表**:

| ID | 分类名称 |
|----|----------|
| 1 | COS |
| 2 | CG |
| 3 | 本子 |
| 4 | 套图 |
| 9 | A漫 |
| 19 | 里番 |
| 20 | 3D |
| 21 | 同人 |

**响应数据**:

```typescript
{
  items: ComicItem[]
  pagination: PaginationInfo
}
```

**响应示例**:

```json
{
  "code": 0,
  "data": {
    "items": [
      {
        "id": "12345",
        "title": "示例漫画标题",
        "thumbnail": "/api/image?url=https%3A%2F%2Fexample.com%2Fthumb.jpg",
        "category": "本子",
        "categoryId": 3,
        "tags": ["标签1"],
        "likes": "500",
        "link": "moehome-3-12345.html"
      }
    ],
    "pagination": {
      "current": 0,
      "total": 10,
      "hasNext": true,
      "hasPrev": false
    }
  }
}
```

---

### 3. 搜索漫画

根据关键词和分类搜索漫画，支持多页并发拉取全部结果。

```
POST /api/search
Content-Type: application/x-www-form-urlencoded
```

**请求参数**（表单格式）:

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `classid` | `string` | 是 | - | 搜索分类 ID |
| `keyword` | `string` | 是 | - | 搜索关键词 |
| `show` | `string` | 否 | `title,text,keyboard,ftitle` | 搜索范围 |
| `tempid` | `string` | 否 | `1` | 模板 ID |

**响应数据**:

```typescript
{
  items: ComicItem[]
  pagination: PaginationInfo
  pageUrlTemplate: string | null
}
```

> 注意：搜索接口会自动并发拉取所有分页结果，返回的 `pagination` 固定为初始值，所有结果统一在 `items` 中。

**响应示例**:

```json
{
  "code": 0,
  "data": {
    "items": [
      {
        "id": "67890",
        "title": "搜索结果示例",
        "thumbnail": "/api/image?url=https%3A%2F%2Fexample.com%2Fthumb.jpg",
        "category": "A漫",
        "categoryId": 9,
        "tags": ["标签A", "标签B"],
        "likes": "200",
        "link": "moehome-9-67890.html"
      }
    ],
    "pagination": {
      "current": 0,
      "total": 0,
      "hasNext": false,
      "hasPrev": false
    },
    "pageUrlTemplate": null
  }
}
```

---

### 4. 获取漫画详情

获取指定漫画的详细信息，包括作者、作品、角色、标签以及全部图片。

```
GET /api/comic?cat={categoryId}&id={comicId}
```

**请求参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `cat` | `string` | 是 | 分类 ID |
| `id` | `string` | 是 | 漫画 ID |

**响应数据**: `ComicDetail`

**响应示例**:

```json
{
  "code": 0,
  "data": {
    "id": "12345",
    "title": "漫画标题",
    "thumbnail": "/api/image?url=https%3A%2F%2Fexample.com%2Fcover.jpg",
    "category": "COS",
    "categoryId": 1,
    "author": "作者名",
    "works": ["作品1"],
    "characters": ["角色1", "角色2"],
    "tags": ["标签1", "标签2"],
    "likes": "1500",
    "images": [
      "/api/image?url=https%3A%2F%2Fexample.com%2Fimg1.jpg",
      "/api/image?url=https%3A%2F%2Fexample.com%2Fimg2.jpg"
    ]
  }
}
```

**错误响应**（缺少 id 参数）:

```json
{
  "code": -1,
  "message": "Missing id parameter"
}
```

---

### 5. 图片代理

代理远程图片资源，绕过跨域限制并提供缓存。

```
GET /api/image?url={encodedImageUrl}
```

**请求参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `url` | `string` | 是 | 远程图片的完整 URL（需 URL 编码） |

**响应**: 直接返回图片二进制数据，`Content-Type` 为原始图片的 MIME 类型。

**响应头**:

| 头字段 | 值 | 说明 |
|--------|-----|------|
| `Content-Type` | `image/jpeg` 等 | 与原始图片一致 |
| `Cache-Control` | `public, max-age=86400` | 缓存 24 小时 |

**错误响应**（缺少 url 参数）:

```
HTTP 400 Bad Request
Missing url parameter
```

**错误响应**（代理失败）:

```
HTTP 502 Bad Gateway
Image proxy error
```

---

## 错误码说明

| code | HTTP 状态码 | 说明 |
|------|-------------|------|
| `0` | 200 | 请求成功 |
| `-1` | 200 | 业务逻辑错误，详见 `message` 字段 |

图片代理接口不遵循通用 JSON 响应格式，而是直接返回 HTTP 错误状态码：

| HTTP 状态码 | 说明 |
|-------------|------|
| 400 | 缺少 `url` 参数 |
| 502 | 图片代理失败（网络超时或远程服务器错误） |

---

## 前端调用示例

```typescript
import { 
  fetchHomeRecommendations, 
  fetchCategoryList, 
  searchComics, 
  fetchComicDetail 
} from '@/services/api'

// 获取首页推荐
const sections = await fetchHomeRecommendations()

// 获取本子分类第 1 页
const { items, pagination } = await fetchCategoryList(3, 0)

// 搜索漫画
const result = await searchComics({
  keyword: '关键词',
  classid: 9,
})

// 获取漫画详情
const detail = await fetchComicDetail(1, '12345')
```
