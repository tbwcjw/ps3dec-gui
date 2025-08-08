#!/bin/bash

#linux 64
cross build --release --target x86_64-unknown-linux-gnu
x86_linux_path="target/x86_64-unknown-linux-gnu/release/ps3dec_gui"
zip -j "x86_64-unknown-linux-gnu.zip" "$x86_linux_path" 

#linux 32
cross build --release --target i686-unknown-linux-gnu
i686_linux_path="target/i686-unknown-linux-gnu/release/ps3dec_gui"
zip -j "i686-unknown-linux-gnu.zip" "$i686_linux_path" 

#win 64
cross build --release --target x86_64-pc-windows-gnu
x86_64_windows_path="target/x86_64-pc-windows-gnu/release/ps3dec_gui.exe"
zip -j "x86_64-pc-windows-gnu.zip" "$x86_64_windows_path" 


#win 32
cross build --release --target i686-pc-windows-gnu
i686_windows_path="target/i686-pc-windows-gnu/release/ps3dec_gui.exe"
zip -j "i686-pc-windows-gnu.zip" "$i686_windows_path" 

