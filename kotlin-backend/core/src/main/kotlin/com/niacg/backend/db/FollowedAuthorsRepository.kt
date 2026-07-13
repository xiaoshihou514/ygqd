package com.niacg.backend.db

import org.jetbrains.exposed.sql.SqlExpressionBuilder.eq
import org.jetbrains.exposed.sql.deleteWhere
import org.jetbrains.exposed.sql.insertIgnore
import org.jetbrains.exposed.sql.selectAll
import org.jetbrains.exposed.sql.update

data class FollowedAuthor(
    val author: String,
    val followedAt: Long,
    val lastCheckedAt: Long,
)

class FollowedAuthorsRepository(private val factory: DatabaseProvider) {

    fun follow(author: String) {
        val now = System.currentTimeMillis()
        factory.query {
            FollowedAuthorsTable.insertIgnore {
                it[FollowedAuthorsTable.author] = author
                it[FollowedAuthorsTable.followedAt] = now
                it[FollowedAuthorsTable.lastCheckedAt] = now
            }
        }
    }

    fun unfollow(author: String) {
        factory.query {
            FollowedAuthorsTable.deleteWhere { FollowedAuthorsTable.author eq author }
        }
    }

    fun isFollowing(author: String): Boolean {
        return factory.query {
            FollowedAuthorsTable.select(FollowedAuthorsTable.author)
                .where { FollowedAuthorsTable.author eq author }
                .count() > 0
        }
    }

    fun listAll(): List<FollowedAuthor> {
        return factory.query {
            FollowedAuthorsTable.selectAll()
                .orderBy(FollowedAuthorsTable.followedAt, org.jetbrains.exposed.sql.SortOrder.DESC)
                .map { row ->
                    FollowedAuthor(
                        author = row[FollowedAuthorsTable.author] as String,
                        followedAt = row[FollowedAuthorsTable.followedAt] as Long,
                        lastCheckedAt = row[FollowedAuthorsTable.lastCheckedAt] as Long,
                    )
                }
        }
    }

    fun touch(author: String) {
        val now = System.currentTimeMillis()
        factory.query {
            FollowedAuthorsTable.update({ FollowedAuthorsTable.author eq author }) {
                it[FollowedAuthorsTable.lastCheckedAt] = now
            }
        }
    }
}
