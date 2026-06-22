# Pylos — Pasarela LLM y Proxy MCP en Rust (Empresarial)

<p align="center">
  <a href="README.md"><b>English</b></a> | 
  <a href="README.fr.md"><b>Français</b></a> | 
  <a href="README.es.md"><b>Español</b></a>
</p>

Pylos es una pasarela (gateway) de IA de alto rendimiento y ultra baja latencia escrita en Rust. Funciona como un proxy unificado y seguro para más de 20 proveedores de LLM, ofreciendo un reemplazo directo y compatible con las SDK de OpenAI. Con gobernanza integrada, gestión de costos, salvaguardas de privacidad y un moderno panel de administración en React, Pylos ayuda a los equipos a escalar y monitorizar sus flujos de trabajo de IA de forma segura.

---

## 🎯 Beneficios Clave y Propuesta de Valor

- **⚡ Rendimiento Ultrarrápido:** Escrito en Rust con E/S asíncronas (Axum y Tokio), agregando menos de `< 2ms` de sobrecarga.
- **💰 Control de Costos y Presupuestos:** Evita facturas inesperadas con seguimiento de tokens en tiempo real, presupuestos semanales/mensuales en USD y límites de velocidad asociados a cada clave virtual.
- **🛡️ Seguridad y Privacidad Empresarial:** Autenticación mediante Google OAuth (con alternativa de clave estática `PYLOS_ADMIN_KEY`) para asegurar el panel de control. En el flujo de datos, salvaguardas automáticas enmascaran datos personales (PII) y bloquean contenido sensible.
- **🔄 Resiliencia sin Interrupciones:** Alternativas automáticas multiproveedor, patrón de disyuntor (circuit breaker) y reintentos automáticos con retroceso exponencial garantizan que tus integraciones de IA nunca fallen.
- **🌐 Gestión Multi-Tenant:** Organiza tus usuarios en Organizaciones y Equipos, y asigna Claves Virtuales (`sk-pylos-*`) con accesos restringidos por modelo y proveedor.
- **🔌 Soporte dinámico MCP (Model Context Protocol):** Conecta agentes con servidores de herramientas de terceros mediante un proxy MCP dinámico.

---

## ✨ Características Principales

- **Pasarela de IA Unificada:** Reemplazo directo para endpoints de OpenAI (`/v1/chat/completions`, `/v1/embeddings`, etc.) compatible con OpenAI, Anthropic, AWS Bedrock, Google Gemini, DeepSeek, Groq, Ollama, OpenRouter y más.
- **Enrutamiento Inteligente:** Enrutamiento dinámico según los modelos, balanceo de carga ponderado y reglas de enrutamiento CEL (Common Expression Language).
- **Gobernanza Estricta:** Claves virtuales con límites RPM/TPM configurables y ventanas de presupuesto activas.
- **Observabilidad Integrada:** Métricas de Prometheus (`/metrics`), trazas distribuidas con OpenTelemetry e histórico de peticiones (SQLite/Postgres) con histogramas de consumo de tokens.
- **Actualización en Caliente:** Recarga la configuración de modelos, claves virtuales y proveedores sin reiniciar el servidor (`POST /config/reload`).
- **Interfaz de Administración Moderna:** Panel de control en React 19 + Vite 8 para gestionar claves, analizar registros y configurar reglas de seguridad.
- **Caché y Optimización de Tokens:** Caché de prefijo en memoria (política de desalojo **TinyLFU** mediante `moka`) y caché semántica (búsqueda vectorial por **similitud de coseno** mediante **Qdrant**) incorporados para evitar peticiones redundantes y abaratar el costo en tokens.
- **Grafo de Memoria Cross-Agent:** Memoria a largo plazo potenciada por **Memgraph** (mediante el protocolo Neo4j Bolt) que utiliza consultas **Cypher** para almacenar y recuperar dinámicamente grafos de entidades-relaciones vinculados a Claves Virtuales.
- **RAG (Generación Aumentada por Recuperación) Integrado:** Inyección automática de contexto a partir de colecciones vectoriales de documentos o correos electrónicos almacenadas en **Qdrant** antes de delegar la petición a los LLM.

---

## 🏗️ Architecture

Pylos utiliza una arquitectura hexagonal (puertos y adaptadores):

```
┌─────────────────────────────────────────────────────┐
│                   pylos-server                       │
│           Axum HTTP/WS server, routes, middleware    │
│├───────────────────────────────────────────────────┤│
│                 pylos-application                     │
│     Use cases, orchestration, stores, plugins        │
│├───────────────────────────────────────────────────┤│
│               pylos-infrastructure                   │
│      Provider adapters (OpenAI, Anthropic, etc.)     │
│├───────────────────────────────────────────────────┤│
│                   pylos-core                         │
│         Domain entities, traits, config types        │
└─────────────────────────────────────────────────────┘
```

---

## 🚀 Inicio Rápido

### 1. Ejecución Local (Desarrollo)

```bash
# Clonar el repositorio
git clone <repo-url> && cd Pylos

# Instalar herramientas de desarrollo y dependencias
make setup

# Configurar el entorno
cp .env.example .env
# Edita .env con tus claves de API (OPENAI_API_KEY, PYLOS_ADMIN_KEY, etc.)

# Iniciar el servidor backend y la interfaz UI
make run
```

