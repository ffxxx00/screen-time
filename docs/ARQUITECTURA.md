# Arquitectura

## Proceso nativo

`src/main.rs` coordina el loop de eventos Tao, el estado del tracker, el menú de bandeja y las dos ventanas opcionales.

1. `SetWinEventHook` se registra solo para `EVENT_SYSTEM_FOREGROUND`. El callback envía el `HWND` al loop; allí se consulta PID/ruta con `GetWindowThreadProcessId` y `QueryFullProcessImageNameW`.
2. Hooks low level de ratón/teclado envían únicamente notificaciones de actividad. El callback descarta sus parámetros; no los serializa. `GetLastInputInfo` y una espera de plazo único implementan el ajuste opcional de inactividad.
3. `src/lifecycle.rs` crea una ventana Win32 oculta en un hilo pequeño. Atiende cambios de sesión de WTS, mensajes de energía, invocación del dashboard por una segunda instancia y un timer de checkpoint. No muestra ventanas de consola ni usa un bucle `Sleep`.
4. El estado que llega al loop detiene el segmento durante lock, disconnect, suspend o shutdown. Al reanudarse se vuelve a consultar la ventana en primer plano. `PAUSE_AT` limita la contabilización incluso si un checkpoint quedó en la cola antes de la notificación de bloqueo.

La aplicación normalmente espera mensajes Win32. Mientras el dashboard está abierto, su documento manda una solicitud IPC cada segundo en Día y una vez por minuto en la semana en curso. Para una semana terminada, solo hay una consulta al navegar o cuando un evento de la aplicación requiere actualizar la vista. El panel omite los renders cuando solo cambia la hora de actualización; la lectura y presentación del historial solo ocurre cuando hay una ventana WebView abierta.

## Dashboard y configuración

Wry aloja WebView2 en ventanas Tao. El HTML, CSS, JavaScript y Chart.js están incluidos localmente en `assets/`; no se abre un servidor HTTP ni una URL de Internet. IPC está limitado a actualizar los datos, navegar fechas, abrir/cerrar configuración cambiar preferencias reconocidas, alternar la pausa manual y gestionar datos mediante comandos limitados (abrir carpeta, exportar y borrar con confirmación nativa).

El dashboard agrupa filas CSV en 24 buckets por día. Si una sesión cruza una hora o medianoche, divide sus segundos por la intersección con cada bucket. El gráfico apila las cinco apps principales; «Otras aplicaciones» agrupa el resto de sus barras. La lista y el tooltip conservan el detalle individual. Se guardan las dimensiones lógicas de ambas ventanas.

Al iniciar solo en bandeja y al cerrar la última ventana, `EmptyWorkingSet` pide a Windows retirar páginas residentes del proceso. La memoria privada comprometida no desaparece por este llamado; Windows recarga páginas cuando el tracker vuelve a necesitarlas. No se ejecuta en cada checkpoint.

`src/daily_summary.rs` mantiene `app_usage.daily.json` como índice derivado. Guarda totales por aplicación y hora para cada día, la posición ya procesada del CSV y una huella incremental. Al cargar el índice verifica el prefijo completo. Durante la sesión, antes de anexar una fila el tracker comprueba si hubo cambios externos; después de una escritura confirmada por su journal marca esa cantidad de bytes como confiable y el índice solo lee y calcula la huella de las filas nuevas. Un cambio externo reconstruye el índice. El segmento activo aún no escrito se suma en memoria al mostrar el dashboard. Si se elimina el índice, se regenera desde `app_usage.csv`.

El snapshot del dashboard también agrega siete resúmenes diarios de lunes a domingo para el modo Semana. Envía el total semanal, el promedio diario y su número de días transcurridos desde el inicio de la instalación, los totales por día y las aplicaciones ordenadas por uso semanal. `app_usage.coverage.json` guarda la fecha de inicio y las fechas en que el tracker funcionó, aun sin uso registrado; la escritura es atómica y se realiza como máximo una vez por día. En instalaciones anteriores, la fecha de inicio se infiere de la creación del CSV y se adelanta si existe historial más antiguo. Una fecha con filas CSV también se considera medida para conservar la validez del historial previo. El promedio incluye todos los días desde ese inicio hasta hoy o el domingo de la semana, inclusive: un día sin datos aporta cero, aunque siga marcado como «Sin medición» en la vista diaria. Los días previos al inicio y los futuros no se incluyen. El segmento activo se suma únicamente al snapshot de cada día correspondiente; no modifica el índice ni el CSV. Si el archivo de cobertura no se puede leer, se conserva sin sobrescribir, se registra el problema en `lifecycle.log` y el seguimiento CSV continúa.

