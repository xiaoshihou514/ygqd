package com.niacg.backend.db

import org.jetbrains.exposed.sql.selectAll
import org.jetbrains.exposed.sql.upsert

data class ViewHistoryEntry(
    val comicId: String,
    val title: String,
    val thumbnail: String,
    val categoryId: Int,
    val author: String,
    val viewedAt: Long,
)

class ViewHistoryRepository(private val factory: DatabaseProvider) {

    fun record(entry: ViewHistoryEntry) {
        factory.query {
            ViewHistoryTable.upsert(ViewHistoryTable.comicId) {
                it[ViewHistoryTable.comicId] = entry.comicId
                it[ViewHistoryTable.title] = entry.title
                it[ViewHistoryTable.thumbnail] = entry.thumbnail
                it[ViewHistoryTable.categoryId] = entry.categoryId
                it[ViewHistoryTable.author] = entry.author
                it[ViewHistoryTable.viewedAt] = entry.viewedAt
            }
        }
    }

    fun listRecent(limit: Int = 50): List<ViewHistoryEntry> {
        return factory.query {
            ViewHistoryTable.selectAll()
                .orderBy(ViewHistoryTable.viewedAt, org.jetbrains.exposed.sql.SortOrder.DESC)
                .limit(limit)
                .map { row ->
                    ViewHistoryEntry(
                        comicId = row[ViewHistoryTable.comicId] as String,
                        title = row[ViewHistoryTable.title] as String,
                        thumbnail = row[ViewHistoryTable.thumbnail] as String,
                        categoryId = row[ViewHistoryTable.categoryId] as Int,
                        author = row[ViewHistoryTable.author] as String,
                        viewedAt = row[ViewHistoryTable.viewedAt] as Long,
                    )
                }
        }
    }
}
