#!/bin/sh
# start_dev.sh — 小说/漫画/视频桌面开发环境一键拉起 + 探活自愈
# 用法:真实终端执行 `sh start_dev.sh`(前台驻留,自愈循环;Ctrl+C 退出并停止应用)
# 背景:验证期间反复出现的「应用/前端静默死亡」源于后台 job 沙箱的
# 进程树生命周期管理;本脚本以探活循环对抗——任一组件死掉 5 秒内拉起。
cd "$(dirname "$0")" || exit 1
export XDG_DATA_HOME="$PWD/.scratch/xdgdata"
mkdir -p .scratch/xdgdata

has_vite() { curl -s -o /dev/null --max-time 3 http://localhost:5173/; }
has_app() {
  port=$(grep "app setup done" .scratch/xdgdata/com.niacg.ygqd/diag.log 2>/dev/null | tail -1 | grep -oE '[0-9]+$')
  [ -n "$port" ] && curl -s -o /dev/null --max-time 3 "http://127.0.0.1:$port/"
}

echo "[start_dev] 拉起 vite 前端(5173)..."
has_vite || (npm run dev > .scratch/vite.log 2>&1 &)
until has_vite; do sleep 2; done
echo "[start_dev] vite 就绪"

echo "[start_dev] 拉起桌面应用(cargo run,增量编译)..."
(cd src-tauri && exec cargo run --no-default-features) > .scratch/app.log 2>&1 &
APP_PID=$!

while true; do
  sleep 5
  has_vite || { echo "[start_dev] $(date +%T) vite 掉线,拉起..."; (npm run dev > .scratch/vite.log 2>&1 &); }
  # 应用死而未重启(自愈循环由 while 兜底;cargo run 崩溃退出后重新 exec)
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "[start_dev] $(date +%T) 应用退出,重新拉起..."
    (cd src-tauri && exec cargo run --no-default-features) >> .scratch/app.log 2>&1 &
    APP_PID=$!
  fi
done
