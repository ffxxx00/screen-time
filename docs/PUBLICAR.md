# Publicar una versión personal

Esta guía es para el mantenedor del proyecto. Subir el código a GitHub y etiquetarlo no equivale a certificar su precisión o confiabilidad.

## Revisión antes de publicar

1. Comprueba que `LICENSE`, licencia Chart.js y avisos de crates están presentes.
2. Revisa `git status --short`; confirma que `target/`, el historial de LocalAppData, rutas privadas, backups y cualquier `.env` no estén incluidos.
3. Actualiza `Cargo.toml`, `Cargo.lock`, `assets/app.rc` y `CHANGELOG.md` conjuntamente al versionar un release.
4. En un prompt x64 Native Tools limpio, ejecuta:

   ```bat
   cargo fmt --all -- --check
   cargo build --release --locked --target x86_64-pc-windows-msvc
   cargo test --locked --target x86_64-pc-windows-msvc
   ```

5. Revisa el binario como archivo PE sin consola y los recursos de icono/versión. Instala WebView2 Runtime si hace falta.
6. Revisa el uso de RAM/CPU por separado para el daemon inactivo y el dashboard abierto. Prueba el timer de checkpoint, recuperación parcial, suspensión, reanudación automática, bloqueo, unlock, fin de sesión y el arranque en Windows limpio. El comportamiento de energía necesita una máquina Windows real.
7. Prueba la actualización con datos de ejemplo en otro perfil temporal; comprueba acceso directo, autoarranque y retención del historial. No copies historiales reales a runners de CI ni a issues.

## Crear un release de GitHub

1. Etiqueta una versión, por ejemplo `v0.2.0`, solo después de completar la validación descrita arriba.
2. Compila el binario para `x86_64-pc-windows-msvc` y crea un ZIP que contenga `screen-time.exe`, `LICENSE`, `README.md`, `assets/CHARTJS-LICENSE.txt` y un paso de instalación que también actualice el Registro y los accesos directos.
3. Calcula un SHA-256 para el ZIP y publícalo en las notas del release.
4. Indica Windows/arquitectura, WebView2 requerido, estado de firma digital, pasos de actualización/desinstalación y limitaciones conocidas.
5. No describas el binario como firmado o instalado si no verificaste la firma, el checksum y los accesos directos publicados.

`tools/package-release.ps1` genera el ZIP y su SHA-256 en `dist/`. El paquete incluye `Instalar.cmd`, que usa el mismo instalador transaccional que el checkout. Consulta [Validación](VALIDACION.md) y ejecuta sus pruebas antes de publicar.

## Firma y advertencias de Windows

Para uso propio, se puede ejecutar una compilación local sin firmar según la política del equipo. Para distribuir fuera de GitHub, considera firma Authenticode y publica siempre SHA-256. Una firma no asegura automáticamente reputación SmartScreen para cada descarga nueva; consulta [la guía oficial de SmartScreen](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation).
