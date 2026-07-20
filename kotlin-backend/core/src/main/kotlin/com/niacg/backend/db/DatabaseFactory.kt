package com.niacg.backend.db

import org.jetbrains.exposed.sql.Database
import org.jetbrains.exposed.sql.DatabaseConfig
import org.jetbrains.exposed.sql.SchemaUtils
import org.jetbrains.exposed.sql.Transaction
import org.jetbrains.exposed.sql.transactions.transaction
import org.jetbrains.exposed.sql.vendors.SQLiteDialect
import java.io.File
import java.sql.DriverManager

interface DatabaseProvider {
    fun <T> query(block: Transaction.() -> T): T
}

class JdbcDatabaseProvider(
    jdbcUrl: String,
    driverClass: String = "org.sqlite.JDBC",
    databaseConfig: DatabaseConfig? = null,
) : DatabaseProvider {

    private val db = Database.connect(
        getNewConnection = {
            Class.forName(driverClass).getDeclaredConstructor().newInstance()
            DriverManager.getConnection(jdbcUrl)
        },
        databaseConfig = databaseConfig ?: DatabaseConfig { explicitDialect = SQLiteDialect() },
    )

    init {
        transaction(db) {
            SchemaUtils.create(FollowedAuthorsTable)
            SchemaUtils.create(ViewHistoryTable)
            SchemaUtils.create(TagBlacklistTable)
        }
    }

    override fun <T> query(block: Transaction.() -> T): T = transaction(db, block)

    companion object {
        fun forFile(file: File): JdbcDatabaseProvider {
            file.parentFile?.mkdirs()
            return JdbcDatabaseProvider("jdbc:sqlite:${file.absolutePath}")
        }

        fun inMemory(): JdbcDatabaseProvider {
            val tmp = File(System.getProperty("java.io.tmpdir"), "ygqd-test-${System.nanoTime()}.db")
            return forFile(tmp)
        }
    }
}
