# Validación y recuperación

## Redimensionado continuo por altura (2 de octubre de 2026)

Se eliminaron todos los cortes CSS por altura (580 y 740 px) que alternaban de golpe densidad, tamaños y espacios. El encabezado, total, gráfico, leyenda, filas y calendario ahora usan tamaños acotados con `clamp` y valores continuos según la altura disponible. Se mantienen los límites mínimos y máximos para legibilidad.

`npm test` aprobó 35 pruebas. Se revisaron datos sintéticos con Chart.js real en iframes de 434 px de ancho y alturas de 480, 580, 581, 740, 741 y 820 px. Entre 580/581, el inicio del gráfico cambió de 255.546875 a 255.671875 px y el inicio de la primera fila de 454.921875 a 455.34375 px. Entre 740/741, la primera fila cambió de 544.328125 a 544.890625 px. El total cambia de 31.9 a 31.955 px entre 580/581, sin salto de fuente. Se revisó el render en el navegador de Codex.

Compilación Release e instalación local aprobadas, con coincidencia de SHA-256. Se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Duraciones inferiores a un minuto (2 de octubre de 2026)

Las duraciones positivas menores de un minuto ahora usan `<1 min` en lista, tooltip y detalle, mediante el mismo formateador. Cero sigue siendo `0 min`. `npm test` aprobó 35 pruebas, incluida la verificación de actividad menor de un minuto y su selección; sintaxis JavaScript aprobada.

Compilación Release e instalación local aprobadas; SHA-256 compilado e instalado coinciden. Se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Calendario: cierre prematuro por foco (2 de octubre de 2026)

Se reprodujo con clic real en un iframe de 360 × 480: al pulsar Elegir mes, Chromium ejecutaba la microtarea de `focusout` antes de completar el cambio de foco; el calendario se ocultaba y el clic terminaba en el contenido inferior. El cierre por foco ahora atiende `focusin` del destino real, sin comprobar `activeElement` durante ese intervalo. Clic fuera, foco fuera, Escape y confirmación de fecha conservan su comportamiento.

`npm test` aprobó 35 pruebas, incluida la regresión con pérdida transitoria de foco antes del clic interno, navegación de meses/años y cierre fuera. Sintaxis JavaScript aprobada. Se recorrió con clics reales en el navegador de Codex: abrir calendario, elegir mes, elegir año, volver a meses, elegir Agosto, avanzar a Septiembre y confirmar el día 29. Las operaciones internas conservaron el calendario abierto; la confirmación lo cerró y devolvió el foco al selector.

Compilación Release e instalación local aprobadas, con coincidencia de SHA-256. Se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Navegación de fechas sin fila adicional (2 de octubre de 2026)

Se eliminó el botón `current-period` del encabezado y sus actualizaciones. Volver a hoy/esta semana continúa disponible en el calendario superpuesto. Una fecha anterior no agrega controles al flujo de la página.

`npm test` aprobó 34 pruebas y la sintaxis JavaScript aprobó. Con datos sintéticos iguales en dos iframes de 360 × 560, hoy y el día anterior tuvieron exactamente las mismas posiciones: encabezado de 116 px, gráfico en 233.46875 px, encabezado de Más usadas en 385.28125 px y lista en 413.125 px. Se revisó la comparación visual en el navegador de Codex.

Compilación Release e instalación local aprobadas, con coincidencia de SHA-256. Se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Contorno uniforme de las barras (2 de octubre de 2026)

El gráfico redondea el contorno completo de cada columna mediante un recorte de canvas, en lugar de limitar el radio a la altura del segmento superior. Día y Semana mantienen un radio de hasta 4 px en los extremos exteriores; las uniones internas de las aplicaciones se mantienen continuas. El límite se calcula sobre la geometría visible de la pila y el área del gráfico. Las barras del detalle por app también tienen extremos redondeados.

`npm test` aprobó 34 pruebas y `node --check assets/dashboard.js` aprobó. Se revisaron visualmente en el navegador de Codex con Chart.js real, dentro de un iframe de 400 × 560, columnas de una sola aplicación, varias aplicaciones, total de 60 min, uso mínimo y segmentos superiores de 0.05 min (Día) y 0.1 min (Semana). El contorno permanece redondeado con esos segmentos pequeños.

Compilación Release e instalación local aprobadas; SHA-256 compilado e instalado coinciden. Se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Horas sin actividad sin interacción (2 de octubre de 2026)

En Día, las horas sin uso quedan fuera de los puntos seleccionables de Chart.js y el controlador rechaza sus clics. Las flechas recorren únicamente horas con actividad; Home y End eligen la primera y la última con datos. El detalle por aplicación deshabilita sus horas vacías, sin hover, foco ni selección. Si una actualización elimina los datos de una hora seleccionada, se limpia esa selección. Los valores positivos menores de un minuto siguen siendo seleccionables. Semana conserva la navegación de días pasados sin medición.

`npm test` aprobó 34 pruebas; la regresión cubre clic en vacío antes y después de seleccionar, saltos entre 09:00 y 14:00, Home/End, detalle por app y retirada de datos durante una actualización. `node --check assets/dashboard.js` aprobó.

