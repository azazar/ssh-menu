"""Build and test Windows code under Wine; run in the disposable Wine container."""
import os
from pathlib import Path
import subprocess
import sys

def run(*args, **kwargs):
    subprocess.run(args, check=True, timeout=600, **kwargs)

target = "x86_64-pc-windows-gnu"
if os.environ.get("SHUI_TEST_TOOLCHAIN"):
    os.environ["RUSTUP_TOOLCHAIN"] = os.environ["SHUI_TEST_TOOLCHAIN"]
run("rustup", "target", "add", target)
run("cargo", "clippy", "--locked", "--target", target, "--all-targets", "--", "-D", "warnings")
run("cargo", "build", "--locked", "--target", target)
binary = Path(os.environ["CARGO_TARGET_DIR"]) / target / "debug/shui.exe"
# Wine replaces USERPROFILE/USERNAME during startup. Restore test values inside
# Windows, without a command shell interpreting the program's arguments.
source = Path(os.environ["CARGO_TARGET_DIR"]) / "wine-runner.c"
source.write_text(r'''
#include <windows.h>
#include <stdlib.h>
#include <wchar.h>
int wmain(void) {
    const wchar_t *home = _wgetenv(L"SHUI_TEST_USERPROFILE");
    const wchar_t *user = _wgetenv(L"SHUI_TEST_USERNAME");
    if (_wputenv_s(L"USERPROFILE", home ? home : L"") ||
        _wputenv_s(L"USERNAME", user ? user : L"")) return 3;
    /* Keep Wine's original Windows quoting for every forwarded argument. */
    wchar_t *command = GetCommandLineW();
    int quoted = 0;
    while (*command) {
        if (*command == L'"') quoted = !quoted;
        if (!quoted && (*command == L' ' || *command == L'\t')) break;
        command++;
    }
    while (*command == L' ' || *command == L'\t') command++;
    if (!*command) return 2;
    STARTUPINFOW startup = {0};
    PROCESS_INFORMATION process = {0};
    startup.cb = sizeof(startup);
    if (!CreateProcessW(NULL, command, NULL, NULL, TRUE, 0, NULL, NULL,
                        &startup, &process)) return 127;
    CloseHandle(process.hThread);
    DWORD status = 1;
    if (WaitForSingleObject(process.hProcess, INFINITE) == WAIT_OBJECT_0)
        GetExitCodeProcess(process.hProcess, &status);
    CloseHandle(process.hProcess);
    return (int)status;
}
''')
runner = source.with_suffix(".exe")
run("x86_64-w64-mingw32-gcc", "-municode", "-Wall", "-Wextra", "-Werror",
    str(source), "-o", str(runner))
Path(os.environ["WINEPREFIX"]).mkdir(parents=True, exist_ok=True)
try:
    run("wineboot", "-u")
    run("cargo", "test", "--locked", "--target", target,
        env={**os.environ, "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER": "wine"})
    run(sys.executable, "tests/cli.py", str(binary),
        env={**os.environ, "SHUI_TEST_WINE": "wine", "SHUI_TEST_WINE_RUNNER": str(runner)})
finally:
    run("wineserver", "-k")
