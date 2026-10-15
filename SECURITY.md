# Seguridad y reporte responsable

## Datos y límites de confianza

Screen Time registra localmente ejecutables y tiempos de uso del equipo del usuario. No envía datos a los mantenedores. Trata `app_usage.csv`, `app_usage.paths.json`, las preferencias y los datos de procesos como información personal: no los pegues en issues públicos.

El dashboard ejecuta Chart.js local mediante WebView2 y se comunica con el proceso nativo a través de IPC de Wry. No agregues URLs remotas, navegación web ni endpoints locales sin discutir antes el impacto en privacidad y seguridad.

El tracker consulta ejecutables de otros procesos para leer su ruta e icono. Los ejecutables de terceros no se lanzan ni se cargan en el proceso. El journal de escritura puede contener la última fila pendiente del historial y hereda la sensibilidad del CSV.

## Cómo reportar

Para un problema, publica un issue sin datos personales, binarios de usuario, rutas privadas ni registros completos. Incluye pasos reproducibles, compilación/commit y versión de Windows. Para un problema que pueda facilitar acceso a datos locales de otra sesión, ejecución remota, bypass de controles o corrupción maliciosa del historial, no publiques detalles explotables antes de coordinar una corrección. Hasta configurar un canal privado de seguridad en GitHub, contacta al mantenedor por el mecanismo privado disponible en su perfil de GitHub.

Este proyecto no ofrece un SLA ni una política de respuesta de seguridad.

## Mantenimiento

Descarga Rust crates de sus publicadores fijados en `Cargo.lock`; conserva el runtime oficial de WebView2. Revisa cambios de dependencias antes de actualizar. No hay servidor, servicio privilegiado ni protocolo de red propio.
