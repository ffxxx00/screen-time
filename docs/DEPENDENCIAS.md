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


## Asistente de instalación

El setup se compila con `csc.exe` de .NET Framework incluido en Windows 10/11 x64. Usa Windows Forms, compresión ZIP y SHA-256 del propio framework; no descarga dependencias ni requiere un servidor o compilador adicional en el equipo del usuario. PowerShell 5.1 ejecuta la transacción de instalación incluida.


## Identificación local de aplicaciones

Se usan las interfaces COM del crate windows 0.58 ya presente en el árbol de dependencias (Shell, AppModel y WMI). zip 0.6.6, licencia MIT, permite leer únicamente META-INF/MANIFEST.MF sin extraer archivos; el manifiesto se limita a 64 KiB. Cargo.lock fija las versiones. Java no es una dependencia del tracker: se consulta únicamente cuando un proceso Java ya está ejecutándose.

SHA-256 de identidad nativa: sha2 0.10.9 (RustCrypto, MIT/Apache-2.0), cálculo local en segundo plano; versión fijada en Cargo.lock.
