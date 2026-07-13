# 修复 Android Sqldroid dialect 解析

## 问题

`Database.connect("jdbc:sqldroid:...", "org.sqldroid.SQLDroidDriver")` 在 Exposed 的 `connect()` 方法中先调用 `getDialectName(url)`，检查 URL 前缀是否匹配已知数据库。`jdbc:sqldroid:` 不在 Exposed 内置的 URL 前缀列表中，**在用到 `databaseConfig.explicitDialect` 之前就抛异常了**。

## 修复

在 `Application.android.kt` 的 `module()` 函数中，连接数据库前注册 `jdbc:sqldroid` 前缀到 SQLite dialect 的映射：

```kotlin
Database.registerJdbcDriver("jdbc:sqldroid:", "org.sqldroid.SQLDroidDriver", "sqlite")
```

去掉 `DatabaseConfig { explicitDialect = SQLiteDialect() }`，因为注册前缀后 Exposed 自动识别为 SQLite。

### 改动文件

`kotlin-backend/server-android/src/main/kotlin/com/niacg/backend/server/Application.android.kt`
- 删除 `import org.jetbrains.exposed.sql.DatabaseConfig`
- 删除 `import org.jetbrains.exposed.sql.vendors.SQLiteDialect`
- 在 `module()` 中加一行 `Database.registerJdbcDriver(...)`
- `JdbcDatabaseProvider(...)` 调用去掉 `databaseConfig` 参数
