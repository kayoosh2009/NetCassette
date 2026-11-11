#!/bin/bash
set -e

echo "[1/3] Проверяем таргет x86_64-pc-windows-gnu в rustup..."
rustup target add x86_64-pc-windows-gnu

echo "[2/3] Компилируем NetCassette под Windows... (ง •̀_•́)ง"
cargo build --release --target x86_64-pc-windows-gnu

echo "[3/3] Копируем готовый .exe в корень..."
cp target/x86_64-pc-windows-gnu/release/netcassette.exe ./NetCassette.exe

echo ""
echo "Успех! Файл NetCassette.exe готов к запуску на Windows! (ﾉ^ヮ^)ﾉ*:･ﾟ✧"