# Auditoría de diseño de Screen Time

Fecha: 30 de septiembre de 2026. Público: usuarios de Windows en sus PC personales.

## Dictamen

La aplicación tiene una base visual clara y contenida, pero todavía no aprobaría una revisión de producto para distribución general. Los problemas principales afectan la interpretación del historial, el control del seguimiento y la accesibilidad. Cambiar colores o redondeos no resolvería estas carencias.

No se identificó un bloqueo universal de la tarea principal. Sí hay defectos reproducibles de foco, descripción accesible y contraste que deben corregirse antes de presentar la experiencia como terminada.

## Alcance y evidencia

- Inspección de dashboard, configuración, estados de error, contrato de datos y ventanas nativas.
- Renders locales en Edge de Día, Semana, detalle por aplicación y calendario oscuro; configuración y un iframe de 360 × 480 px. Datos exclusivamente sintéticos.
- Diagnósticos de DOM para refresco, reordenamiento, calendario y duraciones inferiores a un minuto.
- Las 19 pruebas existentes de interfaz y la comprobación de sintaxis aprobaron. Las pruebas existentes no cubren todos los defectos encontrados.
- No se inspeccionó interactivamente la ventana instalada de WebView2. Narrador, escalas DPI, alto contraste, varios monitores y los eventos reales de Windows siguen sin validación en esta auditoría.
- El fixture visual incluye aplicaciones sin iconos y no reproduce el orden descendente del backend. La longitud de sus barras y ese orden no se consideran defectos del producto.

P1: corregir antes de distribución general. P2: corregir en la siguiente iteración. Se distinguen defectos comprobados y recomendaciones de producto.

## Hallazgos

### 1. P1 — El usuario no puede conocer ni controlar con claridad el seguimiento

**Tipo:** carencia de producto comprobada en la interfaz y el menú.

El dashboard informa cuándo se actualizó, pero no si el conteo está activo o pausado por inactividad. La bandeja solo ofrece Abrir y Salir. Cerrar la ventana mantiene el seguimiento y no existe pausa manual. Para una aplicación que observa el uso personal, esto impide responder con seguridad «¿me está registrando ahora?» y «¿cómo lo detengo temporalmente?».

**Evidencia:** `assets/dashboard.html`, snapshot de `src/main.rs:679` y menú de `src/main.rs:1344`.

**Corrección:** estado discreto y explícito, acción Pausar/Reanudar y una explicación breve de que cerrar la ventana conserva el seguimiento. Distinguir pausa manual de pausa automática.

**Aceptación:** el usuario puede identificar el estado y pausar desde dashboard y bandeja, sin terminar el proceso ni perder su contexto.

### 2. P1 — El promedio semanal puede interpretarse como uso medido

**Tipo:** problema de comunicación de una decisión funcional intencional.

El promedio divide por días de calendario transcurridos desde la instalación; los días sin medición aportan cero. En semanas con datos parciales, «Promedio · 3 días» puede sugerir tres días observados. La explicación completa está en README. El dashboard solo explica el cero imputado cuando toda la semana carece de medición y no hay aplicaciones.

Ejemplo: 120 minutos registrados y tres días transcurridos producen 40 minutos de promedio aunque solo un día tenga medición. El cálculo implementado coincide con la documentación; el problema es la certeza que comunica.

**Evidencia:** `src/main.rs:655`, `assets/dashboard.js:418` y `assets/dashboard.js:422`.

**Corrección:** rotular «Promedio diario registrado», explicar el divisor y señalar la cobertura incompleta también cuando hay actividad. No modificar la fórmula sin una decisión explícita de producto.

**Aceptación:** con días faltantes, se entiende qué se promedia y que un día sin medición no certifica cero uso. La marca diaria tampoco debe presentarse como cobertura continua.

### 3. P1 — El refresco pierde el foco de una fila cerrada

**Tipo:** defecto reproducido.

Cuando cambia el orden de aplicaciones, se reconstruyen las filas. La restauración de foco depende de `openAppName`; una fila enfocada que no esté expandida queda fuera de esa recuperación. El diagnóstico enfoca una fila cerrada, cambia el orden y obtiene `BODY` como elemento activo. Las actualizaciones ocurren cada segundo.

