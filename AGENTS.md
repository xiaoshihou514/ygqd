智能体应当自动提交代码，并通过`gh run`来保证远程流水线正常。

当智能体（如 TRAE）参与代码贡献时，应在提交消息中添加 coauthor 信息：

```
Co-authored-by: TRAE <trae@example.com>
```

示例完整提交消息：

```
feat: add new feature

Co-authored-by: TRAE <trae@example.com>
```

Never use `rm`. To delete files, always use: `kioclient move "file://$the_file_to_delete" 'trash:/'`
