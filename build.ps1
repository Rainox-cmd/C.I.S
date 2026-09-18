$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
$path = & $vswhere -latest -property installationPath
$vcvars = "$path\VC\Auxiliary\Build\vcvars64.bat"
cmd.exe /c "call `"$vcvars`" && cargo build --release --bin cis"
