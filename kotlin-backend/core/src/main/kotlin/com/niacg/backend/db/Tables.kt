package com.niacg.backend.db

import org.jetbrains.exposed.sql.Table

object FollowedAuthorsTable : Table("followed_authors") {
    val author = varchar("author", 256)
    val followedAt = long("followed_at")
    val lastCheckedAt = long("last_checked_at")

    override val primaryKey = PrimaryKey(author)
}

object ViewHistoryTable : Table("view_history") {
    val comicId = varchar("comic_id", 64)
    val title = varchar("title", 512)
    val thumbnail = varchar("thumbnail", 1024)
    val categoryId = integer("category_id")
    val author = varchar("author", 256)
    val viewedAt = long("viewed_at")

    override val primaryKey = PrimaryKey(comicId)
}