Compilación Release e instalación local aprobadas. Se verificó coincidencia de SHA-256, se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Días futuros sin interacción (2 de octubre de 2026)

El gráfico semanal omite los puntos futuros de los datasets y rechaza sus tooltips. Los días futuros del detalle por aplicación están deshabilitados, fuera del foco y sin etiquetas de estado ni resaltado. Las flechas y End terminan en el último día ocurrido. Se eliminó el mensaje «Día futuro» de los gráficos. `npm test` aprobó 33 pruebas, incluida una regresión de hover después de un día válido, clic sin navegación, teclado y detalle por app. Sintaxis JavaScript aprobada.

Compilación Release e instalación local aprobadas; SHA-256 compilado e instalado coinciden. Se conservó el autoarranque y se solicitó apertura del dashboard actualizado.

## Etiquetas del eje en minutos enteros (2 de octubre de 2026)

### Rediseño definitivo por período

Este diseño reemplaza los dos ajustes compartidos descritos debajo. Día tiene un eje fijo de 0 a 60 minutos, con marcas `0 min`, `30 min` y `60 min`, incluso ante pequeñas fracciones superiores a 60. Semana tiene una lógica independiente: cada barra representa un día y el eje usa horas enteras, con un mínimo de 2 h y dos intervalos iguales. Para 905.0047654666667 minutos, las marcas son `0 h`, `8 h` y `16 h`. Al alternar modos se reemplazan límite, paso y formato, sin modificar las duraciones originales.

`npm test` aprobó 32 pruebas; se verificaron fracciones, semanas vacías, máximos de hasta 24 h, actualizaciones y regreso a Día. Sintaxis JavaScript y compilación Release aprobadas. Se revisaron ambos modos con Chart.js real y datos sintéticos en el navegador de Codex, dentro de un iframe de 360 × 480: Semana muestra `0 h`, `2 h`, `4 h` en el fixture y Día muestra `0 min`, `30 min`, `60 min` al alternar, sin desbordamiento horizontal.

Se actualizó la instalación local, se comprobó coincidencia de SHA-256 y se reabrió el dashboard. El autoarranque se conservó.

El ajuste posterior redondea también el límite de la escala, con dos intervalos iguales y límites suficientes para cubrir la barra apilada completa. Un máximo de 905.0047654666667 minutos produce `0`, `500` y `1000 min`. `npm test` aprobó 32 pruebas, incluida la regresión en ambos períodos, varios máximos y la escala vacía de 60 minutos; sintaxis JavaScript aprobada.

El eje de los gráficos de Día y Semana redondea únicamente el texto de sus etiquetas a minutos enteros. Los valores fraccionales de las barras y del historial conservan su precisión. `npm test` aprobó 31 pruebas, incluida una regresión con 60.00476546666667 minutos que comprueba las etiquetas `0 min`, `30 min` y `60 min` en ambos modos y la conservación del valor original. `node --check assets/dashboard.js` aprobó.

La compilación Release y `tools/install-user.ps1` aprobaron. El SHA-256 instalado coincide con el compilado, quedó una instancia activa y se conservó el inicio automático. Se solicitó apertura del dashboard con `--dashboard`; esta comprobación no incluye revisión visual interactiva de WebView2.

El registro local de producción `%LOCALAPPDATA%\HorariosTracker\lifecycle.log` debe mostrar un `start` después de instalar, `exit reason=tray` al elegir **Salir**, y `event_loop_ended` al terminar el loop. Un `panic` se registra antes de abortar cuando el hook de Rust puede ejecutarse. Este registro no contiene títulos de ventanas, nombres de procesos ni tiempos de uso; no equivale a un volcado de memoria ni garantiza explicar cierres externos.

Verificación local del 30 de septiembre de 2026: `cargo fmt --all -- --check` y `cargo test --locked --target x86_64-pc-windows-msvc` aprobaron (24 pruebas, 2 benchmarks ignorados). La compilación Release y la instalación de usuario finalizaron correctamente; el SHA-256 del ejecutable instalado coincidió con el compilado. `lifecycle.log` registró el arranque, una salida ordenada solicitada con `--quit` (`exit reason=external_request`, `event_loop_ended`) y el nuevo arranque. El tracker quedó ejecutándose.

Evidencia local del 28 de septiembre de 2026: 24 pruebas Rust aprobadas (2 benchmarks ignorados), 6 pruebas de interfaz, pruebas del instalador y desinstalador, compilación Release y verificación del ZIP con instalación simulada en una carpeta temporal. El ZIP 0.2.0 también se instaló previamente sobre la versión anterior: conservó el historial y el autoarranque, registró la desinstalación y superó el smoke test del dashboard. La orden `--quit` cerró el proceso guardando el segmento y permitió reiniciarlo con una sola instancia. Los cambios actuales de lógica de energía se comprobaron con eventos simulados; la validación manual de Windows limpio y energía continúa pendiente.

Tras el ajuste visual del dashboard del mismo día, `npm test` aprobó 6 pruebas de interfaz. La prueba añadida comprueba el formato sin segundos, el nombre Microsoft Edge, la configuración del gráfico y la apertura del detalle horario por fila. `node --check assets/dashboard.js` también aprobó. Estas comprobaciones no sustituyen la revisión visual en WebView2.

