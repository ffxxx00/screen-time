# Arquitectura

## Proceso nativo

`src/main.rs` coordina el loop de eventos Tao, el estado del tracker, el menú de bandeja y las dos ventanas opcionales.

1. `SetWinEventHook` se registra solo para `EVENT_SYSTEM_FOREGROUND`. El callback envía el `HWND` al loop; allí se consulta PID/ruta con `GetWindowThreadProcessId` y `QueryFullProcessImageNameW`.
2. Hooks low level de ratón/teclado envían únicamente notificaciones de actividad. El callback descarta sus parámetros; no los serializa. `GetLastInputInfo` y una espera de plazo único implementan el ajuste opcional de inactividad.
3. `src/lifecycle.rs` crea una ventana Win32 oculta en un hilo pequeño. Atiende cambios de sesión de WTS, mensajes de energía, invocación del dashboard por una segunda instancia y un timer de checkpoint. No muestra ventanas de consola ni usa un bucle `Sleep`.
4. El estado que llega al loop detiene el segmento durante lock, disconnect, suspend o shutdown. Al reanudarse se vuelve a consultar la ventana en primer plano. `PAUSE_AT` limita la contabilización incluso si un checkpoint quedó en la cola antes de la notificación de bloqueo.

La aplicación normalmente espera mensajes Win32. Mientras el dashboard está abierto, su documento manda una solicitud IPC de actualización cada segundo; la lectura y presentación del historial solo ocurre cuando hay una ventana WebView abierta.

## Dashboard y configuración

Wry aloja WebView2 en ventanas Tao. El HTML, CSS, JavaScript y Chart.js están incluidos localmente en `assets/`; no se abre un servidor HTTP ni una URL de Internet. IPC está limitado a actualizar los datos, navegar fechas, abrir/cerrar configuración cambiar preferencias reconocidas, alternar la pausa manual y gestionar datos mediante comandos limitados (abrir carpeta, exportar y borrar con confirmación nativa).

El dashboard agrupa filas CSV en 24 buckets por día. Si una sesión cruza una hora o medianoche, divide sus segundos por la intersección con cada bucket. El gráfico apila las cinco apps principales; «Otras aplicaciones» agrupa el resto de sus barras. La lista y el tooltip conservan el detalle individual. Se guardan las dimensiones lógicas de ambas ventanas.

Al iniciar solo en bandeja y al cerrar la última ventana, `EmptyWorkingSet` pide a Windows retirar páginas residentes del proceso. La memoria privada comprometida no desaparece por este llamado; Windows recarga páginas cuando el tracker vuelve a necesitarlas. No se ejecuta en cada checkpoint.

`src/daily_summary.rs` mantiene `app_usage.daily.json` como índice derivado. Guarda totales por aplicación y hora para cada día, la posición ya procesada del CSV y una huella incremental. Al cargar el índice verifica el prefijo completo. Durante la sesión, antes de anexar una fila el tracker comprueba si hubo cambios externos; después de una escritura confirmada por su journal marca esa cantidad de bytes como confiable y el índice solo lee y calcula la huella de las filas nuevas. Un cambio externo reconstruye el índice. El segmento activo aún no escrito se suma en memoria al mostrar el dashboard. Si se elimina el índice, se regenera desde `app_usage.csv`.

El snapshot del dashboard también agrega siete resúmenes diarios de lunes a domingo para el modo Semana. Envía el total semanal, el promedio diario y su número de días transcurridos desde el inicio de la instalación, los totales por día y las aplicaciones ordenadas por uso semanal. `app_usage.coverage.json` guarda la fecha de inicio y las fechas en que el tracker funcionó, aun sin uso registrado; la escritura es atómica y se realiza como máximo una vez por día. En instalaciones anteriores, la fecha de inicio se infiere de la creación del CSV y se adelanta si existe historial más antiguo. Una fecha con filas CSV también se considera medida para conservar la validez del historial previo. El promedio incluye todos los días desde ese inicio hasta hoy o el domingo de la semana, inclusive: un día sin datos aporta cero, aunque siga marcado como «Sin medición» en la vista diaria. Los días previos al inicio y los futuros no se incluyen. El segmento activo se suma únicamente al snapshot de cada día correspondiente; no modifica el índice ni el CSV. Si el archivo de cobertura no se puede leer, se conserva sin sobrescribir, se registra el problema en `lifecycle.log` y el seguimiento CSV continúa.

