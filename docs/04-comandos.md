# Comandos y eventos de Tauri

La interfaz habla con Rust solo con estos comandos (`invoke`) y eventos (`listen`). Nombres de comando en `snake_case`; argumentos y campos de respuesta en `camelCase` (`#[serde(rename_all = "camelCase")]`). Los tipos TypeScript de este documento se copian literalmente en `app/src/types.ts`.

## 1. Tipos comunes

```ts
export type Tool = 'claude-code' | 'antigravity' | 'opencode';
export type Provider = 'anthropic' | 'google' | 'deepseek' | 'other';
export type PlanId = 'claude-pro' | 'google-ai-pro' | 'google-ai-pro-claude' | 'deepseek';
export type CostKind = 'real' | 'api_equivalent' | 'unknown';
export type WindowName = '5h' | 'weekly';

export interface Filter {
  from: string;            // ISO 8601 UTC
  to: string;              // ISO 8601 UTC, exclusivo
  tool?: Tool;
  provider?: Provider;
  project?: string;
}

export interface CommandError {
  code: 'db' | 'io' | 'parse' | 'secret' | 'http' | 'invalid';
  message: string;         // en español, sin rutas absolutas ni claves
}

export interface Tokens { input: number; cacheWrite: number; cacheRead: number; output: number; }
```

Todos los comandos devuelven `Promise<T>` y, si fallan, rechazan con `CommandError`.

## 2. Comandos

### `get_dashboard(filter: Filter) → Dashboard`

```ts
export interface Kpi { value: number; deltaPct: number | null; spark: number[]; } // spark: un valor por día del rango
export interface Dashboard {
  kpis: {
    inputTotal: Kpi;       // input + cacheRead + cacheWrite
    cachePct: number;      // cacheRead / inputTotal × 100
    output: Kpi;
    outputPerTurn: number;
    turns: Kpi;
    sessions: number;
    activeSecs: Kpi;
    maxConcurrency: number;
  };
  byDay: { date: string; inputTotal: number; output: number }[];   // date = 'YYYY-MM-DD' local
  daysWithUse: number;
  heatmap: number[][];     // 7 × 24, fila 0 = lunes, valores = tokens totales
  peak: { weekday: number; hour: number } | null;
  byModel: { model: string; provider: Provider; tokens: Tokens; turns: number; costUsd: number | null }[];
  byProvider: { provider: Provider; pct: number }[];
  byTool: { tool: Tool; tokens: Tokens; activeSecs: number }[];
}
```

### `get_costs(filter: Filter) → CostsView`

```ts
export interface CostsView {
  realUsd: number;                 // DeepSeek
  apiEquivalentUsd: number;        // suscripciones
  feesUsd: number;                 // cuotas prorrateadas al rango
  yield: { planId: PlanId; name: string; feeUsd: number; equivalentUsd: number; ratio: number | null }[];
  byDay: { date: string; provider: Provider; kind: CostKind; usd: number }[];
  byModel: { model: string; kind: CostKind; usd: number; turns: number }[];
  byTool: { tool: Tool; usd: number }[];
  byProject: { project: string; usd: number; sessions: number; avgPerSession: number }[];
  monthProjection: { realUsd: number; apiEquivalentUsd: number };
  deepseekBudget: { budgetUsd: number; spentUsd: number; balanceUsd: number | null } | null;
  unpriced: { model: string; events: number }[];
}
```

### `get_sessions(filter: Filter) → SessionRow[]`

```ts
export interface SessionRow {
  sessionId: string; tool: Tool; project: string | null; mainModel: string;
  start: string; end: string; activeSecs: number; turns: number;
  tokens: Tokens; costUsd: number | null; kind: CostKind; anomalous: boolean;
}
```

### `get_session_detail(sessionId: string) → CallRow[]`

```ts
export interface CallRow { ts: string; model: string; tokens: Tokens; reasoning: number; costUsd: number | null; kind: CostKind; }
```

### `get_plans() → PlanView[]`

```ts
export interface WindowStatus {
  name: WindowName; used: number; limit: number | null; percent: number | null;
  resetsAt: string | null; exhaustsAt: string | null; unpricedEvents: number;
  calibratedBy: 'agy' | 'user' | null;   // origen de la última calibración
  calibratedAt: string | null;
}
export interface PlanView {
  id: PlanId; name: string; kind: 'subscription' | 'payg';
  monthlyFeeUsd: number; budgetUsd: number | null;
  windows: WindowStatus[];          // vacío en payg
  monthSpentUsd: number;            // real (payg) o equivalente (subscription)
  balanceUsd: number | null;        // solo deepseek
  tools: Tool[];
}
```