Después se compiló Release con `cargo build --release --locked --target x86_64-pc-windows-msvc` y se actualizó la instalación del usuario mediante `tools/install-user.ps1`. El instalador respaldó el historial, cerró ordenadamente la instancia anterior y arrancó la nueva. La suma SHA-256 del ejecutable instalado coincide con la del binario recién compilado; se volvió a abrir el dashboard.

Para la actualización de colores del 28 de septiembre, `npm test` aprobó 7 pruebas de interfaz, incluida una que simula un icono con fondo blanco y comprueba que su color dominante se aplique al gráfico, la leyenda y la barra de progreso. `node --check assets/dashboard.js` y la compilación Release aprobaron. Se instaló la nueva versión y se regeneró el paquete 0.2.0. La eliminación de ejecutables antiguos quedó pendiente porque la revisión automática rechazó `Remove-Item`, incluso para un único archivo antiguo concreto.

El ajuste posterior de hover suavizó el fondo de las filas y botones del dashboard, añadió una respuesta ligera al pulsar y respeta `prefers-reduced-motion`. `npm test` volvió a aprobar las 7 pruebas; se compiló Release y se actualizó la instalación del usuario.

El diseño vigente del 29 de septiembre usa una tarjeta flotante de ancho acotado, fuera del flujo de la página y con `pointer-events:none` incluso mientras está visible. No desplaza contenido ni intercepta las barras. Para listas largas, la rueda sobre el gráfico desplaza la lista del hover; solo consume el evento si existe desbordamiento vertical y respeta Ctrl+rueda. No tiene botones. `npm test` aprobó 9 pruebas con los estilos reales cargados: posición fija, transparencia al puntero, límites del viewport, cambio de hora, conservación del scroll y captura selectiva de la rueda. Son comprobaciones de DOM y eventos con jsdom; la verificación interactiva en WebView2 sigue pendiente. Browser Use bloqueó previamente el archivo local por su política de URL.

El ajuste de animación posterior define entrada de 180 ms con opacidad, desplazamiento de 4 px y escala de 0,98 a 1; salida de 120 ms tras una tolerancia de 50 ms. Las horas vacías también completan la salida y los eventos repetidos no la posponen. Volver a una barra cancela el ocultamiento pendiente. El cambio de posición entre barras tarda 120 ms y se respeta movimiento reducido. `npm test` aprobó 10 pruebas, incluida la secuencia de salida interrumpida con reloj controlado; `node --check assets/dashboard.js` aprobó. La percepción visual de estas transiciones en WebView2 queda pendiente de revisión interactiva.

La revisión de la zona superior mantiene los controles y el contrato de datos, y cambia la cabecera a título grande sobre fondo claro, selector de día agrupado, acceso a configuración circular y total en una pila con la etiqueta del día. `npm test` volvió a aprobar las 10 pruebas y `node --check assets/dashboard.js` aprobó. La apariencia real en WebView2 aún requiere revisión visual interactiva.

La corrección posterior a la captura elimina los anchos fijos del selector de fecha y de las barras de progreso, centra las flechas mediante SVG y fija el eje vertical del gráfico en 60 minutos por hora, salvo datos que superen ese valor para evitar recortes. `npm test` aprobó 10 pruebas, con comprobaciones del límite vertical normal y del caso superior a 60 minutos; `node --check assets/dashboard.js` aprobó. La revisión visual interactiva en WebView2 sigue pendiente.

El selector de fecha ahora abre un calendario integrado en el dashboard, con nombres en español y solo fechas del historial hasta hoy. Sustituye el control nativo de WebView2. El botón de configuración utiliza una silueta simétrica de engranaje. `npm test` aprobó 11 pruebas, incluida la selección de fecha, límites, Escape y clic exterior; `node --check assets/dashboard.js` aprobó. La revisión visual de WebView2 sigue pendiente.

Se retiró la flecha desplegable del botón de fecha para que la fila conserve únicamente las flechas de navegación entre días. La fecha sigue siendo un botón con foco visible y abre el calendario integrado.

El calendario ahora calcula su ancho y alto dentro del viewport al abrirse y redimensionarse; se despliega por encima de la fecha cuando allí hay más espacio y permite desplazamiento interno si el mes no entra completo. En ventanas bajas reduce relleno y celdas. `npm test` aprobó 12 pruebas, incluida una comprobación de límites y cambio de lado en 360 × 480; `node --check assets/dashboard.js` aprobó. La revisión visual directa en WebView2 continúa pendiente.

El fondo hover de las filas de aplicaciones ahora se extiende 8 px a cada lado del contenido, dejando espacio entre los bordes redondeados, el icono y la flecha sin mover su alineación. `npm test` aprobó 12 pruebas. `cargo build --release --locked --target x86_64-pc-windows-msvc` aprobó y `tools/install-user.ps1` actualizó la instalación del usuario, conservando el inicio automático. Los SHA-256 del ejecutable compilado y el instalado coinciden; el proceso nuevo quedó en ejecución. El cambio visual aún requiere comprobación en WebView2.

