#!/bin/bash
# Regenera todos los iconos de la aplicación a partir de docs/icon/icon.svg.
#
#   bash docs/icon/generate.sh
#
# Solo usa herramientas que trae macOS (qlmanage, sips, iconutil, python3): el
# icono se rehace sin instalar nada y sin depender de un servicio externo.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_svg="$root/docs/icon/icon.svg"
icons="$root/src-tauri/icons"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [ ! -f "$source_svg" ]; then
  echo "no encuentro $source_svg" >&2
  exit 1
fi

# QuickLook rasteriza el SVG a la resolución que se le pida; el PNG maestro sale
# a 1024 y de ahí se reducen los demás tamaños.
cp "$source_svg" "$work/icon.svg"
qlmanage -t -s 1024 -o "$work" "$work/icon.svg" >/dev/null 2>&1
master="$work/icon.svg.png"
if [ ! -f "$master" ]; then
  echo "qlmanage no pudo rasterizar el SVG" >&2
  exit 1
fi

emit() { # emit <tamaño> <destino>
  sips -z "$1" "$1" "$master" --out "$2" >/dev/null
}

emit 32 "$icons/32x32.png"
emit 128 "$icons/128x128.png"
emit 256 "$icons/128x128@2x.png"
emit 512 "$icons/icon.png"

# Logos de la Microsoft Store: los pide el empaquetado de Windows de Tauri.
emit 30 "$icons/Square30x30Logo.png"
emit 44 "$icons/Square44x44Logo.png"
emit 50 "$icons/StoreLogo.png"
emit 71 "$icons/Square71x71Logo.png"
emit 89 "$icons/Square89x89Logo.png"
emit 107 "$icons/Square107x107Logo.png"
emit 142 "$icons/Square142x142Logo.png"
emit 150 "$icons/Square150x150Logo.png"
emit 284 "$icons/Square284x284Logo.png"
emit 310 "$icons/Square310x310Logo.png"

# .icns para macOS: iconutil exige un .iconset con estos nombres exactos.
iconset="$work/MoneyWatcher.iconset"
mkdir -p "$iconset"
emit 16 "$iconset/icon_16x16.png"
emit 32 "$iconset/icon_16x16@2x.png"
emit 32 "$iconset/icon_32x32.png"
emit 64 "$iconset/icon_32x32@2x.png"
emit 128 "$iconset/icon_128x128.png"
emit 256 "$iconset/icon_128x128@2x.png"
emit 256 "$iconset/icon_256x256.png"
emit 512 "$iconset/icon_256x256@2x.png"
emit 512 "$iconset/icon_512x512.png"
emit 1024 "$iconset/icon_512x512@2x.png"
iconutil -c icns "$iconset" -o "$icons/icon.icns"

# .ico para Windows: un contenedor con los PNG dentro, que Windows admite desde
# Vista. Se arma a mano porque macOS no trae ninguna herramienta que lo escriba.
for size in 16 24 32 48 64 128 256; do
  emit "$size" "$work/ico-$size.png"
done

python3 - "$work" "$icons/icon.ico" <<'PYTHON'
import struct
import sys
from pathlib import Path

work = Path(sys.argv[1])
target = Path(sys.argv[2])
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [(size, (work / f"ico-{size}.png").read_bytes()) for size in sizes]

header = struct.pack("<HHH", 0, 1, len(images))
offset = len(header) + 16 * len(images)
entries = bytearray()
payload = bytearray()

for size, data in images:
    # 0 significa 256 en el formato ICO: el campo es de un solo byte.
    dimension = 0 if size == 256 else size
    entries += struct.pack(
        "<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(data), offset
    )
    payload += data
    offset += len(data)

target.write_bytes(header + bytes(entries) + bytes(payload))
PYTHON

echo "iconos regenerados en $icons"
