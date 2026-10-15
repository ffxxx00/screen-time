# Screen Time

Aplicación nativa para Windows que registra cuánto tiempo permanece cada aplicación en primer plano. Guarda el historial localmente y muestra informes diarios con un gráfico por hora. No usa un servidor local ni envía telemetría.

> **Estado:** proyecto personal en desarrollo. Se compila e instala en Windows 10/11 x64; todavía no es una versión 1.0 validada exhaustivamente. Lee [Limitaciones](#limitaciones-conocidas) antes de confiar en el historial como registro exacto.

## Funciones

- Daemon de bandeja de sistema, sin ventana de consola.
- Seguimiento del cambio de ventana en primer plano mediante `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`.
- Registra el ejecutable, no el contenido de ventanas, teclas ni coordenadas del ratón.
- Informes locales por día, con gráfico apilado por hora, selección de fecha y lista de aplicaciones.
- Dashboard y configuración inicializados bajo demanda. WebView2 se destruye al cerrar el dashboard; al cerrar la última ventana se libera memoria residente del proceso para el modo bandeja.
- Tema nocturno, pausa opcional tras dos minutos sin entrada, y tamaño de ventana persistente.
- Pausa del conteo durante bloqueo, desconexión de sesión, suspensión y fin de sesión de Windows.
- Recuperación de escrituras interrumpidas mediante journal local y checkpoints periódicos.
- Iconos de aplicación obtenidos desde los ejecutables instalados en el equipo.
- Inicio automático opcional para el usuario actual, sin privilegios de administrador.

## Requisitos

Para **compilar** en Windows 10/11 de 64 bits:

- Rust estable con destino `x86_64-pc-windows-msvc`.
- Visual Studio 2022 Build Tools o Visual Studio, con **Desktop development with C++**.
- Windows 10/11 SDK, incluyendo `rc.exe` para compilar el icono y los metadatos del ejecutable.

Para **ejecutar**:

- Windows 10/11 x64.
- Microsoft Edge WebView2 Runtime Evergreen. La distribución habitual de WebView2 requiere que ese runtime esté disponible en el equipo.

El instalador comprueba WebView2 Runtime antes de cerrar o reemplazar una instalación existente. Si falta, muestra el enlace oficial de Microsoft para instalarlo y permite reintentar. El dashboard, Chart.js, iconos, CSV y preferencias funcionan sin conexión. El runtime WebView2 de Evergreen se instala y actualiza por separado.

## Compilar desde el código

Clona el repositorio y abre **x64 Native Tools Command Prompt for VS 2022**. Desde la raíz del proyecto:

```bat
cargo build --release --locked --target x86_64-pc-windows-msvc
```

Si `cargo` no está en `PATH`, usa:

```bat
"%USERPROFILE%\.cargo\bin\cargo.exe" build --release --locked --target x86_64-pc-windows-msvc
```

El binario resultante está en `target\x86_64-pc-windows-msvc\release\screen-time.exe`. Rust aplica el subsistema Windows y `build.rs` compila `assets/app.rc` con `rc.exe`; no aparece una consola al iniciar.

Para ejecutar las pruebas unitarias de Rust en Windows:

```bat
cargo test --locked --target x86_64-pc-windows-msvc
```

Las pruebas de interfaz requieren Node 22 o posterior: `npm ci --ignore-scripts` y `npm test`. La aplicación instalada no usa Node. Consulta [Validación y recuperación](docs/VALIDACION.md) para todas las comprobaciones, recuperación e instalación.

## Instalar para el usuario actual

Después de compilar, desde PowerShell en la raíz del repositorio:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\install-user.ps1
```

El script prepara los archivos, cierra limpiamente las instancias anteriores o actuales, respalda `app_usage.*` y actualiza `%LOCALAPPDATA%\Programs\Screen Time`. Crea accesos directos y conserva el inicio automático; comprueba que el nuevo proceso creó su ventana de sesión y revierte la actualización si el arranque falla. Tras un arranque correcto retira el ejecutable y los accesos directos que apuntan a la instalación anterior. Conserva cualquier otro archivo de esa carpeta. No necesita elevación. Conserva las cinco copias de datos más recientes del instalador.

Para distribuir una compilación, ejecuta `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\package-release.ps1` y luego `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\build-setup.ps1`. En `dist/` quedan el ZIP y **screen-time-0.3.0-setup-x64.exe**, ambos con SHA-256. El EXE ofrece un asistente gráfico por usuario, conserva historial y preferencias y no requiere permisos de administrador. Como alternativa, extrae el ZIP y ejecuta **Instalar.cmd**.

El acceso directo **Screen Time** abre el dashboard. En el icono de bandeja, el doble clic izquierdo abre el dashboard; el clic derecho muestra el menú con **Abrir**, **Pausar/Reanudar seguimiento** y **Salir**. La pausa manual también está en el dashboard y se conserva al reiniciar; cerrar la ventana mantiene el seguimiento.

Para activar el inicio automático, usa **Configuración → Inicio de Windows → Iniciar con Windows**. Para desinstalar, usa **Aplicaciones instaladas → Screen Time → Desinstalar** en Windows. La desinstalación cierra el tracker, retira el autoarranque, los accesos directos y los archivos de programa; conserva el historial de `%LOCALAPPDATA%\HorariosTracker`. Usa **Configuración → Datos y privacidad** para abrir la carpeta, exportar CSV o borrar el historial con confirmación. El borrado conserva preferencias, exportaciones y backups del instalador, y deja el seguimiento pausado.

## Datos locales y privacidad

Todo el historial pertenece al usuario de Windows y permanece en:

```text
%LOCALAPPDATA%\HorariosTracker\
```

- `app_usage.csv`: historial compatible con filas antiguas; los registros nuevos guardan UTC, ejecutable, duración, identidad por ruta, ruta y desfase horario.
- `app_usage.daily.json`: resumen diario regenerable para consultar el historial sin recorrer todo el CSV en cada actualización.
- `app_usage.coverage.json`: fechas en que el tracker estuvo activo, incluso si no registró uso. Permite distinguir «sin uso» de «sin medición»; en días anteriores a esta versión, un día sin filas CSV queda como «sin medición».

- `app_usage.settings.json`: tema, pausa por inactividad, pausa manual y tamaños de ventana.
- `app_usage.paths.json`: rutas locales de los ejecutables para presentar sus iconos.
- `app_usage.pending.json`: journal temporal para completar la última escritura si Windows interrumpe el proceso.
- `app_usage.reset.json`: intención durable de borrado; permite completarlo antes de recuperar el journal al reiniciar.
- `Exportaciones/`: copias CSV creadas desde Configuración.
- `backups/`: copias del historial realizadas por el instalador antes de una actualización.

El promedio semanal incluye todos los días transcurridos desde el inicio de la instalación, aunque un día no tenga datos. Esos días aportan cero minutos al cálculo y conservan la indicación «Sin medición» cuando el tracker no dejó constancia. No se cuentan días anteriores al inicio ni días futuros.

No se guardan títulos de ventanas, teclas, coordenadas, capturas, contenido del portapapeles ni datos de cuentas. El programa no tiene backend, conexión HTTP local, sincronización, analytics ni telemetría. Consulta el PID/ruta del proceso en foco para asociarlo al registro; después de mostrar el dashboard, consulta procesos activos para completar los iconos en segundo plano. `WH_KEYBOARD_LL` y `WH_MOUSE_LL` solo indican que hubo entrada y descartan el contenido del evento.

Para cambiar o borrar tus datos, cierra primero el tracker desde la bandeja. Luego respalda o elimina `%LOCALAPPDATA%\HorariosTracker` desde el Explorador de archivos.

## Formato del historial

El CSV no tiene encabezado. Desde 0.3.0, cada fila nueva contiene seis campos:

```text
inicio_utc_rfc3339,ejecutable,segundos,identidad,ruta,desfase_utc_segundos
2026-10-15T13:00:00.123456Z,app.exe,12.500000,path:c:\apps\app.exe,C:\Apps\app.exe,-10800
```

El instante se guarda en UTC con microsegundos; el desfase conserva la hora y el día locales en que se registró. Las duraciones provienen del reloj monotónico. La identidad usa la ruta normalizada, de modo que dos ejecutables con el mismo nombre en carpetas distintas tienen filas separadas. Si coinciden sus nombres, el dashboard muestra la carpeta para distinguirlos.

Las filas anteriores de tres campos (`fecha local,ejecutable,segundos`) siguen siendo válidas y no se reescriben. Su zona horaria y ruta no se pueden reconstruir con certeza. El dashboard las mantiene como historial anterior cuando coincide un nombre con registros nuevos. El índice se regenera automáticamente y CSV estándar permite rutas con comas o caracteres Unicode.

Para volver a una versión anterior a 0.3.0, restaurá primero el respaldo `pre-install-*` creado al actualizar, con el tracker cerrado. Las versiones anteriores no leen el formato nuevo. Una actualización fallida restaura los datos anteriores y conserva aparte las escrituras del intento fallido.

## Arquitectura

Resumen técnico, detalles del formato y límites de recuperación en [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md). Las dependencias directas y sus licencias están en [docs/DEPENDENCIAS.md](docs/DEPENDENCIAS.md). Para contribuir, consulta [CONTRIBUTING.md](CONTRIBUTING.md); para comunicar problemas de seguridad, [SECURITY.md](SECURITY.md).

## Limitaciones conocidas

- La pausa por inactividad es **opcional**. Si está desactivada, mirar una aplicación en primer plano cuenta como uso. Una sesión bloqueada o desconectada y la suspensión del sistema siempre pausan el conteo.
- Un checkpoint se intenta cada minuto. Los cambios de aplicación, bloqueo, suspensión, salida y la pausa por inactividad guardan el tramo en ese momento. Ante un cierre inesperado puede perderse hasta aproximadamente un minuto de uso de la aplicación en foco; errores continuos del disco pueden ampliar la pérdida.
- Aunque el journal permite recuperar filas completas o parciales, ninguna app de usuario puede asegurar escritura persistente ante cualquier fallo físico o de hardware.
- Si Windows no permite consultar el estado de sesión, el programa adopta la opción segura y pausa hasta que recibe una notificación de desbloqueo o reconexión.
- El icono de procesos que no estaban en ejecución al abrir el dashboard aparece con la inicial hasta que el tracker conoce su ruta.
- El informe refleja sesiones observadas desde que el tracker está instalado y ejecutándose. No reconstruye actividad anterior ni ventanas de seguimiento perdidas.
- El consumo de RAM menor a 10 MB y CPU de 0,0 % en reposo son objetivos, no garantías medidas para todos los equipos. WebView2 usa procesos de Windows cuando se abre el dashboard.
- La suspensión, reanudación y recuperación se implementaron, pero deben probarse en el hardware, las ediciones de Windows y las configuraciones de energía concretos antes de depender del resultado.

## Terceros y licencias

Screen Time se distribuye bajo MIT; mira [LICENSE](LICENSE). Chart.js está incluido como archivo local y conserva su licencia MIT en `assets/CHARTJS-LICENSE.txt`. Las otras dependencias Rust conservan las licencias de sus autores; la lista está en [docs/DEPENDENCIAS.md](docs/DEPENDENCIAS.md).

## Historial de cambios

Consulta [CHANGELOG.md](CHANGELOG.md).

La actualización desde **Tiempo en pantalla** o **Horarios Tracker** conserva el CSV, las preferencias y el inicio automático; al terminar, retira el ejecutable antiguo y los accesos directos que lo abren. `HorariosTracker` permanece como identificador interno de compatibilidad, no como nombre mostrado al usuario. El producto se llama **Screen Time** y el binario `screen-time.exe`.