`src/icons.rs` consulta los procesos activos por PID para descubrir rutas y usa la API Shell de Windows para leer iconos existentes. Codifica el PNG RGBA como `data:` local para el dashboard; no ejecuta los procesos. Las rutas conocidas se almacenan en `app_usage.paths.json`; el PNG se queda en memoria del WebView abierto. El dashboard asigna una paleta local estable por aplicación durante la sesión, compartida por gráfico, leyenda y barra de progreso; la llegada de iconos no cambia sus colores. La escala muestra minutos y la leyenda incluye las cinco categorías principales y Otras. El inspector fijado ofrece el detalle textual completo; el tooltip muestra hasta cuatro aplicaciones y no captura la rueda.

## Archivos de datos y escritura

Los datos se escriben en `%LOCALAPPDATA%\HorariosTracker`, separados del directorio de instalación. Es un nombre interno heredado que se conserva para que actualizar el nombre del producto no mueva ni duplique tu historial. `src/durability.rs` se ocupa de la sustitución de preferencias y del journal CSV.

`lifecycle.log` registra inicio, solicitud de salida desde bandeja o sesión, finalización del loop y panics sin incluir nombres de aplicaciones ni contenido del historial. Se rota a `lifecycle.previous.log` al llegar a 64 KiB. La falta de una salida registrada entre dos inicios indica una terminación no observada por la aplicación; por sí sola no identifica si fue un cierre forzado, un fallo o un corte de energía. Los volcados de memoria no se activan de forma predeterminada.

Para una fila CSV, primero guarda un JSON temporal con el offset al final actual del historial y los bytes de la fila en base64, sincroniza el temporal y lo reemplaza usando `MoveFileExW` con `MOVEFILE_WRITE_THROUGH`. Después verifica cualquier prefijo existente, escribe lo restante al CSV y llama a `sync_all`. En el arranque, si hay journal, termina esa misma fila antes de iniciar el seguimiento. Comprobar offset y prefijo vuelve idempotente la recuperación de escrituras parciales.

El checkpoint periódico se realiza cada minuto. El cambio de aplicación, el bloqueo, la suspensión, la salida y el plazo de inactividad guardan el segmento inmediatamente. Un cierre abrupto puede perder hasta aproximadamente un minuto del segmento activo. Disco desconectado, errores de E/S persistentes, datos editados manualmente o fallo de hardware pueden ampliar esa pérdida. Si el historial existente no coincide con el journal, el programa se detiene con error para preservar ambos archivos.

El instalador hace una copia de seguridad de `app_usage.*` antes de reemplazar el ejecutable. Registra la desinstalación en Windows; esta pide al daemon que guarde y salga, retira solo archivos y accesos directos propios y conserva el directorio de datos.

## Componentes Win32

- `EVENT_SYSTEM_FOREGROUND`, `SetWinEventHook` y `UnhookWinEvent`.
- `GetWindowThreadProcessId`, `OpenProcess` y `QueryFullProcessImageNameW`.
- `GetLastInputInfo`; hooks `WH_KEYBOARD_LL` y `WH_MOUSE_LL` solo para señal de actividad.
- Ventana Win32, `SetTimer`, notificaciones de WTS y `WM_POWERBROADCAST`.
- Registry de `HKCU` para el inicio automático opcional.
- Shell/GDI para obtener iconos; `rc.exe` para incrustar icono y versión en el `.exe`.
- Tao + Wry + WebView2 para ventanas; Chart.js para barras.

