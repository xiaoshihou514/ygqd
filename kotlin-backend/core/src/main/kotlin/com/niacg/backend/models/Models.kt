package com.niacg.backend.models

import kotlinx.serialization.Serializable

@Serializable
data class ComicItem(
    val id: String,
    val title: String,
    val thumbnail: String,
    val category: String,
    val categoryId: Int,
    val tags: List<String>,
    val likes: String,
    val link: String
)

@Serializable
data class ComicDetail(
    val id: String,
    val title: String,
    val thumbnail: String,
    val category: String,
    val categoryId: Int,
    val author: String,
    val works: List<String>,
    val characters: List<String>,
    val tags: List<String>,
    val likes: String,
    val images: List<String>
)

@Serializable
data class PaginationInfo(
    val current: Int,
    val total: Int,
    val hasNext: Boolean,
    val hasPrev: Boolean
)

@Serializable
data class HomeSection(
    val category: String,
    val categoryId: Int,
    val label: String,
    val items: List<ComicItem>
)

@Serializable
data class SearchResult(
    val items: List<ComicItem>,
    val pagination: PaginationInfo,
    val pageUrlTemplate: String? = null
)

@Serializable
data class CategoryListResult(
    val items: List<ComicItem>,
    val pagination: PaginationInfo
)

@Serializable
data class ApiResponse<T>(
    val code: Int,
    val data: T? = null,
    val message: String? = null
)

@Serializable
data class SearchParams(
    val keyword: String,
    val classid: Int,
    val show: String = "title,text,keyboard,ftitle",
    val tempid: String = "1"
)

@Serializable
data class ParsedTitle(
    val author: String,
    val workName: String,
    val chapters: List<Int>,
    val hasExtra: Boolean,
    val isComplete: Boolean,
    val rawTitle: String
)

@Serializable
data class SplitGroup(
    val author: String,
    val workName: String,
    val items: List<ComicItem>,
    val knownChapters: List<Int>,
    val missingChapters: List<Int>,
    val hasExtra: Boolean,
    val isComplete: Boolean,
    val isStandalone: Boolean
)

@Serializable
data class CategoryOption(
    val id: Int,
    val label: String
)

@Serializable
data class BlacklistEntryResponse(
    val tag: String,
    val mode: String,
    val createdAt: Long,
)

@Serializable
data class BlacklistRequest(
    val tag: String,
    val mode: String,
)

@Serializable
data class BlacklistUpdateRequest(
    val tag: String,
    val mode: String,
)

@Serializable
data class FollowAuthorRequest(val author: String)

@Serializable
data class FollowedAuthorResponse(
    val author: String,
    val followedAt: Long,
    val lastCheckedAt: Long,
)

@Serializable
data class RecordHistoryRequest(
    val comicId: String,
    val title: String,
    val thumbnail: String,
    val categoryId: Int,
    val author: String,
)

@Serializable
data class ViewHistoryResponse(
    val comicId: String,
    val title: String,
    val thumbnail: String,
    val categoryId: Int,
    val author: String,
    val viewedAt: Long,
)
