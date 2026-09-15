# hive-colony Helm chart (lab)

Chart de despliegue para un clúster de LABORATORIO que controlas. La colonia
es una topología **node-local**: los agentes comparten un segmento POSIX shm
(`shm_open`) que vive en el `/dev/shm` del nodo, así que **todos los pods
deben correr en el mismo nodo** (usa `nodeSelector`/`affinity`).

## Construcción y publicación de la imagen

El chart no construye nada: consume una imagen que tú publicas.

```bash
# 1. Construir la imagen (multi-stage, compila el workspace entero dentro)
docker build -t hive-colony:latest .

# 2. (Opcional) Subirla a tu registry y fijar el tag en values.yaml
docker tag hive-colony:latest registry.example.com/lab/hive-colony:3.0.0
docker push registry.example.com/lab/hive-colony:3.0.0
```

```yaml
# values.yaml
image:
  repository: registry.example.com/lab/hive-colony
  tag: "3.0.0"
  pullPolicy: IfNotPresent
```

El binario `stinger` (lanzador fileless, gated por `HIVE_LAB_AUTHORIZED=1`)
no se incluye en la imagen por diseño.

## Instalación

```bash
helm install hive deploy/charts/hive -f your-values.yaml
```

## Qué despliega

| Componente | Nota |
|------------|------|
| `hive-c2` Deployment + Service `:8444` | API del operador (auth `x-api-key` opcional, rate-limit, CORS cerrado) |
| `queen`, `worker`, `drone`, `honeybee` | agentes de la colonia, attach al shm `hive_arena` vía `/dev/shm` (hostPath del nodo) |

## Seguridad (defaults de ronda 10)

- `securityContext`: `privileged: false`, `runAsNonRoot: true`,
  `allowPrivilegeEscalation: false`, capabilities `drop: ALL`.
  `memfd_create`, `shm_open` y los escaneos de solo lectura funcionan sin
  privilegios; no flipes esto salvo necesidad explícita del lab.
- `hostPID`/`hostNetwork`: `false` por defecto (knobs `podSecurity.*`).
- RBAC: solo ServiceAccount. El ClusterRole privilegiado anterior no tenía
  consumidor (ningún binario habla con la API de k8s) y se eliminó.
- El ConfigMap solo expone variables que el código realmente lee
  (`HIVE_C2_URL`, `HIVE_C2_DNS_DOMAIN`, `HIVE_C2_DEAD_DROP_TOKEN`).
- La arena es shm (shm_open): el nombre debe ser válido para `shm_open`
  (una barra inicial, sin más barras) — por eso el default es `hive_arena`
  y no una ruta de fichero.
