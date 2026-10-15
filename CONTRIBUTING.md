# Contribuir

Gracias por interesarte en Screen Time. El proyecto está en desarrollo y prioriza un daemon nativo liviano, datos locales y un dashboard que solo se crea cuando se solicita.

## Antes de enviar cambios

1. Abre un issue para cambios grandes de arquitectura o comportamiento y explica el caso de uso.
2. Para un bug, incluye la versión de Windows, los pasos para reproducirlo y el resultado esperado. No subas tu historial CSV, nombres de usuario, rutas personales ni capturas con datos privados.
3. Mantén los cambios acotados. Evita servidores locales, telemetría, red en tiempo de ejecución, polling de foco y dependencias nuevas sin justificar.
4. Actualiza documentación y `CHANGELOG.md` cuando cambies formato, instalación, permisos o comportamiento visible.
5. Ejecuta formato, compilación y pruebas en Windows:

   ```bat
   cargo fmt --all -- --check
   cargo build --release --locked --target x86_64-pc-windows-msvc
   cargo test --locked --target x86_64-pc-windows-msvc
   ```

6. Describe qué validaste manualmente; no indiques que suspensión, cierre abrupto, RAM o CPU fueron validados si no los mediste.

## Guías de implementación

- El dashboard se distribuye en HTML, CSS y JavaScript local dentro de `assets/`; no dependas de un servidor HTTP.
- Los eventos Win32 y el estado del tracker se coordinan desde `src/main.rs`; la durabilidad y monitorización de sesión están separadas en sus módulos.
- Mantén accesibilidad de teclado, texto en español y soporte de tema claro/nocturno.
- El CSV es historial del usuario: no lo edites, borres, adjuntes ni envíes en PRs.
- Conserva las licencias y avisos de Chart.js y crates de terceros.

## Pull requests

Incluye un resumen, motivo, pasos de validación y una nota de riesgos o limitaciones. No incluyas binarios generados del directorio `target/`.

Al contribuir aceptas que tus cambios se publiquen bajo la licencia MIT del repositorio.
