# Preparación de una versión personal estable

## Incorporado

- Instalación por usuario en `%LOCALAPPDATA%\Programs\Screen Time`, arranque comprobado con reversión, retirada del ejecutable y accesos directos anteriores, autoarranque conservado, respaldo local previo a la actualización y desinstalación registrada en Windows.
- Checkpoints de 60 segundos, guardado inmediato en cambios de estado, journal recuperable, sincronización del CSV, reintento de escrituras y preferencias atómicas.
- Índice diario incremental para filas propias, verificación de cambios externos y medición local del daemon en bandeja; ver `docs/VALIDACION.md`.
- Notificaciones de bloqueo, desconexión de sesión, suspensión, reanudación y fin de sesión pausan el conteo independientemente del ajuste de inactividad.
- Simulaciones automáticas de eventos de sesión y energía, tiempo antes/durante/después de suspensión y extracción e instalación temporal del ZIP; ejecutadas en CI sin esperas reales.
- Licencia y documentación para un repositorio público en GitHub.

## Incorporado en 0.3.0

- Asistente EXE por usuario, actualización con respaldo y recuperación de datos, y desinstalación desde Windows.
- UTC con microsegundos y desfase original para registros nuevos.
- Identidad por ruta, nombres homónimos separados y lectura del historial anterior sin reescritura.
- Validación de hardware y varios días de funcionamiento confirmados por el usuario el 15/10/2026; evidencia declarada, no una repetición de pruebas por el agente.

## Validación pendiente antes de afirmar que la versión 1.0 es fiable

- Confirmar Windows 10/11 limpio, instalación, acceso directo, reinicio, actualización y retiro manual.
- Extender las comprobaciones de energía a otros equipos y a RDP si se pretende soportarlo. El usuario confirmó el funcionamiento en su hardware y varios días de uso el 15/10/2026.
- Repetir la recuperación después de cerrar el proceso durante un checkpoint en un equipo de prueba. La prueba automatizada ya cubre cuatro cortes reales de un proceso hijo con historial sintético.
- Repetir mediciones de RAM y CPU en otros equipos, con daemon solo y con dashboard WebView2. Una medición local no garantiza menos de 10 MB ni 0,0 % CPU en todas las configuraciones.
- Probar el ZIP generado con `tools/package-release.ps1` en Windows limpio; ya se probó la instalación desde ZIP y el arranque en el Windows 10 de desarrollo.
- La integración con un Windows limpio y otros equipos sigue pendiente; las pruebas reales del equipo del usuario están confirmadas.
- Revisar licencias/avisos de dependencias transitivas antes de distribuir un bundle.
- Mantener la fuente CSV y el índice regenerable; considerar SQLite solo ante una necesidad medida.
- Verificar la versión 0.3.0 y su nuevo formato en otros equipos. UTC e identidad por ruta ya están implementados; el historial antiguo conserva sus limitaciones.

Un checkpoint cada 60 segundos limita la pérdida habitual del segmento activo a aproximadamente un minuto. Fallos físicos, errores persistentes de E/S o apagado durante sincronización todavía pueden causar pérdida de datos.

## Comprobaciones reproducibles

Ver [docs/VALIDACION.md](docs/VALIDACION.md) para pruebas automatizadas y la matriz manual pendiente. Los tests de DOM no sustituyen una prueba visual con WebView2.
