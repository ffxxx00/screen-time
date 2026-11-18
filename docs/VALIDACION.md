# Validación y recuperación

## Estado de CI, firma y reporte de seguridad

GitHub Actions está habilitado para compilar, probar y empaquetar. La solicitud a SignPath Foundation fue enviada el 18/11/2026; aprobación e integración pendientes. Los binarios 0.3.0 siguen sin firma Authenticode. El reporte de VirusTotal fue asociado al SHA-256 del instalador; el informe completo y sus detecciones todavía requieren investigación. Ver [SECURITY.md](../SECURITY.md). Estas comprobaciones de identidad y firma no equivalen a una auditoría de malware.

## Release pública 0.3.0 — 18 de noviembre de 2026

[Release 0.3.0](https://github.com/ffxxx00/screen-time/releases/tag/v0.3.0) publicada con cuatro archivos: EXE, ZIP y sus SHA-256. Se verificaron las descargas públicas y la coincidencia de la etiqueta con el commit compilado `c5e8eeb`.

Ruta única `%LOCALAPPDATA%\Screen Time` y entrada de autoarranque `Screen Time`; los identificadores de instancia y lifecycle usan el mismo nombre. Se retiraron las rutinas de migración de productos anteriores. El cambio local de carpeta se realiza una sola vez fuera del código distribuido.

Comprobaciones de esta revisión: formato Rust y diff sin errores; 71 pruebas Rust aprobadas y 4 ignoradas; 40 pruebas de dashboard aprobadas; pruebas de instalación con fallos inyectados, rollback, retención de backups y desinstalación aprobadas. Se ejecutaron con el SDK de Windows configurado y los módulos de Windows PowerShell en su ruta estándar.

Compilación Release 0.3.0 aprobada. ZIP y setup EXE generados con SHA-256; aprobaron las pruebas de contenido, copia temporal, extracción, paquete alterado, argumentos, limpieza y subsistema gráfico. Instalación local desde ZIP aprobada: ejecutable instalado idéntico al empaquetado, autoarranque conservado e historial anterior preservado byte por byte. El smoke test abrió el dashboard mediante una segunda invocación, comprobó una sola instancia y cerró la ventana manteniendo el tracker activo. No se realizó una auditoría visual interactiva ni una instalación en Windows limpio.

El paquete usa una lista explícita de documentos técnicos: no incluye capturas ni informes históricos de auditoría. La prueba de paquete rechaza documentos fuera de esa lista. El repositorio conserva ese archivo histórico y ofrece un índice de documentación en docs/README.md.

La validación en Windows limpio y otros equipos sigue pendiente; no se aplicó firma Authenticode. Antes de publicar se revisaron 156 blobs históricos en busca de patrones de credenciales y nombres de archivos sensibles, sin coincidencias. Esta comprobación no sustituye una auditoría de seguridad.

## Comprobaciones reproducibles

Desde un prompt x64 Native Tools con Rust, Windows SDK y Node 22 o posterior:

```powershell
cargo fmt --all -- --check
cargo test --locked --target x86_64-pc-windows-msvc
npm ci --ignore-scripts
npm test
.\tests\install.test.ps1
.\tests\uninstall.test.ps1
cargo build --release --locked --target x86_64-pc-windows-msvc
.\tools\package-release.ps1
.\tools\build-setup.ps1
.\tests\setup.test.ps1
.\tests\package.test.ps1
```

Las pruebas de persistencia usan historiales sintéticos, cortes de procesos hijos, recuperación parcial y conflictos de journal. Las pruebas de instalador inyectan fallos en carpetas temporales. Las pruebas de interfaz usan jsdom y un doble de Chart.js: no sustituyen una revisión visual en WebView2.

Las cuatro pruebas Rust ignoradas son dos benchmarks de historiales grandes y dos integraciones optativas de identidad nativa/WMI/Java con requisitos locales. Se deben ejecutar explícitamente cuando se mida ese comportamiento.

`tools/verify-package-install.ps1` actualiza la instalación del usuario actual desde el ZIP. `tests/smoke-installed.ps1` requiere una sola instancia instalada, abre el dashboard mediante una segunda invocación y vuelve a cerrarlo. No son ensayos aislados en una carpeta temporal.

## Matriz manual

| Comprobación | Estado |
| --- | --- |
| Actualización local, historial y autoarranque | Aprobada el 18/11/2026 |
| Segunda invocación, apertura y cierre del dashboard | Aprobada el 18/11/2026 |
| Instalación y desinstalación en Windows 10/11 limpio | Pendiente |
| WebView2 ausente y reintento desde el asistente | Pendiente en Windows limpio |
| Bloqueo, suspensión, reanudación y energía | Simulaciones automatizadas; uso real confirmado por el usuario en su hardware el 15/10/2026; otros equipos pendientes |
| RDP, múltiples monitores, DPI y accesibilidad con Narrador | Pendiente |
| RAM/CPU en otros equipos, bandeja y dashboard | Pendiente; sin garantía universal de consumo |
| Firma Authenticode | No aplicada |

## Recuperación de datos

La fuente es `%LOCALAPPDATA%\Screen Time\app_usage.csv`. Antes de intervenir, cierra el tracker desde la bandeja y respalda la carpeta completa, incluidos journal y preferencias.

- `app_usage.daily.json` es un índice regenerable. Con el tracker cerrado, puede retirarse para reconstruirlo desde el CSV al abrir.
- `app_usage.pending.json` completa una escritura interrumpida. No lo borres para ocultar un conflicto: conserva el journal y el CSV para diagnosticarlo.
- El instalador guarda copias previas en `backups/pre-install-*`. Ante una actualización fallida, restaura archivos y datos anteriores y conserva aparte las escrituras del intento fallido.
- Para volver a una versión anterior que no lea las filas nuevas, restaura su respaldo previo con la aplicación cerrada. No combines a mano formatos incompatibles.
- El borrado desde Configuración deja el seguimiento pausado y conserva preferencias, exportaciones y backups. Estas copias se gestionan por separado.

Un cierre inesperado puede perder aproximadamente el último minuto de actividad; errores persistentes de disco pueden ampliar esa pérdida. Una fecha marcada como medida indica actividad del tracker durante ese día, no cobertura continua de 24 horas.
