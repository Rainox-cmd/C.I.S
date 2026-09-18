@echo off
setlocal
echo ==========================================
echo C.I.S. Comprehensive Setup and Startup
echo ==========================================

REM Ensure we are in the script's directory
cd /d "%~dp0"

echo [1/6] Building C.I.S. from source (Release Mode)...
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
cargo build --release --bin cis
if %ERRORLEVEL% NEQ 0 (
    echo Build failed! Please ensure you have the Rust toolchain installed.
    pause
    exit /b 1
)

echo [2/6] Temporarily disabling MCP to release database locks...
set GLOBAL_MCP="%USERPROFILE%\.gemini\config\mcp_config.json"
set LOCAL_MCP=".agents\mcp_config.json"

if exist %GLOBAL_MCP% (
    move /Y %GLOBAL_MCP% "%USERPROFILE%\.gemini\config\mcp_config.json.bak" >nul 2>&1
)
if exist %LOCAL_MCP% (
    move /Y %LOCAL_MCP% ".agents\mcp_config.json.bak" >nul 2>&1
)

echo [3/6] Stopping any background C.I.S. processes...
powershell -Command "Stop-Process -Name cis -Force -ErrorAction SilentlyContinue" >nul 2>&1
REM Short pause to ensure process handles are fully released by the OS
ping 127.0.0.1 -n 3 > nul

echo [4/6] Initializing C.I.S. database...
if not exist ".cis" mkdir ".cis"
.\target\release\cis.exe init

echo [5/6] Scanning and indexing project files...
.\target\release\cis.exe scan

echo [6/6] Restoring MCP configuration...
if exist "%USERPROFILE%\.gemini\config\mcp_config.json.bak" (
    move /Y "%USERPROFILE%\.gemini\config\mcp_config.json.bak" %GLOBAL_MCP% >nul 2>&1
)
if exist ".agents\mcp_config.json.bak" (
    move /Y ".agents\mcp_config.json.bak" %LOCAL_MCP% >nul 2>&1
)

echo ==========================================
echo Setup Complete!
echo Your C.I.S. environment is now fully built, indexed, and connected to Antigravity.
echo ==========================================
pause