## Límites

Desde 0.3.0 los registros nuevos guardan UTC y el desfase local del inicio; mantienen un instante inequívoco aunque se repita una hora. Los registros antiguos sin zona conservan esa limitación. El gráfico usa el día y la hora del lugar donde se registró el segmento, incluso si después cambia la zona del equipo. El desfase queda fijo para cada segmento (normalmente hasta 60 segundos); un cambio de zona o DST dentro de ese intervalo se refleja al comenzar el siguiente segmento.

La marca diaria indica que el tracker funcionó en algún momento del día; no certifica cobertura continua de 24 horas. Los días históricos sin filas CSV ni marca se muestran como «Sin medición», incluso si en realidad hubo cero uso. Una PC apagada tampoco puede medirse desde esta aplicación.

El loop principal y los callbacks Win32 son parte de una aplicación interactiva de usuario, no de un servicio de Windows. El modo de inicio actual es `HKCU Run`, es decir, se ejecuta en la sesión iniciada por ese usuario. La política de bloqueo y sesión remota debe verificarse en cada configuración Windows que importe.
- En actualizaciones desde versiones anteriores, conserva `%LOCALAPPDATA%\HorariosTracker`, `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\HorariosTracker`, `Local\HorariosTracker.Daemon` y `HorariosTracker.Lifecycle`; son IDs heredados que evitan perder el historial y permiten abrir el dashboard de una instancia ya iniciada.

## Recuperación visual y mantenimiento

`assets/view-state.js` comparte la señal de carga, pantalla de error y reintento. Configuración recibe preferencias sin depender del CSV. Un temporizador Win32 independiente del checkpoint revela una carga que no termina después de 12 segundos.

Las comprobaciones y recuperación de datos están en [VALIDACION.md](VALIDACION.md). La skill del proyecto está en `.agents/skills/screen-time-maintenance/SKILL.md`.

## Correcciones de auditoría de diseño (30/09/2026)

`manual_paused` se persiste en preferencias. Al pausar se guarda el segmento activo; foreground, input, checkpoint y cambios de política de inactividad no crean segmentos mientras la pausa está activa. La reanudación consulta el foreground actual. El snapshot informa `trackingState` (manual, system, idle, active o waiting). Una reanudación de Windows no cancela una pausa manual.

`src/data_management.rs` exporta el CSV tras un checkpoint a una carpeta única de Exportaciones. El borrado exige confirmación nativa con No como opción predeterminada, pausa el tracker y rechaza pendientes de guardado. Guarda `app_usage.reset.json` antes de eliminar archivos conocidos; si se interrumpe, `Tracker::new` completa el borrado antes de recuperar el journal. Conserva preferencias, backups y exportaciones. Las cargas de iconos usan una generación para descartar resultados anteriores al borrado.

El calendario es un popover no modal con botones nativos: un único punto tabulable para fechas o elecciones, flechas, Home/End, foco al abrir/cambiar nivel, cierre al confirmar y retorno al disparador con Escape. Tab puede salir y cierra el popover. El ranking reutiliza nodos al reordenarse y recupera foco por identidad si cambia el conjunto; si desaparece una fila, usa la fila contigua o el selector de fecha. El inspector no es una región de anuncios automáticos cada segundo. El promedio conserva la fórmula y explica días de calendario, ausencia de medición y límites de la marca diaria.


## Segunda pasada de diseño (30/09/2026)

El error guarda el control de origen, enfoca su encabezado solo al entrar y restaura el control si sigue disponible; usa un control válido de la vista cuando desaparece. No altera el foco al repetir el error ni si la persona navegó a otro control visible de Configuración.

La paleta reserva gris exclusivamente para Otras. Antes de dibujar, las cinco aplicaciones del gráfico reciben colores distintos; los cambios se sincronizan con filas, leyenda e inspector. La identidad se conserva dentro del documento cuando no colisiona con otra categoría visible; se prioriza distinguir las categorías simultáneas. No se persiste entre aperturas.

