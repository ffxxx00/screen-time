# Auditoría de diseño — segunda pasada

Fecha: 30 de septiembre de 2026. Revisión del código actual contra los once hallazgos de `AUDITORIA-DISENO-2026-09-30.md`.

## Dictamen

La versión actual mejora de forma sustancial el control del seguimiento, la interpretación del promedio y la navegación por teclado. Nueve puntos anteriores tienen su corrección comprobada en el alcance de esta revisión. Dos quedan parcialmente resueltos: lectura del gráfico y flujo de error.

Todavía no daría aprobación final para distribución general. Queda un defecto de foco en errores, una colisión de colores reproducible y una regresión de densidad que perjudica la consulta rápida en ventanas pequeñas. La siguiente iteración debería consolidar y simplificar lo añadido.

## Evidencia y límites

- `npm test`: 26 pruebas aprobadas.
- `node --check assets/dashboard.js`: aprobado.
- `cargo test --locked --target x86_64-pc-windows-msvc`: 35 pruebas aprobadas, dos benchmarks ignorados.
- Renders nuevos de Día oscuro con calendario, Semana clara, Configuración y Día/Semana en iframe de 360 × 480. Se utilizó el fixture sintético de `tools/render-audit.cjs`.
- Diagnóstico adicional de categorías tras reordenamiento y foco al mostrar errores. El foco en un canvas oculto también se verificó en Edge headless.
- La comprobación adicional del hover semanal conservó el texto y fondo correctos; no se encontró una regresión de contraste en ese estado.
- No se abrió ni se modificó el historial personal. No se instalaron cambios. Esta revisión no ejecutó un recorrido interactivo en WebView2, Narrador, distintas escalas DPI o alto contraste.
- Los ensayos de energía y los diálogos nativos de exportación/borrado requieren validación real; las pruebas unitarias no sustituyen esa evidencia.

## Estado de los puntos anteriores

| Punto | Estado en esta pasada | Evidencia |
|---|---|---|
| 1. Estado y control del seguimiento | Corregido en implementación | Estado explícito, Pausar/Reanudar, bandeja y persistencia de pausa |
| 2. Comunicación del promedio | Corregido; ajustar densidad | Divisor explicado y aviso de cobertura parcial visible |
| 3. Foco al cambiar el ranking | Corregido | Reutilización de filas, recuperación por identidad y pruebas de desaparición |
| 4. Descripción accesible del gráfico | Corregido en DOM | Selección estable en snapshots y duración en nombres de filas |
| 5. Teclado del calendario | Corregido en casos revisados | Foco al abrir/cambiar nivel, flechas, cierre y retorno al disparador |
| 6. Contraste de selección | Corregido | Token de texto sobre acento y pruebas de ambas parejas; hover semanal adicional correcto |
| 7. Lectura del gráfico | Parcial | Escala y leyenda completas; persiste colisión de colores |
| 8. Datos y privacidad | Corregido en implementación | Información y acciones disponibles; exportación/reset cubiertos por pruebas Rust |
| 9. Duraciones positivas menores de un minuto | Corregido | «Menos de 1 min» y espacios entre número y unidad |
| 10. Descubrimiento de acciones | Corregido | Indicador de expansión, retorno al período actual y rueda de página libre |
| 11. Errores | Parcial | Configuración y detalles copiables; falta gestión del foco |

«Corregido» describe los casos comprobados en código, pruebas y renders; no certifica accesibilidad completa ni funcionamiento en todos los equipos Windows.

## Hallazgos actuales

### A. P1 — El error deja el foco dentro del contenido que oculta

**Defecto reproducido.** Con el gráfico enfocado, `showUsageError()` oculta `main` y mantiene `document.activeElement` en `CANVAS`. Con el selector de fecha enfocado, el diagnóstico DOM conserva `date-trigger` dentro de ese mismo contenido oculto. El panel de error no recibe foco. `clearUsageError()` elimina el panel sin recuperar el foco si el usuario estaba en Reintentar.