`src/icons.rs` consulta los procesos activos por PID para descubrir rutas y usa la API Shell de Windows para leer iconos existentes. Codifica el PNG RGBA como `data:` local para el dashboard; no ejecuta los procesos. Las rutas conocidas se almacenan en `app_usage.paths.json`; el PNG se queda en memoria del WebView abierto. El dashboard extrae el color dominante del icono, ignorando transparencias, fondos blancos y bordes casi negros. Comparte el tono entre gráfico, leyenda y barra de progreso; lo guarda en icon_colors de las preferencias mediante IPC validado y escritura atómica para recuperarlo antes del primer render al reabrir. Sin icono usa un tono determinista por identidad. La escala muestra minutos y la leyenda incluye las cinco categorías principales y Otras. El inspector fijado ofrece el detalle textual completo; el tooltip muestra hasta cuatro aplicaciones y no captura la rueda.

## Archivos de datos y escritura

Los datos se escriben en `%LOCALAPPDATA%\Screen Time`, separados del directorio de instalación. `src/durability.rs` se ocupa de la sustitución de preferencias y del journal CSV.

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

## Recuperación visual y mantenimiento

`assets/view-state.js` comparte la señal de carga, pantalla de error y reintento. Configuración recibe preferencias sin depender del CSV. Un temporizador Win32 independiente del checkpoint revela una carga que no termina después de 12 segundos.

Las comprobaciones y recuperación de datos están en [VALIDACION.md](VALIDACION.md).

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


## Colores de iconos restaurados — 15 de octubre de 2026

Esta decisión sustituye la paleta fija de la auditoría del 30 de septiembre: se prioriza la asociación entre icono y aplicación solicitada por el usuario. Los iconos semejantes pueden compartir tono; nombres, iconos y detalle textual mantienen la identificación. Los colores se conservan en las preferencias (hasta 512 entradas), no dependen del ranking.


## Identidad de aplicaciones alojadas — 15 de octubre de 2026

`src/app_identity.rs` consulta en un hilo COM propio el AppUserModelID explícito de ventana o de aplicación empaquetada y el nombre declarado o del Shell. Para Java analiza localmente la clase principal, módulo o JAR mediante WMI; usa el manifiesto del JAR cuando está disponible. No depende de rutas de JVM ni contiene reglas de Minecraft u otros juegos. El ejecutable es el último recurso. Las respuestas se validan por generación de foreground, PID y fecha de creación para evitar reutilización de PID y resultados tardíos. Las consultas y los mensajes de icono no bloquean el loop de seguimiento.

Los registros reconocidos tienen siete columnas: las seis existentes (con identidad app: en la cuarta) más nombre visible. Las filas de tres y seis columnas conservan su interpretación; no se reescribe ni reclasifica historial anterior. El catálogo app_usage.apps.json guarda nombres e iconos PNG de ventana; los nombres se recuperan del CSV al iniciar. El borrado de historial elimina también este catálogo. La identidad semántica se mantiene si cambia la JVM; una clase de arranque compartida, metadatos ausentes, argfiles o JAR relativos pueden limitar el reconocimiento. No se registran títulos de ventanas ni comandos completos. Las versiones anteriores no interpretan las filas de siete columnas.

## Compatibilidad de agrupación — 15 de octubre de 2026

La identidad AppUserModelID se usa solamente para ejecutables anfitriones compartidos (Java, Python, Node, Electron genérico y hosts de Windows). Las aplicaciones nativas normales conservan la identidad por ruta; los perfiles de un navegador no crean aplicaciones distintas. grouping.rs reconstruye asociaciones de presentación desde el CSV: un nombre antiguo se une a una ruta solo cuando existe una única candidata no anfitriona; los AppUserModelID nativos ya guardados se unen a su ruta cuando es inequívoca. Día y Semana suman los segmentos por esa identidad sin modificar CSV ni el índice base. Nombres homónimos con rutas diferentes y aplicaciones alojadas permanecen separados.

## Reconocimiento de ejecutables trasladados — 15 de octubre de 2026

