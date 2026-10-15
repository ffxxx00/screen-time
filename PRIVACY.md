# Privacidad

Screen Time crea un historial de uso de aplicaciones en primer plano en el mismo PC. Está pensado para una cuenta local de Windows y no es un sistema de supervisión de otras personas.

## Qué registra

- Instante UTC y desfase de la hora local de cada segmento nuevo; los registros anteriores conservan su fecha local.
- Nombre y ruta normalizada del ejecutable en primer plano, usados para distinguir aplicaciones homónimas.
- Duración del segmento en segundos.
- Ruta local del ejecutable, guardada para buscar y mostrar su icono.
- Preferencias de pantalla, pausa opcional por inactividad, tamaños de ventanas y estado de inicio automático en el Registro del usuario.

El tracker observa eventos de teclado y ratón solo para saber cuándo hubo actividad. No almacena los códigos de teclas, texto escrito, coordenadas ni identificadores del hardware de entrada. El uso de `GetLastInputInfo` permite aplicar la pausa opcional por inactividad.

## Dónde están los datos

Historial, fechas de medición, rutas, journal y preferencias: `%LOCALAPPDATA%\HorariosTracker`. Esta carpeta conserva su nombre anterior por compatibilidad con instalaciones existentes. El instalador deja copias previas en su carpeta `backups`. El Registro `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` contiene solo el comando para iniciar la app, si esa opción está activa.

## Red y terceros

La aplicación no implementa telemetría, sincronización, API remota, servidor HTTP local ni cliente de red para transmitir el historial. WebView2 es el runtime de Microsoft Edge usado para dibujar el dashboard y puede actualizarse conforme a su instalación Evergreen. Chart.js es local y está incluido en el repositorio.

## Control del usuario

Desactiva **Iniciar con Windows** desde Configuración para retirar el inicio automático. **Salir** cierra el tracker. Al desinstalar, los archivos de historial se conservan en LocalAppData hasta que el usuario decida eliminarlos.

Desde el dashboard o la bandeja podés **Pausar/Reanudar seguimiento**. La pausa manual se conserva al reiniciar; cerrar el dashboard no detiene el registro. **Configuración → Datos y privacidad** explica qué se guarda y ofrece abrir la carpeta, exportar el CSV y borrar el historial con confirmación nativa. El borrado elimina tiempos, cobertura, índice y rutas conocidas, conserva preferencias y deja el tracker pausado. Las exportaciones y copias anteriores del instalador se conservan y pueden gestionarse desde la carpeta de datos.