**Evidencia:** `assets/dashboard.js:431` y `assets/dashboard.js:540`.

**Corrección:** preservar foco por identidad de aplicación y control, independientemente de su expansión. Evitar reconstrucciones cuando pueda reutilizarse el nodo.

**Aceptación:** una fila enfocada conserva foco después de subir o bajar en el ranking; si desaparece, el foco se traslada a un destino lógico.

### 4. P1 — La selección del gráfico pierde su descripción accesible

**Tipo:** defecto reproducido; el impacto con Narrador requiere prueba real.

Las flechas cambian la descripción a «10:00–11:00: 2min». El siguiente snapshot la reemplaza por la instrucción genérica de explorar horas. Si los valores no cambiaron, el inspector omite su actualización por caché y no restaura esa descripción. La selección visual y la descripción accesible quedan desalineadas. Las filas también usan un `aria-label` que omite la duración visible.

**Evidencia:** `assets/dashboard.js:166`, `assets/dashboard.js:448` y `assets/dashboard.js:571`.

**Corrección:** mantener la descripción de la selección mientras exista y exponer nombre, duración y estado expandido de cada fila. Evitar anuncios constantes en cada segundo.

**Aceptación:** la información accesible permanece estable después de varios snapshots y la duración de cada app puede consultarse sin depender del canvas.

### 5. P1 — El calendario pierde el foco al elegir mes o año

**Tipo:** defecto reproducido y comportamiento de teclado incompleto.

Al abrir el calendario, el foco permanece en el disparador. Las fechas son botones tabulables individuales; no existe navegación de calendario con flechas. Elegir un año o mes reemplaza el control enfocado y deja el foco en `BODY`. Escape sí cierra y devuelve el foco correctamente.

**Evidencia:** `assets/dashboard.js:327`, `assets/dashboard.js:336`, `assets/dashboard.js:340` y `assets/dashboard.js:349`.

**Corrección:** definir un patrón consistente de calendario, entrar a la fecha seleccionada, usar un único punto de tabulación en la grilla y restaurar foco al cambiar de nivel. Elegir explícitamente un popover no modal o un diálogo modal y aplicar su gestión de foco.

**Aceptación:** abrir, cambiar mes/año, seleccionar y cerrar puede hacerse con teclado, sin pérdida de foco ni decenas de tabulaciones. Referencia: [patrón de selector de fecha de WAI-ARIA](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/examples/datepicker-dialog/).

### 6. P1 — El día seleccionado en tema oscuro tiene contraste insuficiente

**Tipo:** defecto medido y observado en render.

El calendario usa texto blanco sobre `#65b1ff`: contraste calculado de 2,26:1. El número de fecha de 13 px requiere al menos 4,5:1 para el nivel AA. También afecta las selecciones de mes/año que reutilizan esos colores.

**Evidencia:** tokens de `assets/dashboard.css:2` y reglas de selección del calendario en ese archivo.

**Corrección:** utilizar texto oscuro sobre el acento claro o un fondo de selección más oscuro con texto blanco. Verificar ambos temas y estados seleccionados.

**Aceptación:** todas las selecciones activas cumplen 4,5:1. Referencia: [WCAG 2.2, contraste mínimo](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html).

### 7. P2 — El gráfico exige demasiada exploración para entenderlo

**Tipo:** problema de diseño observado y confirmado en código.

El eje vertical está oculto. Hay hasta cinco aplicaciones más «Otras», pero la leyenda identifica solo tres; en ventanas de menos de 740 px de alto desaparece. No puede leerse la magnitud de una barra ni identificar todas sus categorías a simple vista. Los colores derivados de iconos pueden ser semejantes y cambian con su llegada; no se garantiza discriminación suficiente.

**Evidencia:** `assets/dashboard.js:548`, `assets/dashboard.js:556`, `assets/dashboard.js:563` y la regla de altura de `.legend`.

**Corrección:** una escala mínima con unidad; leyenda compacta de todas las categorías o un diseño menos segmentado. Mantener acceso textual al detalle y no depender exclusivamente del color.

