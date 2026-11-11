#!/bin/bash
set -e

echo "[1/6] Собираем NetCassette в release режиме... (ง •̀_•́)ง"
cargo build --release

echo "[2/6] Подготавливаем структуру AppDir..."
rm -rf AppDir
mkdir -p AppDir/usr/bin
mkdir -p AppDir/usr/share/icons/hicolor/256x256/apps

echo "[3/6] Копируем бинарник и иконки..."
cp target/release/netcassette AppDir/usr/bin/
cp icons/play.png AppDir/usr/share/icons/hicolor/256x256/apps/netcassette.png
cp icons/play.png AppDir/netcassette.png

echo "[4/6] Генерируем desktop файл и AppRun..."
cat << 'EOF' > AppDir/netcassette.desktop
[Desktop Entry]
Name=NetCassette
Exec=netcassette
Icon=netcassette
Type=Application
Categories=AudioVideo;Audio;Player;
Comment=P2P Retro Cassette Audio Player
Terminal=false
EOF

cat << 'EOF' > AppDir/AppRun
#!/bin/sh
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
exec "${HERE}/usr/bin/netcassette" "$@"
EOF
chmod +x AppDir/AppRun

echo "[5/6] Проверяем наличие appimagetool..."
if [ ! -f "appimagetool" ]; then
    echo "Скачиваем appimagetool..."
    wget -q --show-progress "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" -O appimagetool
    chmod +x appimagetool
fi

echo "[6/6] Собираем итоговый AppImage... (★ω★)"
ARCH=x86_64 ./appimagetool AppDir NetCassette-x86_64.AppImage

echo ""
echo "Готово! Файл NetCassette-x86_64.AppImage успешно собран! (ﾉ^ヮ^)ﾉ*:･ﾟ✧"