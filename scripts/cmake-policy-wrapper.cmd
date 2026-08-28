@echo off
setlocal EnableDelayedExpansion

rem 兼容 CMake 4.x：为“配置阶段”注入 CMAKE_POLICY_VERSION_MINIMUM，避免旧项目报错。
rem 注意：--build/--install 阶段不应追加 -D 参数，否则会被 cmake 视为无效参数。

set inject=1
if "%~1"=="--build" set inject=0
if "%~1"=="--install" set inject=0

if "%inject%"=="1" (
  cmake %* -DCMAKE_POLICY_VERSION_MINIMUM=3.5
) else (
  cmake %*
)

exit /b %errorlevel%