### 2. Configuración (`pylos.json`)

Pylos utiliza un archivo de configuración centralizado `pylos.json` para el enrutamiento, la gobernanza, los presupuestos y la configuración de complementos. Para obtener una guía completa de todas las claves de configuración disponibles, referencias a variables de entorno y configuraciones de proveedores, consulte la [Guía de configuración de Pylos](docs/pylos-config.md).

### 3. Docker Compose (Pila Completa)

Inicia Pylos, la interfaz de usuario, Prometheus y Grafana con un solo comando:

```bash
docker compose up -d
```
Accede a la pasarela en `http://localhost:3000` y al panel de administración en `http://localhost:8080`.

---

## ⚡ Caché y Optimización de Tokens

Pylos minimiza la latencia y los costos de los proveedores de LLM usando dos estrategias de caché complementarias:

1. **Caché de Prefijo en Memoria (Desalojo TinyLFU):**
   - **Algoritmo:** Gestionado por una política de desalojo de caché de alta concurrencia **TinyLFU** mediante la biblioteca Rust `moka`.
   - **Cómo funciona:** Almacena la respuesta exacta basada en el ID del modelo y el historial de mensajes de la consulta. Las peticiones idénticas posteriores omiten el LLM por completo, ahorrando el **100% de los tokens de entrada y salida**.
2. **Caché Semántica (Búsqueda Vectorial por Similitud de Coseno):**
   - **Algoritmo:** Utiliza representaciones vectoriales (embeddings) mapeadas en una colección de **Qdrant**, consultadas mediante una métrica de **similitude de coseno**.
   - **Cómo funciona:** Si una nueva petición coincide semánticamente con una consulta ya registrada (con una similitud que supere el umbral configurado, por ejemplo, `0.92`), la respuesta almacenada se devuelve inmediatamente. Esto detecta intenciones similares formuladas de forma diferente, ahorrando el **100% de los costos del LLM**.

---

## 🧠 Grafo de Memoria y RAG

Pylos incorpora complementos (plugins) para la recuperación dinámica de contexto y la memoria a largo plazo de los agentes:

1. **Grafo de Memoria Cross-Agent (Memgraph):**
   - **Tecnología:** Se conecta a la base de datos de grafos **Memgraph** mediante el protocolo **Bolt** (biblioteca `neo4rs`) y consultas **Cypher**.
   - **Cómo funciona:** 
     - **Pre-hook:** Intercepta los mensajes entrantes, consulta Memgraph en busca de entidades y relaciones asociadas con la `VirtualKey` activa, y las inyecta en el prompt de contexto del sistema.
     - **Post-hook:** Analiza las salidas de los modelos en busca de etiquetas `<memory>EntidadA|RELACIÓN|EntidadB</memory>`, extrae los nuevos datos y los fusiona (`MERGE`) en el grafo de la base de datos.
2. **Generación Aumentada por Recuperación (RAG):**
   - **Tecnología:** Se integra con **Qdrant** para la búsqueda vectorial y la recuperación de información basada en embeddings.
   - **Cómo funciona:** Al enviar peticiones a modelos específicos como `graphon-rag-emails` o `mnemosyne-search`, Pylos genera en primer lugar el embedding de la consulta del usuario, realiza la búsqueda en las colecciones vectoriales de Qdrant (correos electrónicos o archivos), construye un prompt de contexto aumentado con los resultados, y redirige la petición final al LLM.

---

## 🛡️ Autenticación y Control de Acceso

Pylos soporta dos métodos de autenticación para la administración:
1. **Google OAuth (SSO):** Conexión a través de cuentas Google corporativas. El primer usuario en iniciar sesión obtiene automáticamente el rol `admin` (bootstrap). Los siguientes se crean con el rol `member`, permitiendo a un administrador asignarlos a Equipos y Organizaciones más tarde.
2. **Alternativa Clave Admin (Fallback):** Si Google OAuth no está configurado o no está disponible, se puede usar la clave estática `PYLOS_ADMIN_KEY` configurada en las variables de entorno para acceder al panel.

---

## 📊 Referencia de la API

### Endpoints de Inferencia

| Método | Ruta | Descripción |
|---|---|---|
| `POST` | `/v1/chat/completions` | Completions de chat unarias o en streaming |
| `POST` | `/v1/embeddings` | Generación de vectores de embeddings |
| `POST` | `/v1/images/generations` | Generación de imágenes |
| `GET` | `/v1/models` | Lista de modelos disponibles |

### Endpoints de Gestión (Requiere Autenticación)

| Método | Ruta | Description |
|---|---|---|
| `GET/POST` | `/providers` | Registro y gestión de proveedores LLM |
| `GET/POST` | `/virtual-keys` | Gestión de claves virtuales, límites y presupuestos |
| `GET` | `/api/logs/stats` | Vista general y métricas de uso del panel |
| `POST` | `/config/reload` | Recarga dinámica de la configuración |

---

## 🛠️ Comandos de Desarrollo

```bash
make test         # Ejecutar pruebas unitarias y de integración
make lint         # Ejecutar comprobaciones de clippy
make audit        # Analizar vulnerabilidades en las dependencias
```

## 📄 Licencia

TBD
