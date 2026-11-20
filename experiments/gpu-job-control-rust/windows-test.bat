@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 exit /b 1
cd /d "%~dp0"
if not defined RUSTUP_HOME set "RUSTUP_HOME=%USERPROFILE%\.rustup"
set "PATH=%RUSTUP_HOME%\toolchains\1.88.0-x86_64-pc-windows-msvc\bin;%PATH%"
cargo test
exit /b %errorlevel%
