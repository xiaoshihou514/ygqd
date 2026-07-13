package com.niacg.backend.db

import org.jetbrains.exposed.sql.Database
import org.jetbrains.exposed.sql.SchemaUtils
import org.jetbrains.exposed.sql.Transaction
import org.jetbrains.exposed.sql.transactions.transaction
import java.io.File

class DatabaseFactory(jdbcUrl: String) {

    private val db = Database.connect(jdbcUrl, "org.sqlite.JDBC")

    init {
        transaction(db) {
            SchemaUtils.createMissingTablesAndColumns(
                FollowedAuthorsTable,
                ViewHistoryTable,
            )
        }
    }

    fun <T> query(block: Transaction.() -> T): T = transaction(db, block)

    companion object {
        fun forFile(file: File): DatabaseFactory {
            file.parentFile?.mkdirs()
            return DatabaseFactory("jdbc:sqlite:${file.absolutePath}")
        }

        fun inMemory(): DatabaseFactory {
            val tmp = File(System.getProperty("java.io.tmpdir"), "ygqd-test-${System.nanoTime()}.db")
            return forFile(tmp)
        }
    }
}
