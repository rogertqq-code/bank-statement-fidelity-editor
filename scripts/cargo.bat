@echo off
REM Prepend MinGW64 bin dir to PATH so pyo3-ffi build script can find dlltool.exe.
REM The cargo [env] PATH override is avoided because it completely replaces
REM the inherited PATH and breaks the GNU linker on Windows.
set "PATH=C:\msys64\mingw64\bin;%PATH%"
"C:\Users\zbook\.rustup\toolchains\1.89.0-x86_64-pc-windows-gnu\bin\cargo.exe" %*