La revisión posterior eliminó el límite de 900 px y el fondo exterior que formaban una tarjeta alrededor del dashboard. El selector de fecha conserva un ancho máximo de 720 px. Las filas usan un hover plano y tenue, sin animación de escala ni tooltip nativo con el nombre del ejecutable. Se revisó un render local de escritorio en Edge con datos sintéticos. `npm test` aprobó 12 pruebas, `node --check assets/dashboard.js` y la compilación Release aprobaron; `tools/install-user.ps1` actualizó la instalación y el SHA-256 instalado coincide con el compilado. El proceso nuevo quedó en ejecución. La revisión directa en WebView2 sigue pendiente.

La primera fila de «Más usadas» ahora empieza 8 px debajo del encabezado, de modo que su resaltado no toca la banda del título. `npm test` aprobó 12 pruebas; se compiló Release, se actualizó la instalación del usuario y se reabrió el dashboard. Los SHA-256 del ejecutable compilado e instalado coinciden.

El hover de las filas ahora se extiende hasta ambos bordes de la ventana, igual que la banda de «Más usadas»; los iconos, tiempos y flechas conservan el margen interior. Se revisó un render local en Edge con datos sintéticos. `npm test` aprobó 12 pruebas; se compiló Release, se actualizó la instalación, se verificó la igualdad de SHA-256 y se reabrió el dashboard.

La revisión del extremo derecho eliminó la flecha aislada de cada fila y alineó la duración con el contador del encabezado. La fila sigue siendo un botón que abre y cierra el detalle por hora. Se comprobó el render en Edge con datos sintéticos, `npm test` aprobó 12 pruebas, `node --check assets/dashboard.js` y la compilación Release aprobaron. Se instaló el ejecutable, se comprobó que su SHA-256 coincide con el compilado y se reabrió el dashboard.

La revisión de separadores elimina los tramos vacíos a la izquierda: todas las líneas de la lista van de borde a borde, incluida la de encima de la primera aplicación. Se retiró el espacio extra entre «Más usadas» y esa línea. Se revisó el render local en Edge con datos sintéticos, `npm test` aprobó 12 pruebas, se compiló Release, se actualizó la instalación, se verificó la igualdad de SHA-256 y se reabrió el dashboard.

El selector de fecha se redujo a un control de hasta 340 px con «Hoy» a su lado, adecuado para el dashboard sin tarjeta. Se eliminó el estado «En uso ahora» del pie y los campos `active`, `paused` y `systemPaused` del JSON de la vista; la contabilización y las pausas internas se conservan. Se revisó el render de escritorio en Edge con datos sintéticos. `npm test` aprobó 12 pruebas, `cargo test --locked --target x86_64-pc-windows-msvc` aprobó 24 (2 benchmarks ignorados), `cargo fmt --all -- --check`, `node --check assets/dashboard.js` y la compilación Release aprobaron. Se actualizó la instalación, se comprobó la igualdad de SHA-256 y se reabrió el dashboard.

En ventanas maximizadas, el selector compacto dejaba demasiado espacio vacío. El control de fecha ahora ocupa el ancho disponible junto a «Hoy». Se verificó un render de 1350 px en Edge, `npm test` aprobó 12 pruebas, se compiló Release, se actualizó la instalación, se comprobó la igualdad de SHA-256 y se reabrió el dashboard.

El botón externo «Hoy» se sustituyó por «Ir a hoy» dentro del calendario, visible solo al consultar una fecha anterior; la barra superior conserva el ancho disponible como un único control. El título del mes capitaliza solo su primera letra. Se verificó el calendario abierto con datos sintéticos en Edge, `npm test` aprobó 12 pruebas, `node --check assets/dashboard.js` y la compilación Release aprobaron. Se actualizó la instalación, se comprobó la igualdad de SHA-256 y se reabrió el dashboard.

El calendario permite tocar el mes para escoger entre los doce meses y el año para escoger directamente entre los años con historial. Elegir un día actualiza el dashboard sin cerrar el calendario; permanece abierto hasta pulsar la fecha otra vez, hacer clic fuera o presionar Escape. Las vistas de día, mes y año se revisaron en Edge con datos sintéticos. `npm test` aprobó 13 pruebas, incluida la navegación año → mes → día y sus límites; `node --check assets/dashboard.js` y la compilación Release aprobaron. Se actualizó la instalación, se verificó la igualdad de SHA-256 y se reabrió el dashboard.

La corrección visual del calendario quitó las «V» junto al mes y el año. La vista mensual muestra semanas completas con días vecinos deshabilitados; la vista anual muestra una página de doce años, incluidos los que están fuera del historial en estado deshabilitado. Se revisaron ambas vistas en Edge con datos sintéticos, `npm test` aprobó 13 pruebas, `node --check assets/dashboard.js` y la compilación Release aprobaron. Se instaló la nueva versión, se verificó la igualdad de SHA-256 y se reabrió el dashboard.

## Comprobaciones automatizadas

Desde un prompt MSVC x64 en la raíz del proyecto:

```bat
rustup component add rustfmt
cargo fmt --all -- --check
cargo test --locked --target x86_64-pc-windows-msvc
npm ci --ignore-scripts
npm test
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests\install.test.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests\uninstall.test.ps1
cargo build --release --locked --target x86_64-pc-windows-msvc
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\package-release.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests\package.test.ps1
```

