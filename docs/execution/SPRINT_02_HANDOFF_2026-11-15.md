# Sprint 2 Handoff — 2026-11-15

## Estado al cierre

- PR [#602](https://github.com/scherenhaenden/FusionPanel/pull/602) está fusionado en `sprint-2`, merge commit `67eae339833c0da8adf116d57989a4f0fe75ae6e`. Su package pasó en run [43757289385](https://github.com/scherenhaenden/FusionPanel/actions/runs/43757289385). El agente tuvo 16/16 unit tests; el binario locked con Rustls, los tests de entorno/systemd del instalador, formato y sintaxis pasaron. Revisión independiente sin bloqueos.
- PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590) sigue abierto hacia `master`, en el head `67eae339833c0da8adf116d57989a4f0fe75ae6e`.
  - Package run [43758110106](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110106): PASS.
  - Browser E2E con Docker, job [132946572841](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110127/job/132946572841): PASS.
  - CI Qualification run [43758110127](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110127) seguía en curso: `crates/agent/Cargo.toml` falló (job `132946716176`) después de pasar en el head anterior; `tests/e2e/hosting` seguía ejecutándose (job `132946716331`). Inspeccionar el log del agente cuando termine; puede ser regresión del cambio TLS. **No fusionar #590 antes de resolver/entender esto.**
- En el run anterior, 44 jobs Cargo fallidos coincidieron por nombre con `master`; `tests/e2e/hosting` fue cancelado en ambas ramas. Volver a comparar el run final del head actual antes de atribuir fallos al sprint.
- El Browser E2E actual prueba registro local, job `system.info` exitoso y logs visibles, pero no reinicia API/agente. La persistencia tras ambos reinicios sigue en el DoD de Sprint 2 y falta probarse en el head final.
- PR [#585](https://github.com/scherenhaenden/FusionPanel/pull/585) sigue abierto hacia `sprint-3`. Su package falló con el error antiguo de `safe.directory`; actualizar su base tras integrar Sprint 2 y repetirlo.

## Trabajo siguiente

1. Esperar el resultado de CI Qualification [43758110127](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110127), inspeccionar el fallo de `crates/agent`, y corregirlo por PR atómico a `sprint-2` si es una regresión real. En ramas atómicas, correr solo unit tests del componente afectado.
2. Añadir al fixture Docker/Playwright la secuencia exigida: ejecutar un job, reiniciar API y agente, volver a abrir Jobs y comprobar que sigan `succeeded` el estado y los logs. Correr el E2E solo después de integrar esa rama en `sprint-2`.
3. Repetir el gate final de Sprint 2 en el head actualizado, documentar resultados reales y no fusionar #590 hasta resolver los fallos relevantes.
4. Después de #590, sincronizar `sprint-3` desde el nuevo `master`, revalidar y fusionar #585.
5. Mantener issue #24 abierto hasta merge a `master` y aceptación/evidencia correspondiente.

## Límite del artefacto de instalación

El tarball de desarrollo contiene API y agente, pero no `fusionpanel-cli` ni `frontend/dist`; una instalación limpia no puede hacer el bootstrap previsto ni servir la UI. El Browser E2E construye CLI/frontend desde el checkout y no cualifica ese tarball. También se detectó que `packaging/update.sh` y `packaging/install.sh` usan layouts de versión distintos. Mantener el artefacto marcado como desarrollo; no llamarlo instalación completa. Si se requiere un bundle instalable completo, abrir un PR atómico para empaquetar CLI/UI, configurar la ruta de assets, alinear updater/rollback y añadir un smoke test Docker de instalación limpia.

## Continuar desde otra PC

Tras clonar el repositorio, obtener esta documentación desde la rama publicada:

```sh
git fetch origin
git switch --track origin/docs/sprint-2-handoff
```

Si el PR del handoff ya fue integrado, usar la rama de Sprint 2:

```sh
git fetch origin
git switch --track origin/sprint-2
```
