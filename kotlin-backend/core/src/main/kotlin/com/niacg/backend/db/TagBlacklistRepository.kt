package com.niacg.backend.db

import org.jetbrains.exposed.sql.SqlExpressionBuilder.eq
import org.jetbrains.exposed.sql.deleteWhere
import org.jetbrains.exposed.sql.insertIgnore
import org.jetbrains.exposed.sql.selectAll
import org.jetbrains.exposed.sql.update

data class BlacklistEntry(
    val tag: String,
    val mode: String,
    val createdAt: Long,
)

class TagBlacklistRepository(private val factory: DatabaseProvider) {

    fun add(tag: String, mode: String) {
        val now = System.currentTimeMillis()
        factory.query {
            TagBlacklistTable.insertIgnore {
                it[TagBlacklistTable.tag] = tag
                it[TagBlacklistTable.mode] = mode
                it[TagBlacklistTable.createdAt] = now
            }
        }
    }

    fun remove(tag: String) {
        factory.query {
            TagBlacklistTable.deleteWhere { TagBlacklistTable.tag eq tag }
        }
    }

    fun updateMode(tag: String, mode: String) {
        factory.query {
            TagBlacklistTable.update({ TagBlacklistTable.tag eq tag }) {
                it[TagBlacklistTable.mode] = mode
            }
        }
    }

    fun listAll(): List<BlacklistEntry> {
        return factory.query {
            TagBlacklistTable.selectAll()
                .orderBy(TagBlacklistTable.createdAt, org.jetbrains.exposed.sql.SortOrder.DESC)
                .map { row ->
                    BlacklistEntry(
                        tag = row[TagBlacklistTable.tag] as String,
                        mode = row[TagBlacklistTable.mode] as String,
                        createdAt = row[TagBlacklistTable.createdAt] as Long,
                    )
                }
        }
    }
}