Para medir el índice con historiales sintéticos de uno, tres y cinco años, y un año con checkpoints cada minuto, en Release:

```bat
cargo test --release --locked --target x86_64-pc-windows-msvc benchmark_large_histories -- --ignored --nocapture
cargo test --release --locked --target x86_64-pc-windows-msvc benchmark_checkpoint_cadence -- --ignored --nocapture
```

Node 22 o posterior se usa solo para pruebas. No forma parte de la aplicación instalada. Las pruebas JavaScript usan jsdom y sustituyen el dibujo de Chart.js; no demuestran el renderizado real de WebView2.

El ensayo sintético de un año con una fila por minuto generó 525.600 filas y un CSV de 16,54 MB. La primera construcción del índice tardó 1,36 s; reabrirlo, 61,9 ms; anexar una fila, 37,3 ms en esta máquina. Se ejecutó `benchmark_checkpoint_cadence` en Release. Estos números miden ese historial sintético, no el tiempo de apertura de la interfaz ni la frecuencia real de cambios de aplicación.

`tools/verify-package-install.ps1` instala el ZIP sobre la instalación del usuario actual, por lo que debe ejecutarse solo en un perfil de prueba o después de respaldar el historial. `tests/smoke-installed.ps1` requiere el tracker instalado y el dashboard cerrado; abre el dashboard desde una segunda invocación y vuelve a cerrarlo. `tools/measure-idle.ps1 -Seconds 30` mide CPU y memoria de una sola instancia mientras permanece en bandeja.

Rust comprueba el intervalo de checkpoint de un minuto, el aviso de WebView2 separado, el guardado de un segmento de un minuto, fracciones de segundo, pausa por inactividad, pausa del sistema, segmentos vivos sin duplicación, separación de horas y medianoche, reconstrucción del índice, cambios en medio del CSV, huella incremental de escrituras propias, duración corrupta excesiva, filas incompletas y recuperación idempotente del journal para todos los prefijos parciales de una fila. Simula eventos WTS de bloqueo, desbloqueo, cierre de sesión, desconexión y reconexión local/remota; suspensión, reanudación automática o por usuario y cancelación del apagado. Otra prueba avanza el reloj de contabilización sin esperar: guarda cinco minutos anteriores a la suspensión, excluye cinco minutos suspendido y añade dos minutos tras reanudar, sin duplicar registros. Un proceso hijo termina sin desenrollar en cuatro fases de un checkpoint y el proceso padre verifica la recuperación sin duplicación. Los historiales son sintéticos y viven en directorios temporales exclusivos.

La interfaz se prueba para tema inicial, señal de carga completada, errores visibles, reintento, recuperación, fechas, duración, iconos tardíos, tooltip dentro del viewport y liberación del timer. El instalador se prueba en una carpeta temporal: presencia y ausencia simuladas de WebView2 Runtime, actualización, fallo deliberado después de copiar, restauración, paquete incompleto, retención de backups y limpieza selectiva de las instalaciones anteriores. La desinstalación se prueba con fallo de cierre inyectado, retirada selectiva de archivos/Registro y conservación del historial. `tests/package.test.ps1` comprueba SHA-256, archivos y ruta de `Instalar.cmd`, extrae el ZIP y ejecuta `Install-AppFiles` con el contenido real en una carpeta temporal. No inicia el ejecutable ni altera el perfil personal.

## Fallos de carga

Un error del historial muestra una pantalla con **Reintentar**. Configuración se carga sin consultar el CSV. Si la interfaz no responde, un temporizador Win32 independiente del checkpoint revela la ventana y avisa después de aproximadamente 12 segundos; no se realiza polling adicional.

Para reparar datos manualmente, primero elige **Salir** y copia `%LOCALAPPDATA%\HorariosTracker` a otra ubicación. `app_usage.csv` es la fuente de verdad. Puedes eliminar únicamente `app_usage.daily.json` para reconstruir el índice. Conserva el CSV y cualquier journal pendiente si hay un conflicto de recuperación.

El índice versión 2 verifica una huella del contenido completo procesado al cargar y al detectar crecimiento externo. Antes de anexar una fila, comprueba cambios externos; tras una escritura confirmada por el journal, calcula la huella únicamente de los bytes nuevos. Consulta metadatos cuando no hubo cambios. La huella detecta cambios accidentales; no es una protección contra manipulación deliberada. El índice en memoria se libera al cerrar el dashboard.

## Instalación y actualización

El ZIP incluye `Instalar.cmd`, ejecutable y scripts: se instala sin Rust ni Node. Antes de modificar la instalación, comprueba la versión `pv` de WebView2 Runtime en las claves por máquina y usuario [documentadas por Microsoft](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution). Si falta o la versión no es válida, se detiene con un enlace para instalarlo. Luego prepara archivos, cierra ordenadamente el tracker, copia los datos y aplica la actualización. Espera hasta diez segundos a que el nuevo proceso cree su ventana Win32 de sesión y comprueba que siga vivo; si falla el arranque o una operación de archivos o integración, restaura archivos anteriores, accesos directos, autoarranque y entrada de desinstalación. Ante un fallo de restauración conserva `.update-*` e informa su ruta. Tras un arranque correcto retira únicamente el ejecutable anterior y los accesos directos que apuntan a él. La entrada de **Aplicaciones instaladas** apunta a `uninstall-user.ps1`, que conserva `%LOCALAPPDATA%\HorariosTracker`.

