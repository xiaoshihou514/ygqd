_grd := "kotlin-backend/gradlew"

# show available recipes
default:
    @just --list

# ── Development ──────────────────────────────────────────────

# start Vite dev server (:5173)
dev:
    npx vite

# start Kotlin desktop backend (:8080)
backend:
    {{_grd}} :server-desktop:run

# preview production build
preview:
    npx vite preview

# ── Lint & Format ────────────────────────────────────────────

# run eslint --fix
lint:
    npx eslint . --fix

# run prettier
format:
    npx prettier --write src/

# run full verification (test + coverage >= 95%)
check:
    npx eslint .
    {{_grd}} check
    {{_grd}} :core:test

# ── Package ──────────────────────────────────────────────────

# auto-rebuild Vue if src/ changed, then package desktop app
desktop: _vue-build-if-needed
    {{_grd}} :server-desktop:installDist

# auto-rebuild Vue if src/ changed, then assemble Android APK
android: _vue-build-if-needed
    {{_grd}} :server-android:assembleRelease

_vue-build-if-needed:
    @if [ ! -d dist ] || [ -n "$$(find src/ -newer dist -type f -print -quit 2>/dev/null)" ]; then just build-only; fi

# ── Launch ───────────────────────────────────────────────────

# open new kitty window running backend + frontend
launch-kitty:
    kitty -o allow_remote_control=yes -o enabled_layouts=tall just --justfile {{justfile()}} --working-directory {{justfile_directory()}} launch

# start backend + frontend in kitty splits (run inside kitty)
launch:
    kitten @ launch bash -c 'just --justfile {{justfile()}} --working-directory {{justfile_directory()}} dev'
    {{_grd}} :server-desktop:run