### `calibrate_plan(planId: PlanId, window: WindowName, percent: number) → WindowStatus`

`percent` en [1, 100]. Fuera de rango → `{ code: 'invalid', message: 'El porcentaje debe estar entre 1 y 100' }`. Sin uso en la ventana → `{ code: 'invalid', message: 'No hay uso en esta ventana: usa la herramienta y vuelve a calibrar' }`.

### `refresh_agy_usage() → PlanView[]`

Ejecuta `agy -p "/usage" --output-format json` y calibra `google-ai-pro` y `google-ai-pro-claude` (`06` §1.4). Sin `agy` en el PATH → `{ code: 'unavailable', message: 'No se encontró agy: calibra a mano en Antigravity › View Usage' }`. Más de 30 s → `{ code: 'timeout', message: 'agy no respondió: se reintentará en 15 min' }`. La app lo llama también sola cada 15 min si `agyAutoCalibrate` es `true`.

### `update_plan(planId: PlanId, monthlyFeeUsd: number, budgetUsd: number | null) → PlanView`

### `get_sources() → SourceView[]`

```ts
export interface SourceView {
  collector: Tool; root: string;     // relativa a %USERPROFILE%, p. ej. '.claude\\projects'
  found: boolean; files: number; events: number; lastEvent: string | null;
  errors: { file: string; line: number; reason: string }[];   // file relativo a root; máx. 50
}
```

### `rescan() → IngestStats`

```ts
export interface IngestStats { filesSeen: number; filesRead: number; newEvents: number; duplicates: number; lineErrors: number; }
```

### `get_prices() → Price[]` · `upsert_price(input: PriceInput) → RecalcResult` · `get_recalc_log() → RecalcEntry[]`

```ts
export interface Price {
  id: number; provider: Provider; modelPattern: string; validFrom: string; validTo: string | null;
  input: number; output: number; cacheWrite: number; cacheRead: number;   // $ por millón
  sourceUrl: string; approximate: boolean; userEdited: boolean;
}
export type PriceInput = Omit<Price, 'id' | 'userEdited' | 'validTo'>;
export interface RecalcResult { events: number; oldTotal: number; newTotal: number; }
export interface RecalcEntry extends RecalcResult { at: string; modelPattern: string; }
```

### `get_settings() → Settings` · `set_setting(key: keyof Settings, value: string) → Settings`

```ts
export interface Settings {
  theme: 'dark' | 'light' | 'system';
  autostart: boolean;
  alertWindow5hLevels: number[];     // [80, 95]
  alertWeeklyLevel: number;          // 80
  alertBudgetLevel: number;          // 80
  alertAnomaly: boolean;             // true
  agyAutoCalibrate: boolean;         // true: calibrar Google AI Pro con agy cada 15 min
  deepseekKeySaved: boolean;         // solo lectura
}
```

### `set_deepseek_key(key: string) → { saved: true }` · `delete_deepseek_key() → { saved: false }`

La clave se valida llamando a `/user/balance`; si responde 401 → `{ code: 'http', message: 'DeepSeek rechazó la clave' }` y no se guarda.

### `export_data(format: 'csv' | 'json', filter: Filter, path: string) → { rows: number }` · `import_data(path: string) → IngestStats`

`path` lo da el diálogo nativo (`plugin-dialog`); la interfaz no lee ni escribe archivos.

### `get_today_summary() → TraySummary`

```ts
export interface TraySummary { tokensToday: number; costTodayUsd: number; windows: { planName: string; percent: number | null }[]; }
```

## 3. Eventos (Rust → interfaz)

| Evento | Carga | Cuándo |
| --- | --- | --- |
| `usage-updated` | `{ newEvents: number }` | Tras una ingesta con eventos nuevos |
| `ingest-progress` | `{ done: number; total: number }` | Durante el escaneo inicial, cada 100 archivos |
| `prices-recalculated` | `RecalcResult` | Tras `upsert_price` |

## 4. Errores

| `code` | Cuándo | Mensaje de ejemplo |
| --- | --- | --- |
| `invalid` | Argumento fuera de rango | «El porcentaje debe estar entre 1 y 100» |
| `db` | Fallo de SQLite | «No se pudo leer la base de datos. Reinicia la app; si sigue, revisa Fuentes» |
| `io` | Carpeta o archivo inaccesible | «No se pudo leer .claude\projects» |
| `parse` | Importación corrupta | «El archivo no es una exportación válida del Gestor de Tokens» |
| `secret` | Administrador de credenciales | «Windows no permitió guardar la clave» |
| `http` | DeepSeek | «DeepSeek no respondió; se reintentará en 15 minutos» |