La cobertura visible ocupa una línea contextual. El método completo y el comportamiento al cerrar están en `details` con resumen nativo accesible; su apertura y foco se conservan durante los snapshots. La leyenda solo identifica categorías y las duraciones quedan en la lista. Configuración agrupa información y acciones bajo Qué se guarda, Dónde está y Qué elimina el borrado, con lenguaje de usuario.

## Correcciones de seguimiento y resumen (14 de octubre de 2026)

El escritorio y la barra de tareas (clases Progman, WorkerW, Shell_TrayWnd y Shell_SecondaryTrayWnd) ya no crean segmentos de explorer.exe; las ventanas del Explorador siguen contando. La ventana de lifecycle registra GUID_LIDSWITCH_STATE_CHANGE y GUID_SESSION_DISPLAY_STATUS. Tapa cerrada y pantalla apagada son causas independientes de pausa, acumuladas con bloqueo y suspensión: abrir una no cancela las restantes. Pantalla atenuada permite seguimiento. Referencia: https://learn.microsoft.com/windows/win32/power/power-setting-guids.

Semana compara su promedio diario registrado con el promedio de la semana anterior, usando en ambos casos días de calendario desde el inicio del historial. Sin semana anterior o divisor cero no se presenta un porcentaje. La ayuda está junto a Configuración, superpuesta y limitada al viewport; Escape, clic fuera y foco fuera la cierran. El detalle semanal limita el área de resaltado al ancho de la barra.

La ayuda del encabezado usa un icono de información con tooltip al pasar el cursor o recibir foco. El clic permite fijarlo; Escape y navegación fuera lo cierran. Mantiene el control nativo summary y una descripción asociada mediante aria-describedby.

El tooltip del gráfico principal se adapta a entre una y cuatro filas y se ancla sobre el extremo superior de la columna apilada. El aviso de apps adicionales se muestra al pie; el detalle completo sigue disponible mediante selección. No reserva espacio para filas ausentes ni anima cambios de posición.


## Formato 0.3.0 e instalador ejecutable

`src/history.rs` centraliza identidad, escritura y lectura. Las filas nuevas tienen seis columnas: UTC RFC3339 con microsegundos, ejecutable, segundos, identidad `path:` por ruta normalizada sin distinguir mayúsculas, ruta y desfase UTC en segundos. El lector CSV flexible acepta filas nuevas y antiguas mezcladas. Los datos antiguos no se reescriben ni se atribuyen a una ruta inferida. La identidad distingue ubicaciones; no intenta distinguir binarios reemplazados en una misma ruta ni resolver enlaces a un mismo archivo.

El índice pasa a versión 3 y se reconstruye desde el CSV. La división por día conserva microsegundos al cruzar medianoche. Los IDs se mantienen en gráfico, selección, ranking e iconos; la presentación traduce el nombre y agrega carpeta o «historial anterior» cuando hay colisiones.

`installer/Setup.cs` genera un asistente Windows Forms x64 con .NET Framework de Windows 10/11. `tools/build-setup.ps1` incrusta el ZIP y su SHA-256 en un EXE gráfico sin elevación. Verifica el recurso, valida las rutas al extraer y ejecuta el instalador transaccional con una ventana de progreso. Un mutex impide ejecutar dos asistentes a la vez. La desinstalación permanece registrada en Windows. `--verify` comprueba extracción e integridad sin instalar; `--silent` realiza la misma instalación sin interfaz.

La actualización respalda el historial antes del reemplazo. Si falla el arranque, restaura archivos, accesos directos, Registro y datos originales antes de reiniciar la versión anterior; conserva las escrituras fallidas en el respaldo. Un downgrade manual anterior a 0.3.0 requiere restaurar ese respaldo, porque el lector antiguo no acepta seis columnas.
