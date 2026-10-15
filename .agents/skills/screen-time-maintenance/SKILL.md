---
name: screen-time-maintenance
description: Mantiene el tracker Windows Screen Time, sus resúmenes CSV, WebView2 e instalador. Aplicar al modificar seguimiento, persistencia, dashboard o empaquetado de este proyecto.
---

# Mantenimiento de Screen Time

Lee `docs/ARQUITECTURA.md` y `docs/VALIDACION.md` del repositorio para localizar componentes y comandos. Usa el checkout actual; la ruta de la carpeta puede cambiar.

- El CSV es la fuente de verdad. Los resúmenes diarios son derivados y deben poder borrarse y regenerarse. El segmento activo se suma solo a la vista, sin duplicarlo en el índice.
- Conserva el journal idempotente y las fracciones de segundo. Prueba recuperación parcial, conflicto y medianoche cuando cambies persistencia.
- El dashboard se crea bajo demanda, con tema inicial y contenido listo antes de mostrarse. Los errores también deben revelar la ventana. Configuración debe funcionar aunque falle el CSV.
- La interfaz no requiere Node ni servidor en ejecución; npm y jsdom son herramientas de desarrollo. Los iconos se extraen después del primer render.
- Conserva `%LOCALAPPDATA%\HorariosTracker` y la entrada `HorariosTracker` del Registro por compatibilidad.
- Prueba el instalador en una carpeta temporal con fallos inyectados. La instalación real debe cerrar ordenadamente el tracker, respaldar datos y restaurar archivos/accesos directos si falla.
- Ejecuta las comprobaciones relevantes de `docs/VALIDACION.md` y actualiza su evidencia. No presentes pruebas unitarias como validación de suspensión, hibernación o Windows limpio.
- Usa historiales sintéticos en pruebas, issues y CI. No copies datos personales al repositorio ni a los paquetes.
