# Dependencias y avisos

## Dependencias Rust directas

| Crate | Uso | Licencia declarada |
|---|---|---|
| `chrono` | Marcas de tiempo y calendario local | MIT o Apache-2.0 |
| `csv` | Lectura/escritura compatible del historial | MIT o Apache-2.0 |
| `serde_json` | Preferencias, IPC y journal | MIT o Apache-2.0 |
| `serde` | Serialización del índice diario | MIT o Apache-2.0 |
| `base64` | Fila almacenada dentro del journal | MIT o Apache-2.0 |
| `png` | Serializar iconos nativos extraídos | MIT o Apache-2.0 |
| `windows-sys` | Interfaces Win32 | MIT o Apache-2.0 |
| `tao` | Loop de mensajes y ventanas nativas | MIT o Apache-2.0 |
| `tray-icon` | Icono y menú de bandeja | MIT o Apache-2.0 |
| `wry` | WebView2 para el dashboard | MIT o Apache-2.0 |

`Cargo.lock` registra versiones exactas de todos los crates resueltos, incluidas dependencias transitivas. Cada dependencia mantiene sus propios avisos y términos. Comprueba los metadatos del registry/crate correspondiente antes de redistribuir una modificación o un bundle.

## Chart.js

Las pruebas de desarrollo usan `jsdom` (MIT), fijado en `package-lock.json`. Node y estas dependencias no se incluyen ni se ejecutan en la aplicación instalada.

`assets/chart.umd.min.js` es una copia vendorizada de Chart.js y conserva su licencia en `assets/CHARTJS-LICENSE.txt` (MIT, Chart.js Contributors). Mantenla junto al archivo al redistribuir el dashboard.

## Microsoft WebView2

Wry usa el Microsoft Edge WebView2 Runtime Evergreen instalado en Windows. El runtime no está empaquetado dentro de este repositorio. Consulta [la guía oficial de distribución de WebView2](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution) si preparas un paquete para otros equipos.

## Recurso Win32

`assets/app.ico` y `assets/app.rc` incluyen el icono y metadatos de versión en el ejecutable mediante `rc.exe`, proporcionado por el Windows SDK. El script `tools/generate_icon.py` es opcional y solo se requiere al editar o regenerar los assets PNG/ICO; necesita Pillow.