**Aceptación:** se pueden interpretar magnitud, unidad y categorías sin recorrer cada barra con el puntero.

### 8. P2 — La privacidad y la gestión de datos están fuera de la aplicación

**Tipo:** carencia de producto comprobada.

La implementación es local y el README explica qué se registra. Configuración no ofrece esa información ni permite abrir la carpeta, exportar desde un flujo comprensible o borrar el historial de forma guiada. Se exige localizar una carpeta interna y manipular archivos. La desinstalación conserva los datos, comportamiento que debe ser descubrible.

**Evidencia:** `assets/settings.html` y sección de datos y privacidad de `README.md`.

**Corrección:** sección «Datos y privacidad» con qué se registra, ubicación, retención al desinstalar y acciones proporcionadas. El borrado debe ser explícito, con alcance y confirmación claros.

**Aceptación:** un usuario sin conocimientos del proyecto puede entender y gestionar su historial desde la app.

### 9. P2 — La lista y los tooltips muestran cero para actividad positiva

**Tipo:** defecto de presentación reproducido.

Una aplicación con 0,5 minutos aparece como «0min» en la lista; los detalles sí dicen «Menos de 1 min». Esta incoherencia hace parecer que una app registrada no tuvo uso. Además, el texto compacto «7h 53min» perjudica la lectura frente a «7 h 53 min».

**Evidencia:** `assets/dashboard.js:67`, `assets/dashboard.js:134` y `assets/dashboard.js:535`.

**Corrección:** formato coherente para duraciones positivas inferiores a un minuto, con espacios entre número y unidad. Conservar la precisión real almacenada.

**Aceptación:** 30 segundos nunca se presentan como cero en las superficies de consulta.

### 10. P2 — Hay acciones cuya disponibilidad resulta poco evidente

**Tipo:** recomendación de usabilidad.

Las filas son botones expandibles pero no tienen indicador persistente de expansión. El calendario sigue abierto después de seleccionar una fecha y oculta parte del informe; «Ir a hoy» está dentro del calendario. El tooltip largo se desplaza con la rueda sobre el gráfico, un gesto poco habitual y que compite con desplazar la página.

**Evidencia:** `assets/dashboard.html:3`, `assets/dashboard.js:316`, `assets/dashboard.js:224` y render del detalle.

**Corrección:** indicador discreto de expandir/contraer; retorno visible al período actual; cerrar el calendario tras confirmar cuando esa sea la expectativa del flujo. Usar el inspector fijado para listas extensas y validar el gesto de rueda con usuarios.

**Aceptación:** las acciones se descubren sin tantear filas ni conocer instrucciones ocultas.

### 11. P2 — Los estados de error dejan pocas salidas útiles

**Tipo:** carencia comprobada del flujo.

El error oculta todo el contenido principal, incluido el acceso a Configuración, y ofrece únicamente Reintentar. Puede mostrar errores internos sin traducirlos a una acción comprensible. El texto tiene selección desactivada globalmente, lo que dificulta copiar el mensaje para pedir ayuda.

**Evidencia:** `assets/view-state.js:12` y `assets/dashboard.css:3`.

**Corrección:** conservar acceso a configuración/ayuda, explicar el problema en lenguaje común y ofrecer detalles copiables. Separar error de carga de historial y error de guardado.

**Aceptación:** ante un error persistente, el usuario entiende el estado y puede obtener información útil sin editar archivos a ciegas.

## Qué conservar

El total dominante, la navegación por fecha, las preferencias breves y la baja cantidad de acciones ayudan a la comprensión. Hay controles semánticos, foco visible, Escape, soporte de movimiento reducido y separación entre cero uso y ausencia de medición. Los tamaños pequeños mantienen los controles principales y el scroll vertical permite acceder al detalle. La arquitectura local y la destrucción del WebView al cerrar son buenas bases para una app personal.

## Orden recomendado

1. Corregir contraste, pérdida de foco, descripción accesible y formato de duraciones.
2. Resolver la comunicación del promedio y del estado de seguimiento.
3. Añadir privacidad y gestión de datos; mejorar error y descubrimiento de acciones.
4. Simplificar la lectura del gráfico y validar con usuarios sin conocimientos técnicos.

