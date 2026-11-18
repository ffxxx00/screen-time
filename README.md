# Screen Time

Aplicación para Windows que registra el tiempo de uso de tus aplicaciones y lo muestra en informes diarios y semanales. El historial queda en tu PC: sin cuentas, servidor ni telemetría.

## Instalar

1. Descarga **screen-time-0.3.0-setup-x64.exe** de la sección **Releases** del repositorio cuando esté publicada.
2. Ejecuta el asistente. Instala para tu usuario, sin permisos de administrador.
3. Abre **Screen Time** desde Inicio o el escritorio.

Requiere **Windows 10/11 x64** y **Microsoft Edge WebView2 Runtime**. Si falta WebView2, el asistente muestra cómo instalarlo y permite reintentar. El instalador aún no tiene firma digital.

Como alternativa, extrae el ZIP de la release y ejecuta `Instalar.cmd`. No ejecutes el instalador dentro del ZIP sin extraerlo.

**Estado:** primera release pública en preparación, versión 0.3.0. Las comprobaciones en Windows limpio y otros equipos siguen pendientes; consulta las [limitaciones y validación](docs/VALIDACION.md).

## Uso

- El dashboard muestra uso por día y semana, aplicaciones y detalle por hora.
- Cerrar la ventana mantiene el seguimiento en la bandeja. Doble clic en el icono abre el dashboard; clic derecho permite pausar, reanudar o salir.
- Configuración permite activar el inicio con Windows, elegir tema y pausar tras dos minutos sin entrada.
- Bloqueo, suspensión, tapa cerrada y pantalla apagada pausan el seguimiento.
- Desde **Datos y privacidad** puedes abrir la carpeta, exportar CSV o borrar el historial con confirmación.

## Datos y privacidad

Los programas se instalan en `%LOCALAPPDATA%\Programs\Screen Time`. El historial y las preferencias se guardan por separado en **`%LOCALAPPDATA%\Screen Time`**.

El CSV `app_usage.csv` es la fuente del historial; los índices diarios son regenerables. También se guardan preferencias, cobertura, metadatos e iconos de aplicaciones y un journal de recuperación. Las actualizaciones respaldan los datos en `backups/`.

Se registran identidad, nombre, ruta y duración de uso de aplicaciones. No se guardan títulos de ventanas, teclas, capturas ni contenido del portapapeles. El dashboard funciona sin conexión; WebView2 Evergreen se instala y actualiza por separado. Lee [PRIVACY.md](PRIVACY.md) para el detalle.

Para desinstalar, abre **Aplicaciones instaladas → Screen Time → Desinstalar**. El historial se conserva; las exportaciones y backups se gestionan por separado.

## Limitaciones

El historial refleja solamente actividad observada mientras el tracker está ejecutándose; no reconstruye actividad anterior. Con la pausa por inactividad desactivada, una aplicación en primer plano cuenta aunque no haya entrada.

Los checkpoints se intentan cada minuto. Un cierre inesperado puede perder aproximadamente el último minuto; errores de disco pueden ampliar esa pérdida. La cobertura diaria no certifica medición continua de 24 horas. El reconocimiento de aplicaciones depende de los metadatos disponibles y puede ser incompleto.

## Desarrollo

Requiere Rust estable, Visual Studio Build Tools con C++ y Windows SDK (`rc.exe`). En un prompt **x64 Native Tools**:

```bat
cargo build --release --locked --target x86_64-pc-windows-msvc
cargo test --locked --target x86_64-pc-windows-msvc
```

El binario queda en `target\x86_64-pc-windows-msvc\release\screen-time.exe`. Las pruebas de interfaz usan Node 22 o posterior: `npm ci --ignore-scripts` y `npm test`. La aplicación instalada no requiere Node.

Para generar los archivos de distribución desde PowerShell:

```powershell
.\tools\package-release.ps1
.\tools\build-setup.ps1
```

En `dist/` quedan el instalador EXE, el ZIP y sus archivos SHA-256. Consulta la [guía de publicación](docs/PUBLICAR.md) para los controles y comandos completos.

## Documentación

- [Arquitectura y formato de datos](docs/ARQUITECTURA.md)
- [Validación y recuperación](docs/VALIDACION.md)
- [Preparación de la release](RELEASE_CHECKLIST.md)
- [Cambios](CHANGELOG.md) · [Dependencias](docs/DEPENDENCIAS.md)
- [Contribuir](CONTRIBUTING.md) · [Seguridad](SECURITY.md)

Distribuido bajo [MIT](LICENSE). Chart.js conserva su [licencia MIT](assets/CHARTJS-LICENSE.txt); las dependencias mantienen sus propias licencias.