Conserva las cinco copias de historial más recientes generadas por el instalador. No elimina carpetas con nombres o contenido ajenos a ese formato. De los antiguos ejecutables con fecha conserva dos; las actualizaciones nuevas no acumulan más ejecutables. Un corte eléctrico durante la instalación no equivale a una excepción capturable: usa el respaldo y vuelve a ejecutar el instalador si ocurre.

En la prueba local del ZIP, el ejecutable instalado coincidió por SHA-256 con el incluido en el paquete. La segunda invocación terminó con código 0, dejó un único daemon, mostró la ventana y permitió cerrarla; después no quedaron procesos `msedgewebview2` de esa sesión. La entrada de desinstalación indica versión 0.2.0 y el script instalado está presente. La desinstalación real sobre el perfil personal no se ejecutó; su flujo completo se ensayó en un directorio y una clave de Registro temporales.

## Medición local de recursos

Windows 10 Pro 19042, binario Release instalado, una instancia, dashboard cerrado. Cada muestra duró aproximadamente 30 segundos y la CPU es el promedio dividido por todos los núcleos lógicos del equipo.

| Estado | Memoria residente | Memoria privada | CPU promedio |
| --- | ---: | ---: | ---: |
| Inicio solo en bandeja | 1,27–1,98 MB | 2,05–2,18 MB | 0,051 % |
| Después de abrir y cerrar el dashboard | 5,43–6,49 MB | 6,64–6,77 MB | 0,025 % |
| Sesión en bandeja tras más tiempo de ejecución | 10,27–10,34 MB | 6,80–7,41 MB | 0,094 % |

La última muestra duró 45,6 segundos y se tomó sin procesos `msedgewebview2`. `EmptyWorkingSet` retira páginas residentes al entrar en modo bandeja, pero el conjunto residente puede volver a crecer con el uso normal: el objetivo de menos de 10 MB no se mantiene siempre, incluso en esta máquina. Estas cifras no representan memoria máxima durante uso activo ni garantizan el mismo resultado en otros Windows o equipos. La CPU no es literalmente 0 %; el redondeo de algunas herramientas puede mostrar 0,0 %.

## Rediseño del detalle horario (30 de septiembre de 2026)

Se retiraron la tabla fija de 24 horas y la grilla de 24 valores dentro de cada aplicación. El gráfico diario permite fijar una hora para ver su total y las aplicaciones ordenadas por duración; cada aplicación muestra un minigráfico de 24 barras con su hora de mayor uso. La exploración por teclado y la selección sobreviven a las actualizaciones automáticas sin perder el foco. Los usos positivos inferiores a un minuto se presentan como «Menos de 1 min» en los detalles.

`npm test` aprobó 15 pruebas de interfaz y `node --check assets/dashboard.js` aprobó. Se revisaron imágenes generadas en Edge con datos sintéticos a 780 px y dentro de un viewport de 380 px; el detalle contextual y el minigráfico mantuvieron la alineación. `cargo build --release --locked --target x86_64-pc-windows-msvc` aprobó. `tools/install-user.ps1` actualizó la instalación, conservó el autoarranque y dejó el proceso ejecutándose. El SHA-256 del ejecutable instalado coincide con el compilado. La revisión visual interactiva dentro de WebView2 queda pendiente.

El ajuste posterior movió «Actualizado» desde el pie al mismo renglón que «Hoy», alineado al extremo opuesto. La hora se muestra sin segundos y su nodo de texto solo cambia al avanzar el minuto; los datos siguen refrescándose cada segundo. `npm test` aprobó 16 pruebas de interfaz y `node --check assets/dashboard.js` aprobó. Se revisó con datos sintéticos en Edge a 780 px y en un viewport de 380 px, incluida una fecha anterior larga. La compilación Release y la actualización de la instalación aprobaron; se conservó el autoarranque, el ejecutable instalado tiene el mismo SHA-256 y quedó una sola instancia activa.

El texto del dashboard y Configuración ahora usa `user-select: none` con el prefijo de WebView2; los futuros controles editables conservan selección normal. `npm test` volvió a aprobar las 16 pruebas de interfaz y `node --check assets/dashboard.js` aprobó. La compilación Release y la actualización de la instalación aprobaron; el SHA-256 instalado coincide con el compilado y quedó una sola instancia activa.

## Vista semanal y promedio diario (30 de septiembre de 2026)

El dashboard ofrece Día y Semana. Semana agrupa lunes a domingo, presenta el promedio diario con el número de días usados como divisor, el total semanal, siete barras diarias y las aplicaciones ordenadas por tiempo semanal. Cada aplicación expande un minigráfico de siete días; una barra principal abre su día. El calendario y las flechas navegan por semanas en ese modo. En la semana actual, los días futuros no suman; si el historial empieza a mitad de una semana, el promedio comienza en el primer día registrado. El segmento activo se suma a la vista sin escribir ni duplicar datos. El CSV y el índice diario conservaron su formato.