Antes de aprobar distribución general, verificar la aplicación instalada con teclado y Narrador, escalas 100/125/150/200 %, tema claro/oscuro, alto contraste, ventanas pequeñas y maximizadas. La matriz debe incluir primer uso, historial parcial, error persistente, cambio de ranking en vivo y cierre/reapertura desde bandeja. Los renders de Edge y las pruebas de DOM no certifican esos escenarios nativos.


## Correcciones aplicadas — 30/09/2026

La auditoría anterior describe el estado previo a esta intervención y se conserva como evidencia. Los once hallazgos tienen cambios implementados; esto no equivale a aprobar distribución general sin la matriz nativa pendiente.

Backup anterior: `<respaldo-local>/screen-time-backup-20260930-133959`. Incluye código, documentación, paquetes previos y los ejecutables compilado e instalado en `binarios/`; excluye dependencias regenerables y cachés de compilación.

| Punto | Cambio implementado | Evidencia |
|---|---|---|
| 1 | Estado explícito, pausa/reanudación desde dashboard y bandeja, persistencia de pausa manual y explicación del cierre | Pruebas de interfaz y Rust: guardado, reinicio, inactividad y reanudación del sistema |
| 2 | «Promedio diario registrado», divisor de calendario y cobertura incompleta aun con actividad; límites de la marca diaria | Prueba semanal; fórmula conservada |
| 3 | Reutilización de filas al reordenar; foco por identidad y control; fallback contiguo o fecha si desaparece | Pruebas de ranking cerrado, detalle abierto y eliminación |
| 4 | Selección accesible estable en Día/Semana; duración y expansión en filas; inspector sin anuncios automáticos cada segundo | Snapshots repetidos; Narrador pendiente |
| 5 | Popover no modal, foco inicial, un punto tabulable, flechas incluso entre meses, Home/End y recuperación entre niveles | Pruebas de teclado, Escape y selección |
| 6 | Texto oscuro sobre acento claro en tema oscuro; texto blanco sobre acento oscuro en claro | Comprobación de contraste de ambas parejas ≥ 4,5:1; render oscuro |
| 7 | Escala visible en minutos, todas las categorías en leyenda permanente, paleta estable e inspector textual | Prueba de seis categorías y siete apps en inspector; renders |
| 8 | Datos y privacidad, ubicación, retención, abrir carpeta, exportar CSV y borrado confirmado | CSV fraccional intacto; reset interrumpido y fallido; preferencias y copias conservadas |
| 9 | «Menos de 1 min» en todas las superficies y espacios entre número y unidad | Prueba con 30 segundos en lista, nombre accesible y tooltip |
| 10 | Indicador de expansión, retorno al período actual, cierre del calendario tras seleccionar y listas extensas en inspector | Pruebas de interacción; la rueda vuelve a desplazar la página |
| 11 | Error comprensible, separación de carga/guardado, Configuración accesible y detalles seleccionables | Pruebas de error, copia y comandos; configuración independiente del CSV |

Validación: 26 pruebas de interfaz aprobadas; 35 pruebas Rust aprobadas y 2 benchmarks ignorados; sintaxis JavaScript, formato Rust y compilación Release aprobados. Instalación local actualizada, con una instancia activa y coincidencia SHA-256 entre compilado e instalado. Capturas con datos sintéticos en [auditoria-2026-09-30](auditoria-2026-09-30/); fixture reproducible en `tools/render-audit.cjs`.

Referencias consultadas: [selector de fecha WAI-ARIA](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/examples/datepicker-dialog/) y [contraste mínimo WCAG](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). Se eligió un popover no modal con botones nativos; no se afirma conformidad completa con el ejemplo modal.

Pendiente para aprobar distribución general: recorrido real con Narrador, DPI 100/125/150/200 %, alto contraste, varios monitores, Windows limpio y pruebas de energía reales. Las pruebas de DOM, eventos simulados y renders de Edge no sustituyen esos ensayos ni la validación de comprensión con usuarios.
