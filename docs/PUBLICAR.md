# Publicar Screen Time

La primera release pública se prepara como **0.3.0** para Windows 10/11 x64. Esta guía describe el proceso; no implica que ya se haya publicado ni probado en Windows limpio.

## Revisar el repositorio

- Revisar `git status`, diff y archivos rastreados. No incluir historiales, backups, exportaciones, credenciales ni rutas personales. `.gitignore` no elimina archivos ya rastreados ni secretos del historial Git.
- Revisar también el historial Git antes de cambiar la visibilidad del repositorio.
- Mantener versión coherente en `Cargo.toml`, `Cargo.lock`, `assets/app.rc` y `CHANGELOG.md`.
- Revisar LICENSE, privacidad y avisos de dependencias, incluidas las transitivas. Los informes de auditorías anteriores son evidencia histórica, no validación de esta compilación.

## Compilar y comprobar

Desde un entorno x64 Native Tools con Rust, Windows SDK y Node 22 o posterior:

```powershell
cargo fmt --all -- --check
cargo test --locked --target x86_64-pc-windows-msvc
cargo build --release --locked --target x86_64-pc-windows-msvc
npm ci --ignore-scripts
npm test
.\tests\install.test.ps1
.\tests\uninstall.test.ps1
.\tools\package-release.ps1
.\tools\build-setup.ps1
.\tests\setup.test.ps1
.\tests\package.test.ps1
```

El empaquetado incluye documentación técnica seleccionada, licencias y scripts de instalación; excluye capturas e informes históricos de auditoría. El asistente EXE incrusta el ZIP y comprueba su integridad antes de instalar. Las pruebas de paquete y rollback usan carpetas temporales y datos sintéticos.

Comprobar además la matriz manual de [VALIDACION.md](VALIDACION.md), especialmente instalación en Windows limpio, WebView2 ausente, apertura, reinicio, autoarranque, actualización, desinstalación y energía. No sustituirla por tests unitarios.

## Archivos de la release

Publicar juntos estos cuatro archivos de `dist/`:

- `screen-time-0.3.0-setup-x64.exe`
- `screen-time-0.3.0-setup-x64.exe.sha256`
- `screen-time-0.3.0-windows-x64.zip`
- `screen-time-0.3.0-windows-x64.zip.sha256`

Para comprobar una descarga: `Get-FileHash .\screen-time-0.3.0-setup-x64.exe -Algorithm SHA256` y comparar con el archivo `.sha256` de la misma release. Los hashes detectan cambios; no sustituyen una firma de autor.

## Publicación en GitHub

1. Terminar las comprobaciones y registrar los resultados reales en VALIDACION.md. Marcar pendientes sin atribuirles resultados.
2. Crear el commit definitivo y la etiqueta `v0.3.0` sobre ese commit; compilar y empaquetar esa misma fuente.
3. Crear un borrador de release y adjuntar los cuatro archivos. Los archivos se generan localmente con los comandos anteriores.
4. Incluir funciones, Windows x64, requisito WebView2, instrucciones de instalación/desinstalación, conservación de datos, ausencia de firma y limitaciones conocidas.
5. Verificar descargas y hashes antes de publicar el borrador. Si la validación requerida sigue pendiente, mantener el borrador o declarar explícitamente una versión preliminar.

La aplicación y el instalador están sin firma Authenticode salvo que el mantenedor los firme por separado. Si se firman, regenerar los hashes de los archivos finales y repetir su comprobación.