`npm test` aprobó 17 pruebas de interfaz, `cargo test --locked --target x86_64-pc-windows-msvc` aprobó 26 pruebas (dos benchmarks ignorados), `node --check assets/dashboard.js` y `cargo fmt --check` aprobaron. Las pruebas nuevas cubren semana completa, inicio parcial, promedio de la semana actual, segmento activo y controles de la interfaz. Se revisaron capturas con datos sintéticos en Edge a 780 px y en un iframe de 380 px, además del detalle semanal abierto. `cargo build --release --locked --target x86_64-pc-windows-msvc` aprobó; `tools/install-user.ps1` actualizó la instalación, conservó el autoarranque, dejó una instancia activa y el SHA-256 instalado coincide con el compilado. La revisión visual interactiva dentro de WebView2 queda pendiente.

El ajuste siguiente mantuvo la altura del resumen al alternar Día y Semana: el total semanal se muestra en el encabezado de «Más usadas», junto al listado que suma, y ya no agrega una línea debajo del promedio. Con datos sintéticos idénticos, el inicio de «Más usadas» quedó a 516,70 px en ambos modos a 780 px de ancho y a 527,36 px en ambos modos dentro de un iframe de 380 px. Se revisaron capturas en ambos tamaños. `npm test` volvió a aprobar 17 pruebas y `node --check assets/dashboard.js` aprobó. La compilación Release y la actualización local aprobaron; el hash del ejecutable instalado coincide y quedó una sola instancia activa.

El dato de una barra en los minigráficos por app ahora ocupa el mismo renglón que «Mayor uso», en lugar de crear una fila al hacer hover o usar el teclado. En Edge, con datos sintéticos, la siguiente app conservó exactamente su posición antes y después del hover en Día y Semana: 826,42 px a 780 px de ancho y 837,08 px dentro de un iframe de 380 px. Se revisó una captura estrecha con la barra activa. `npm test` aprobó las 17 pruebas y `node --check assets/dashboard.js` aprobó. La compilación Release y la actualización local aprobaron; el hash del ejecutable instalado coincide con el compilado y quedó una sola instancia activa.

El tooltip del gráfico principal reserva cuatro filas de 28 px y mantiene fijo el encabezado; las aplicaciones restantes se recorren con la rueda sobre el gráfico. Los nombres largos se truncan en una sola línea para evitar cambios de altura. Con datos sintéticos en Edge, el tooltip midió 167 px con una, cuatro y ocho aplicaciones tanto a 780 px como en un iframe de 380 px; con ocho aplicaciones la lista tenía 112 px visibles, 224 px de contenido y avanzó 56 px con la rueda. Se revisó una captura estrecha después de desplazar la lista. `npm test` aprobó 18 pruebas y `node --check assets/dashboard.js` aprobó. La compilación Release y la actualización local aprobaron; el hash instalado coincide con el compilado y quedó una sola instancia activa.

## Días sin medición (30 de septiembre de 2026)

`app_usage.coverage.json` registra una fecha cuando el tracker funciona en una sesión activa; no registra actividad de aplicaciones ni escribe de nuevo la misma fecha. Los días con filas CSV anteriores a esta versión también cuentan como medidos. Un día marcado sin filas muestra `0min` y «Sin uso registrado»; uno sin marca ni filas muestra «—» y «Sin medición». Semana marca los días faltantes con `—`. Un archivo de cobertura ilegible queda intacto y no impide registrar uso en CSV.

La prueba Rust sintética comprueba persistencia de un día sin uso, un día sin medición entre dos días medidos y el fallback de un archivo corrupto. La prueba de interfaz comprueba ambos estados, el promedio semanal, el eje del gráfico y el detalle de una app. En la primera entrega, `cargo test --locked --target x86_64-pc-windows-msvc` aprobó 28 pruebas (dos benchmarks ignorados); `npm test` aprobó 19 pruebas; `node --check assets/dashboard.js` y `cargo fmt --check` aprobaron.

El ajuste posterior usa todos los días del calendario desde la instalación como divisor del promedio, incluso los que no tienen registros: un historial de 60 minutos con inicio en lunes da 60/7 minutos en una semana completa aunque el tracker solo haya medido miércoles y viernes. La fecha de inicio se conserva en `app_usage.coverage.json`; en instalaciones previas se infiere de la creación del CSV o de un registro histórico anterior. Los días sin medición siguen identificados por separado en la vista diaria y el gráfico semanal.

`cargo test --locked --target x86_64-pc-windows-msvc` aprobó 29 pruebas (dos benchmarks ignorados), incluida la migración de un archivo de cobertura anterior sin fecha de inicio; `npm test` aprobó 19 pruebas. `cargo fmt --check`, `node --check assets/dashboard.js` y la compilación Release aprobaron. `tools/install-user.ps1` actualizó la instalación local, conservó el autoarranque y dejó una sola instancia activa. El SHA-256 instalado coincide con el compilado; el archivo de cobertura migrado ya contiene `startDate`.

La compilación Release aprobó y `tools/install-user.ps1` actualizó la instalación local. El SHA-256 instalado coincide con el compilado, quedó una instancia activa, se conservó el inicio automático y apareció `app_usage.coverage.json` en el directorio de datos existente. La verificación manual de bloqueo, suspensión y días completos todavía requiere observar esos eventos reales.

