# Registro de cambios

Los cambios notables de este proyecto se documentan aquí. El historial anterior a esta documentación no fue reconstruido a partir de commits publicados.

## 0.3.0 — En desarrollo

- Instalador gráfico EXE por usuario con verificación del paquete, progreso, actualización y recuperación de archivos y datos.
- Registros nuevos con instante UTC, microsegundos y desfase de hora local; lectura compatible del historial antiguo sin reescritura.
- Identidad por ruta para separar aplicaciones homónimas y distinguir sus iconos, ranking y selección.
- Índice diario versión 3 y división precisa de segmentos fraccionarios al cruzar medianoche.
- Pruebas de historial mixto, cambios horarios, homónimos y setup con paquete alterado.
- Pruebas reales de hardware y varios días de funcionamiento confirmados por el usuario.

## 0.2.0

Correcciones del 14/10/2026:
- Exclusión del escritorio y la barra de tareas del conteo de explorer.exe, conservando las ventanas del Explorador.
- Pausa por tapa cerrada o pantalla apagada, independiente de suspensión y bloqueo.
- Comparación semanal del promedio diario mediante flecha y porcentaje respecto de la semana anterior.
- Ayuda contextual mediante icono de información y tooltip con soporte de teclado y clic.
- Resaltado del detalle semanal limitado al ancho de cada barra.
- Tooltip principal con altura según contenido, colores por categoría y posición sobre la columna.
- Nombre Screen Time en el Administrador de tareas.
- Publicación inicial con historial reconstruido; véase docs/HISTORIAL-RECONSTRUIDO.md.

Segunda pasada de auditoría del 30/09/2026:
- Foco visible al entrar en error y recuperación al resolverlo, sin saltos durante reintentos automáticos.
- Cinco categorías simultáneas con colores distintos y gris exclusivo de Otras; identidad limitada a la consulta.
- Ayuda de metodología desplegable, cobertura breve y leyenda sin duraciones repetidas; más espacio para apps en ventanas pequeñas.
- Privacidad agrupada con lenguaje claro y avisos de retención junto a las acciones.

Correcciones de auditoría del 30/09/2026:
- Estado explícito del seguimiento y pausa/reanudación persistente desde dashboard y bandeja.
- Promedio diario registrado con divisor y aviso de cobertura incompleta, conservando la fórmula.
- Ranking con foco estable, descripciones accesibles con duración, calendario con teclado y selección de contraste corregido.
- Escala en minutos y leyenda completa, colores estables y detalle extenso en inspector fijo.
- Datos y privacidad dentro de Configuración, exportación CSV y borrado confirmado con recuperación de interrupciones.
- Duraciones inferiores a un minuto, indicadores de expansión, regreso al período actual y errores con detalles copiables y acceso a Configuración.


- Simulaciones automáticas de bloqueo, desconexión, suspensión, reanudación automática y cancelación del apagado; prueba del tiempo excluido durante suspensión sin esperas reales.
- Verificación del ZIP Release y ensayo de su contenido con una instalación temporal en CI.
- Desinstalación registrada en Windows que detiene la app de forma ordenada, retira archivos y accesos directos propios y conserva el historial local.
- Prueba automatizada en un proceso separado que termina durante cuatro fases de un checkpoint y comprueba recuperación sin duplicación.
- Nuevo nombre público **Screen Time** y ejecutable `screen-time.exe`; migración desde Tiempo en pantalla sin mover el historial ni perder el autoarranque.
- Checkpoint periódico cada minuto, con guardado inmediato en cambios de aplicación y estado del sistema; temporizador independiente para el aviso de carga de WebView2.
- Comprobación previa de WebView2 Runtime en el instalador, con pruebas de ausencia, versión inválida e instalaciones por usuario o máquina.
- Ensayo reproducible con historiales sintéticos de uno, tres y cinco años para medir carga y actualización del índice.
- Índice diario incremental para las escrituras propias, con verificación completa al cargar o detectar cambios externos y límite para duraciones CSV corruptas.
- Instalador con comprobación del arranque, reversión si el proceso falla y retirada segura del ejecutable y accesos directos anteriores.
- Detección de la ventana de sesión por enumeración Win32; una segunda invocación vuelve a abrir el dashboard. Liberación de memoria residente al volver al modo bandeja.
- Estados de error visibles y reintento; Configuración independiente del historial y aviso si la carga no termina.
- Índice diario versión 2 con comprobación del contenido completo, liberación al cerrar el dashboard y regresiones automatizadas.
- Pruebas de interfaz con jsdom, recuperación del journal y restauración del instalador; CI de Windows ampliado.
- Instalación desde ZIP, actualización con reversión, retención acotada de copias, empaquetado con SHA-256 y skill de mantenimiento.
- Índice diario persistente y regenerable para procesar solo filas nuevas del CSV en las actualizaciones del dashboard.
- Carga de iconos en un hilo de trabajo después de mostrar el dashboard para reducir el tiempo de apertura.
- Apertura del dashboard y Configuración después de cargar el tema y el primer conjunto de datos, sin pantalla blanca inicial.
- Doble clic izquierdo sobre el icono de bandeja para abrir el dashboard.
- Instalación por usuario en una ruta fija, accesos directos de escritorio/menú Inicio, conservación del autoarranque y respaldo local antes de actualizar.
- Checkpoints Win32 periódicos, journal idempotente para recuperar la fila CSV incompleta y escrituras atómicas de preferencias/rutas.
- Pausa automática durante bloqueo, desconexión de sesión, suspensión, reanudación automática y cierre de sesión.
- Renombre anterior del producto a Tiempo en pantalla, conservando rutas de datos y autoarranque anteriores.
- Documentación pública de arquitectura, privacidad, dependencias y contribuciones.

## 0.1.0 — Horarios Tracker

- Seguimiento de aplicaciones en primer plano y registros CSV locales.
- Icono de bandeja y dashboard con gráficos por día y hora.
- Configuración de pausa por inactividad, modo nocturno e inicio de Windows.
- Iconos de aplicaciones y diseño compacto adaptable.
