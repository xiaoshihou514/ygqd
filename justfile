default:
    @just --list

dev:
    npm run android:dev

init:
    npm run android:init

build:
    npm run android

type-check:
    npm run type-check

check:
    npm run type-check
    npm run build-only
    cd src-tauri && cargo check

format:
    npm run format
    cd src-tauri && cargo fmt