La pantalla ofrece mejores salidas, pero la transición no mantiene un punto visible de navegación. Un usuario de teclado debe volver a orientarse y un lector de pantalla necesita un destino coherente.

**Ubicación:** `assets/view-state.js:8` y `assets/view-state.js:13`.

**Corrección:** guardar el origen del foco y llevarlo al encabezado o una acción del error solamente al entrar en ese estado. Al recuperar, restaurarlo a un control válido. Los reintentos automáticos no deben volver a moverlo.

**Aceptación:** entrada, repetición del error y recuperación conservan un foco visible y lógico. Agregar una prueba de transición; las pruebas actuales comprueban presencia, comandos y copia, pero no el foco.

### B. P2 — Una aplicación puede tener exactamente el color de «Otras aplicaciones»

**Defecto reproducido.** `stableColors` asigna seis colores por orden de aparición. Su sexto color es `#aab7c5`, el mismo reservado al agregado «Otras aplicaciones». Tras consultar seis apps y llevar la sexta al primer puesto, el gráfico muestra esa app y el agregado con idéntico color.

Resultado del diagnóstico: `F → #aab7c5` y `Otras aplicaciones → #aab7c5`. Las categorías adyacentes no pueden distinguirse por su codificación visual. Las etiquetas textuales siguen disponibles, pero se reduce la utilidad del apilado.

Además, la estabilidad se limita a la vida del documento: al cerrar y reabrir se crea otro mapa, por lo que la asignación puede cambiar con el orden inicial. No debe describirse como estabilidad persistente entre sesiones.

**Ubicación:** `assets/dashboard.js:3`, `assets/dashboard.js:6`, `assets/dashboard.js:556` y `assets/dashboard.js:561`.

**Corrección:** reservar un estilo exclusivo para el agregado y evitar colisiones entre las categorías simultáneamente visibles. Definir expresamente si se quiere identidad estable solo durante la consulta o entre sesiones.

**Aceptación:** después de cambiar ranking, fechas y modos, ninguna app visible comparte el estilo del agregado. La prueba de llegada tardía de iconos no cubre este caso.

### C. P2 — Las explicaciones permanentes desplazan la tarea principal

**Regresión de diseño observada.** Se añadieron a la vista principal la explicación del cierre, el estado del seguimiento, el límite de la marca diaria y un párrafo extenso sobre el promedio. La leyenda completa repite nombres y duraciones que aparecen en la lista.

En el render de 360 × 480, Día muestra el inicio de la leyenda y ninguna fila de «Más usadas». Semana ocupa seis líneas de explicación antes del gráfico y la lista queda fuera del primer pantallazo. A 780 × 820, la vista semanal solo deja ver una fila de apps. Los controles caben y no se observó desbordamiento horizontal en los dos iframes: el problema es la distribución vertical y el esfuerzo de lectura.

**Ubicación:** `assets/dashboard.html:3`, `assets/dashboard.html:4`, `assets/dashboard.js:409` y estilos de `.legend`.

**Corrección:** conservar estado y pausa visibles; resumir cobertura a una línea contextual y dejar el método completo en un control de ayuda accesible. Mostrar la explicación del cierre al primer uso o en ayuda. Usar una leyenda que identifique categorías sin repetir seis duraciones si la lista ya cumple esa función.

**Aceptación:** probar una ventana de 360 × 480 con seis categorías y cobertura parcial. El usuario debe llegar al resumen y a las aplicaciones prioritarias con menos desplazamiento, manteniendo una explicación accesible de la metodología.

### D. P2 — La nueva ayuda usa vocabulario interno y concentra demasiada información

**Recomendación de producto.** La vista diaria usa «tracker»; Configuración habla de «hooks» y «backups». Esos términos provienen de la implementación y no ayudan a la mayoría de los usuarios a decidir qué registrar o cómo gestionar sus datos. En privacidad, registro, ubicación, retención, exportación y copias están concentrados en un bloque de texto pequeño.

La información debe mantenerse, pero con una estructura que permita encontrar rápidamente «Qué se guarda», «Dónde está» y «Qué conserva el borrado». Por ejemplo, «La aplicación registra actividad en este día; puede haber períodos sin seguimiento» comunica el límite sin exigir conocer el tracker.