## Comprobación manual pendiente

En un perfil de prueba, instala el ZIP, abre desde acceso directo y bandeja, activa inicio automático, reinicia y desinstala tras desactivar el inicio. Repite una actualización con historial existente.

Con datos sintéticos, usa una app dos minutos, bloquea un minuto y comprueba que no sumó el bloqueo. Repite suspensión, hibernación y reanudación con pausa por inactividad activada y desactivada. Revisa ambos temas, ventana pequeña, minimización, varias escalas DPI y monitores.

Esto requiere Windows 10/11 real o una VM apropiada. Las pruebas unitarias no certifican mensajes de energía reales, RDP, consumo en todos los equipos ni recuperación ante fallos físicos. No alteres el historial personal para probarlo.


## Correcciones de auditoría — 30 de septiembre de 2026

- `npm test`: 26 pruebas aprobadas, con foco de ranking cerrado y expandido, calendario con flechas/cambio de nivel, selección accesible estable en ambos períodos, duraciones menores de un minuto, leyenda completa, contraste, privacidad y errores.
- `cargo test --locked --target x86_64-pc-windows-msvc`: 35 pruebas aprobadas, dos benchmarks ignorados. Nuevas verificaciones: pausa manual persistente, no reanudación por eventos del sistema, exportación fraccional, borrado con índice ya cargado y recuperación de reset interrumpido/fallido. Se conservan las pruebas de fracciones, medianoche y journal en proceso separado.
- `node --check assets/dashboard.js`, `node --check assets/view-state.js` y `cargo fmt --all -- --check`: aprobados.
- `cargo build --release --locked --target x86_64-pc-windows-msvc`: aprobado. `tools/install-user.ps1` actualizó la instalación, conservó autoarranque y dejó una sola instancia. El SHA-256 instalado coincide con el compilado; se solicitó apertura del dashboard con `--dashboard`.
- Renders sintéticos revisados en Edge: Día oscuro con calendario, Semana clara con cobertura parcial, Configuración y dashboard en iframe de 360 × 480. Capturas en `docs/auditoria-2026-09-30`. El fixture se genera con `node tools/render-audit.cjs <carpeta-temporal>`; no usa historial personal.

La pausa manual se conserva al reiniciar. El borrado exige confirmación nativa, conserva preferencias/exportaciones/backups y queda pausado; no se ejecutó sobre el historial real del usuario. La marca `app_usage.reset.json` se procesa antes del journal para impedir que una recuperación restaure datos borrados. Cargas antiguas de iconos se descartan por generación después del borrado.

La implementación de los once puntos está detallada al final de `AUDITORIA-DISENO-2026-09-30.md`. Pendientes de validación manual: Narrador, DPI, alto contraste, varios monitores, energía real y Windows limpio. Esta evidencia no aprueba por sí sola distribución general.
`tests/package.test.ps1` también aprobó: checksum SHA-256, contenido del ZIP, ruta del instalador y copia del payload en una instalación temporal. Se regeneró el paquete con esta evidencia.

## Segunda pasada de auditoría — 30 de septiembre de 2026

`npm test` aprobó 30 pruebas. Las cuatro nuevas cubren foco de error (entrada, repetición, recuperación, origen desaparecido y navegación independiente en Configuración), colores exclusivos de categorías simultáneas tras ranking/fecha/modo, ayuda accesible con estado y foco preservados, y vocabulario/agrupación de privacidad. Sintaxis JavaScript y formato Rust aprobados. `cargo test --locked --target x86_64-pc-windows-msvc` aprobó 35 pruebas; dos benchmarks permanecen ignorados.

En Edge headless se verificó `load-error-title → retry-diagnostic → chart` en entrada, repetición y recuperación. Renders sintéticos: Día oscuro con calendario, Semana clara a 780 × 820, Configuración, Día y Semana en iframe de 360 × 480. Con seis categorías y cobertura parcial, la primera fila pasó de 610/664 px a 413 px respectivamente; ahora se ve una fila completa en ambos modos y no hay desbordamiento horizontal. Capturas y `diagnosticos.json` en `docs/auditoria-segunda-pasada-2026-09-30/`. Fixture reproducible: `node tools/render-audit.cjs <carpeta-temporal>`.

La estabilidad del color se limita al documento abierto y se conserva mientras no colisione con otra categoría visible; el agregado gris queda reservado. La ayuda completa está en un control `details/summary` nativo y no se reconstruye durante snapshots. La configuración usa tres grupos de privacidad, con retención de copias y exportaciones explícita.

Los renders y pruebas DOM no certifican Narrador, DPI, alto contraste, varios monitores ni eventos reales de energía. Los flujos nativos de gestión de datos siguen pendientes de recorrido manual; no se borró ni exportó el historial personal durante esta validación.

Compilación Release de esta segunda pasada aprobada. `tools/install-user.ps1` actualizó la instalación local, conservó el inicio automático y dejó una sola instancia activa. SHA-256 del compilado e instalado: `6750359BF4EE7AD29E595F840D2B4E5FDE67C78C1F2610B0081EA3BE3D6E5F22`. Se solicitó apertura del dashboard con `--dashboard`.