Los ejecutables nativos se identifican mediante SHA-256 en el worker de identidad, con lectura por bloques, límite de 512 MiB y caché invalidada por tamaño/fecha. No se bloquea el seguimiento. Un archivo idéntico conserva app:native:<huella inicial> aunque cambie de ruta, nombre o unidad. El catálogo apps.json guarda las huellas y ubicaciones asociadas; una actualización en una ubicación conocida conserva su identidad (como la antigua identidad por ruta). Una coincidencia exacta con otra app conocida prevalece y las rutas ambiguas no reclasifican filas antiguas. Cambiar simultáneamente versión y ubicación sin una huella conocida no se une automáticamente.

Java y los ejecutores compartidos quedan excluidos; también se consulta OriginalFilename para excluir runtimes renombrados. Los metadatos de versión solo excluyen hosts: no son prueba para unir. No se implementan coincidencias por editor/producto, familia de paquete ni controles manuales de unión en esta entrega. No existe una identificación universal para todo launcher sin metadatos.

Las filas nuevas app:native: llevan nombre visible y ruta; el catálogo se reconstruye parcialmente desde ellas si falta. Las filas anteriores permanecen intactas: grouping resuelve sus rutas a la identidad estable, y une el nombre antiguo cuando sus candidatas ya pertenecen a la misma app. El catálogo conserva huellas después de desaparecer la ruta original. Se necesitan observaciones previas para reconocer un archivo ya desaparecido; no se deduce su contenido a partir del nombre. Borrar historial elimina estas asociaciones junto al catálogo.

## Actualizaciones y nombres Java — 4 de noviembre de 2026

La agrupación de presentación reconoce carpetas de versión MSIX conservando paquete, editor, arquitectura y ruta relativa, y carpetas Squirrel app-N.N.N cuyo directorio de instalación coincide con el ejecutable. Las versiones de una misma instalación se suman en Día y Semana, incluido el historial anterior. Raíces distintas y editores distintos permanecen separados. La reconciliación reutiliza esta evidencia para registros futuros; las huellas siguen reconociendo traslados. No se agrupa solamente por nombre visible.

Los alias de las clases org.prismlauncher.EntryPoint y com.threerings.projectx.client.ProjectXApp muestran Minecraft (Prism Launcher) y Spiral Knights cuando el nombre guardado era la clase técnica. Conservan nombres declarados, iconos e identidades. Se aplican al resolver y reconstruir el catálogo desde el CSV, sin reescribirlo ni fusionar otras herramientas Java con los juegos.

## Reconocimiento general de aplicaciones — 4 de noviembre de 2026

Esta revisión sustituye los alias específicos de juegos de la sección anterior: no hay clases ni marcas de aplicaciones codificadas en el reconocedor. El worker consulta FileDescription/ProductName para nombres nativos y OriginalFilename para reconocer runtimes renombrados. La evidencia de versión sigue separada de la evidencia de identidad: un nombre comercial coincidente nunca basta para fusionar aplicaciones.

Para Java se leen los títulos Implementation-Title, Specification-Title, JavaFX-Application-Name y Bundle-Name del manifiesto que contiene la clase de entrada, incluso si no declara Main-Class. Las dependencias sin esa clase no aportan nombres. También se admiten directorios de clases y propiedades JVM explícitas de nombre de aplicación, limitadas en longitud y anteriores a los argumentos de la aplicación. Las lecturas permanecen en el worker, limitadas a 64 entradas de classpath y 64 KiB por manifiesto. No se ejecutan archivos ni se guardan comandos completos o títulos de ventana.

Sin nombre declarado se presenta una derivación legible de clase/módulo, conservando su identidad. No se infiere una marca comercial. Los nombres enriquecidos del catálogo no se degradan al leer filas históricas con nombres técnicos. Las versiones agrupadas comparten el mejor nombre nativo y un icono disponible, sin copiar ni fusionar el catálogo fuente; los nombres de perfiles del navegador se excluyen de ese enriquecimiento.

La agrupación Squirrel reconoce app-N.N.N aunque el directorio de instalación y el ejecutable tengan nombres distintos y aunque haya subdirectorios. Conserva la raíz y la ruta relativa para separar instalaciones y herramientas distintas. Se mantiene la agrupación MSIX por paquete, editor, arquitectura y ejecutable. No pretende reconocer todos los instaladores posibles. Argfiles, classpaths relativos sin contexto, comodines y clases de arranque compartidas sin metadatos siguen siendo límites; el historial que nunca guardó esos metadatos no permite reconstruir con certeza el nombre comercial.