**Ubicación:** `assets/dashboard.js:409` y `assets/settings.html:3`.

**Corrección:** lenguaje de usuario, «copias de seguridad» en lugar de «backups» y detalles técnicos en ayuda secundaria. Agrupar privacidad en frases breves cerca de sus acciones, conservando el aviso de retención de copias y exportaciones.

**Aceptación:** un usuario puede explicar qué se registra y qué se elimina leyendo la pantalla, sin interpretar términos de desarrollo.

## Próximo orden de trabajo

1. Resolver la transición de foco del error y comprobar entrada/repetición/recuperación.
2. Separar el estilo del agregado y probar cambios de ranking y fecha.
3. Reducir la densidad de ayuda y leyenda; revisar de nuevo 360 × 480 y la ventana habitual.
4. Validar los flujos completos en WebView2 con teclado y Narrador; revisar DPI y alto contraste.

No hace falta otro rediseño general. La base actual permite una ronda acotada de correcciones y simplificación antes de la validación nativa final.

## Aplicación de esta pasada — 30/09/2026

Se implementaron los cuatro hallazgos sin cambiar la contabilización ni la fórmula del promedio:

- **A:** el error guarda el control de origen y enfoca su encabezado una sola vez. Los errores repetidos conservan el foco en Reintentar u otra acción. Al recuperar se restaura el origen si es válido, o un control visible de la vista; si la persona navegó en Configuración, se conserva ese contexto. Pruebas de entrada/repetición/recuperación desde gráfico y selector, desaparición del origen y navegación en configuración. El diagnóstico real de DOM en Edge confirma `load-error-title → retry-diagnostic → chart`.
- **B:** gris reservado a Otras. Las cinco categorías de aplicaciones visibles tienen colores diferentes, incluso tras consultar más de diez apps, reordenar, cambiar fechas o alternar Día/Semana. La asignación se conserva durante la consulta cuando no colisiona con otra categoría visible; no se promete identidad persistente entre aperturas. Filas, leyenda e inspector se sincronizan con esos cambios.
- **C:** cobertura breve y método completo en la ayuda nativa desplegable Sobre estos datos. Allí está también la explicación del cierre. La apertura y el foco de la ayuda sobreviven a snapshots. La leyenda mantiene todas las categorías y omite duraciones repetidas. Con el mismo fixture y viewport 360 × 480, la primera app pasó de 610 a 413 px en Día y de 664 a 413 px en Semana; una fila completa es visible en ambos modos y el ancho del contenido permanece en 360 px. En 780 × 820 se ven dos filas completas y el comienzo de la tercera en Semana. Capturas y mediciones en `auditoria-segunda-pasada-2026-09-30/`.
- **D:** Configuración agrupa Qué se guarda, Dónde está y Qué elimina el borrado; cada acción queda junto a su explicación. Se usa copias de seguridad en lugar de backups y no se exponen hooks ni tracker como vocabulario de ayuda. La ruta heredada HorariosTracker se conserva por compatibilidad. El aviso mantiene explícita la retención de preferencias, exportaciones y copias del instalador.

Backup de la versión anterior a esta pasada: `<respaldo-local>/screen-time-backup-segunda-pasada-20260930-211358`, con código, documentación, paquete previo y los ejecutables compilado e instalado.

La evidencia automatizada y los renders no sustituyen los recorridos pendientes en WebView2 con teclado y Narrador, DPI, alto contraste, varios monitores ni los ensayos reales de energía y gestión de datos. Esta aplicación de cambios no declara aprobación general para distribución.

Comprobaciones finales de esta aplicación: 30 pruebas de interfaz y 35 pruebas Rust aprobadas (dos benchmarks ignorados), sintaxis JavaScript y formato Rust correctos, compilación Release aprobada. Se actualizó la instalación local, se conservó autoarranque y quedó una instancia; el SHA-256 instalado coincide con el compilado. La evidencia de esta intervención amplía el alcance inicial de la revisión, que no había instalado cambios.
