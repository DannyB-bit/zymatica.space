@echo off
REM rustc-wrapper receives: <original-rustc> <args...>
REM We skip the first arg and call BPF rustc instead
shift
C:\Users\DannyB\.local\share\solana\releases\2.2.3\solana-release\bin\sdk\platform-tools\rust\bin\rustc.exe %1 %2 %3 %4 %5 %6 %7 %8 %9
