# backend-dev 个人操作检查清单

## 清场/杀进程(强制项,两次事故后生效)

- [ ] **一律 `pgrep -x <进程名>` 先列出 pid,再 `kill <pid>`**——禁止 `pkill -f <模式>`(-f 匹配完整命令行,
      会匹配到自身 bash 的命令行导致自杀,已发生两次:侦察实例 143 自杀 ×2)
- [ ] 清场前 `pgrep -fa` 确认目标,kill 后复核进程消失

## 环境启动(第三次事故后生效)

- [ ] **一律 `sh start_dev.sh`**(setsid 分离会话无效——DSH 后台 job 的 bwrap die-with-parent
      会连带清理分离会话;正确方式是 run_in_background job 内前台跑,job 保活则环境活)
- [ ] 禁止手动 cargo run/npm run 起长驻进程

## 提交纪律

- [ ] 提交前 `git status` + `git diff --cached` 复核暂存清单(共享工作区,防误带他人改动)
- [ ] 部分提交(按文件拆分)前:git stash 未暂存部分 → cargo check 验证 staged 子集编译完整性
- [ ] stat 数字直接贴 git diff --stat 原始输出,不手抄
- [ ] 共享工作区的破坏性 git 操作(checkout/restore/stash/reset)先在团队消息打招呼

## 远程同步

- [ ] 每轮开工前 git fetch + git log HEAD..FETCH_HEAD 核对与远程的差距
      (SSH 通道间歇会导致回避 pull,本地 main 落后的累积教训)
