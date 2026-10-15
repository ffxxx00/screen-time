# Preparación de una versión personal estable

## Incorporado

- Instalación por usuario en `%LOCALAPPDATA%\Programs\Screen Time`, arranque comprobado con reversión, retirada del ejecutable y accesos directos anteriores, autoarranque conservado, respaldo local previo a la actualización y desinstalación registrada en Windows.
- Checkpoints de 60 segundos, guardado inmediato en cambios de estado, journal recuperable, sincronización del CSV, reintento de escrituras y preferencias atómicas.
- Índice diario incremental para filas propias, verificación de cambios externos y medición local del daemon en bandeja; ver `docs/VALIDACION.md`.
- Notificaciones de bloqueo, desconexión de sesión, suspensión, reanudación y fin de sesión pausan el conteo independientemente del ajuste de inactividad.
- Simulaciones automáticas de eventos de sesión y energía, tiempo antes/durante/después de suspensión y extracción e instalación temporal del ZIP; ejecutadas en CI sin esperas reales.
- Licencia y documentación para un repositorio público en GitHub.

## Validación pendiente antes de afirmar que la versión 1.0 es fiable

- Confirmar Windows 10/11 limpio, instalación, acceso directo, reinicio, actualización y retiro manual.
- Probar con hardware real bloqueo/desbloqueo, suspensión/reanudación, hibernación, inicio de sesión y RDP si se soporta.
- Repetir la recuperación después de cerrar el proceso durante un checkpoint en un equipo de prueba. La prueba automatizada ya cubre cuatro cortes reales de un proceso hijo con historial sintético.
- Repetir mediciones de RAM y CPU en otros equipos, con daemon solo y con dashboard WebView2. Una medición local no garantiza menos de 10 MB ni 0,0 % CPU en todas las configuraciones.
- Probar el ZIP generado con `tools/package-release.ps1` en Windows limpio; ya se probó la instalación desde ZIP y el arranque en el Windows 10 de desarrollo.
- Las simulaciones de energía y del instalador ya son reproducibles; siguen pendientes las señales reales del hardware y la integración con un Windows limpio.
- Revisar licencias/avisos de dependencias transitivas antes de distribuir un bundle.
- Decidir el formato principal de 1.0 mediante una comparación de CSV e índice actual frente a SQLite, incluyendo RAM, tiempos de consulta, apagado abrupto y migración reversible de historiales existentes.
- Definir identidad de aplicación y marcas temporales con UTC para datos nuevos; dos ejecutables distintos pueden compartir `nombre.exe` y el CSV heredado no permite recuperar offsets UTC pasados.

Un checkpoint cada 60 segundos limita la pérdida habitual del segmento activo a aproximadamente un minuto. Fallos físicos, errores persistentes de E/S o apagado durante sincronización todavía pueden causar pérdida de datos.

## Comprobaciones reproducibles

Ver [docs/VALIDACION.md](docs/VALIDACION.md) para pruebas automatizadas y la matriz manual pendiente. Los tests de DOM no sustituyen una prueba visual con WebView2.
