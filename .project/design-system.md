# Design System — Migration Plan

## Status: Draft

## Goal

Replace the monolithic 889-line `index.css` with a modular design system built on **Tailwind CSS v4** and **shadcn/ui**. Eliminate all inline `style={{}}` props. Every visual decision flows from design tokens defined in one place.

---

## 1. Technology Stack

| Tool | Role | Why |
|------|------|-----|
| **Tailwind CSS v4** | Utility classes + design tokens via `@theme` | CSS-first config, no PostCSS, no `tailwind.config.js`. Vite plugin. |
| **@tailwindcss/vite** | Build integration | Dedicated Vite plugin — faster than PostCSS pipeline. |
| **shadcn/ui** | Accessible component primitives (Button, Tabs, Card, Badge, Dialog, Tooltip) | Copies source into project (not a dependency). Built on Radix UI. Styled with Tailwind. |
| **lucide-react** | Icons | shadcn default icon library. Lightweight, tree-shakeable. |
| **tailwind-merge** | Class conflict resolution | Required by shadcn's `cn()` utility. |
| **class-variance-authority** | Component variants | Required by shadcn for variant props (size, color, state). |
| **clsx** | Conditional class names | Lightweight, used by `cn()` helper. |

---

## 2. Installation Steps

### 2a. Tailwind CSS v4 + Vite Plugin

```bash
cd observer
bun add -d tailwindcss @tailwindcss/vite
```

Update `vite.config.ts`:
```ts
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: { /* existing proxy config */ },
});
```

### 2b. Path Aliases (required by shadcn)

`tsconfig.app.json` — add:
```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}
```

### 2c. shadcn/ui

```bash
cd observer
npx shadcn@latest init
```

Settings for `components.json`:
- Style: **new-york**
- RSC: **false** (Vite, not Next.js)
- TSX: **true**
- CSS path: **src/styles/theme.css**
- Base color: **neutral**
- CSS variables: **true**
- Aliases: components → `@/components`, ui → `@/components/ui`, lib → `@/lib`, hooks → `@/hooks`
- Icon library: **lucide**

Then install components as needed:
```bash
npx shadcn@latest add button badge card tabs tooltip dialog
```

---

## 3. Design Token Architecture

### File Structure

```
observer/src/styles/
  theme.css                ← Entry point. @import "tailwindcss" + @import layers
  tokens/
    colors.css             ← All color tokens: backgrounds, text, semantic, chart palette
    typography.css          ← Font families, sizes, weights, letter-spacing, line-height
    spacing.css             ← Spacing scale, border radii, border widths
    motion.css              ← Transitions, keyframes, durations, easing curves
    elevation.css           ← Shadows, glows, z-index scale
  base/
    reset.css               ← Box-sizing, scrollbar styling, html/body defaults
    prose.css               ← Base text styles, link styles (if needed later)
  layers/
    d3.css                  ← Styles for D3 chart containers, axes, tooltips, radar charts
```

### theme.css (entry point)

```css
@import "tailwindcss";

/* Token layers */
@import "./tokens/colors.css";
@import "./tokens/typography.css";
@import "./tokens/spacing.css";
@import "./tokens/motion.css";
@import "./tokens/elevation.css";

/* Base */
@import "./base/reset.css";

/* D3-specific (can't be done with utility classes) */
@import "./layers/d3.css";
```

### tokens/colors.css

Maps current `:root` variables to Tailwind v4 `@theme` tokens:

```css
@theme {
  /* Backgrounds */
  --color-bg-primary: #0d1117;
  --color-bg-secondary: #161b22;
  --color-bg-tertiary: #1c2128;
  --color-bg-elevated: #21262d;

  /* Borders */
  --color-border-primary: #30363d;
  --color-border-secondary: #21262d;

  /* Text */
  --color-text-primary: #c9d1d9;
  --color-text-secondary: #8b949e;
  --color-text-muted: #484f58;
  --color-text-accent: #58a6ff;

  /* Semantic */
  --color-success: #3fb950;
  --color-warning: #d29922;
  --color-danger: #f85149;
  --color-info: #58a6ff;

  /* Agent vitals */
  --color-energy: #f0c040;
  --color-health: #3fb950;
  --color-hunger: #f85149;

  /* Event categories */
  --color-lifecycle: #bc8cff;
  --color-economy: #f0c040;
  --color-social: #58a6ff;
  --color-world: #3fb950;
  --color-knowledge: #ff7b72;
  --color-system: #8b949e;
  --color-environment: #79c0ff;

  /* Seasons */
  --color-spring: #3fb950;
  --color-summer: #f0c040;
  --color-autumn: #db6d28;
  --color-winter: #79c0ff;

  /* Chart palette (colorblind-safe) */
  --color-chart-1: #58a6ff;
  --color-chart-2: #3fb950;
  --color-chart-3: #f0c040;
  --color-chart-4: #bc8cff;
  --color-chart-5: #ff7b72;
  --color-chart-6: #79c0ff;
  --color-chart-7: #db6d28;
  --color-chart-8: #f778ba;

  /* Map-specific */
  --color-ocean: #060e1a;
  --color-land: #121c2b;
  --color-shelf: #0d1f35;
  --color-coast: #1e3a55;

  /* Relationship */
  --color-positive: #3fb950;
  --color-neutral: #8b949e;
  --color-negative: #f85149;
}
```

These become usable as `bg-bg-primary`, `text-text-accent`, `border-border-primary`, etc.

### tokens/typography.css

```css
@theme {
  --font-mono: "Cascadia Code", "Fira Code", "JetBrains Mono", "Consolas", "Monaco", monospace;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;

  --text-2xs: 0.7rem;
  --text-xs: 0.8rem;
  --text-sm: 0.875rem;
  --text-base: 1rem;
  --text-lg: 1.25rem;
  --text-xl: 1.5rem;
}
```

### tokens/spacing.css

```css
@theme {
  --spacing-xs: 4px;
  --spacing-sm: 8px;
  --spacing-md: 12px;
  --spacing-lg: 16px;
  --spacing-xl: 24px;

  --radius-sm: 4px;
  --radius-md: 6px;
  --radius-lg: 8px;
}
```

### tokens/motion.css

```css
@theme {
  --ease-default: cubic-bezier(0.2, 0, 0, 1);
  --ease-snappy: cubic-bezier(0.3, 0, 0, 1);
  --animate-pulse: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
```

### tokens/elevation.css

```css
@theme {
  --shadow-glow-success: 0 0 4px #3fb950;
  --shadow-glow-warning: 0 0 4px #d29922;
}
```

---

## 4. What Gets Replaced by shadcn

| Current CSS class | shadcn component | Notes |
|---|---|---|
| `.dashboard-tab` / `.dashboard-tabs` | `<Tabs>` + `<TabsList>` + `<TabsTrigger>` | Accessible keyboard nav for free |
| `.badge` (.alive, .dead, .era, .season-*) | `<Badge variant="...">` | Define variants: alive, dead, era, spring, summer, autumn, winter |
| `.panel` / `.panel-header` / `.panel-body` | `<Card>` + `<CardHeader>` + `<CardContent>` | May need custom variant for the dense dashboard look |
| `.filter-btn` | `<Button variant="outline" size="sm">` | Active state via data attribute or variant |
| `.search-input` | `<Input>` | Style with tokens |
| `.stat-card` | `<Card>` variant or custom component | Small enough to be a Tailwind-only component |
| `.empty-state` / `.loading-state` / `.error-state` | Custom components with Tailwind classes | No shadcn equivalent needed |

### What stays as custom CSS (in `layers/d3.css`)

These are D3-rendered SVG elements that can't use utility classes:
- `.chart-container` axis/grid styling
- `.d3-tooltip` positioning
- `.radar-chart` fills/strokes
- WorldMap SVG is fully imperative (D3 attrs) — no CSS needed

---

## 5. Component Migration Order

Priority: migrate the **shell first** (shared by all tabs), then individual tab components.

### Phase A — Foundation (do first)
1. Install Tailwind v4 + Vite plugin
2. Install shadcn/ui + dependencies
3. Create token files (colors, typography, spacing, motion, elevation)
4. Create `theme.css` entry point with `@import "tailwindcss"` + token imports
5. Create `base/reset.css` (scrollbar styles, html/body/root)
6. Create `layers/d3.css` (chart container, tooltip, radar styles)
7. Create `src/lib/utils.ts` with `cn()` helper (required by shadcn)
8. Delete old `index.css` (replaced entirely by theme.css + token files)
9. Update `src/main.tsx` to import `./styles/theme.css` instead of `./styles/index.css`
10. Verify: `bun run build` passes, app renders

### Phase B — Dashboard Shell
1. Convert `App.tsx` layout: replace `.dashboard`, `.dashboard-header`, `.dashboard-content` classes with Tailwind utilities
2. Replace `.dashboard-tabs` with shadcn `<Tabs>` component
3. Replace `.panel` / `.panel-header` / `.panel-body` with shadcn `<Card>` (or keep as a thin wrapper using Tailwind)
4. Replace `.badge` with shadcn `<Badge>` + custom variants
5. Replace `.connection-status` / `.connection-dot` with Tailwind utilities
6. Replace `.header-metric` with Tailwind utilities
7. Remove all `style={{}}` inline props from App.tsx

### Phase C — Shared UI Components
1. Replace `.search-input` with shadcn `<Input>`
2. Replace `.filter-btn` / `.filter-group` with shadcn `<Button variant="outline">`
3. Replace `.vital-bar` with a `<VitalBar>` component using Tailwind classes
4. Replace `.stat-card` / `.stat-row` with Tailwind utility classes
5. Replace `.item-list` with Tailwind utility classes
6. Replace `.section-header` with Tailwind utility classes
7. Replace `.empty-state` / `.loading-state` / `.error-state` with Tailwind

### Phase D — Tab Components (one at a time)
1. **AgentInspector** — largest, most CSS-heavy (vitals, inventory, skills, memory, knowledge tags)
2. **EconomyMonitor** — stat cards, chart containers
3. **Timeline** — event list, event type colors
4. **PopulationTracker** — stat cards, charts
5. **DiscoveryLog** — discovery entries
6. **SocialGraph** — mostly D3/SVG, minimal CSS wrapper
7. **WorldMap** — remove remaining `style={{}}` inline props from JSX wrapper/legend

For each: replace `className="old-class"` and `style={{}}` with Tailwind utility classes. Delete corresponding CSS from old files.

---

## 6. Inline Style Elimination

Every `style={{}}` prop in the codebase must be converted to Tailwind utility classes or extracted into a component. Current offenders:

- **WorldMap.tsx** — panel wrapper, panel-body, SVG container, legend overlay
- **App.tsx** — header metrics, season/era badge inline colors
- **AgentInspector.tsx** — detail panel layout, vital bars, inventory grid
- **EconomyMonitor.tsx** — chart sizing
- **Timeline.tsx** — event entry layout
- **PopulationTracker.tsx** — chart sizing
- **SocialGraph.tsx** — SVG container
- **DiscoveryLog.tsx** — entry layout

Rule after migration: **no `style={{}}` props except for truly dynamic values** (e.g., D3-computed SVG attributes, percentage-based vital bar widths via `style={{ width: pct }}`).

---

## 7. File Structure After Migration

```
observer/src/
  styles/
    theme.css                    ← Entry: @import "tailwindcss" + token/base/layer imports
    tokens/
      colors.css
      typography.css
      spacing.css
      motion.css
      elevation.css
    base/
      reset.css
    layers/
      d3.css
  components/
    ui/                          ← shadcn components (auto-generated, customized)
      badge.tsx
      button.tsx
      card.tsx
      tabs.tsx
      input.tsx
      tooltip.tsx
      dialog.tsx
    AgentInspector.tsx
    DiscoveryLog.tsx
    EconomyMonitor.tsx
    PopulationTracker.tsx
    SocialGraph.tsx
    Timeline.tsx
    WorldMap.tsx
  lib/
    utils.ts                     ← cn() helper
  hooks/
    ...
```

---

## 8. Verification Checklist

After each phase:
- [ ] `bun run build` — zero errors
- [ ] `bun run lint` — zero errors/warnings
- [ ] No `style={{}}` props in converted components (except dynamic values)
- [ ] All colors reference design tokens (no hardcoded hex in TSX)
- [ ] Visual regression: app looks identical to before migration
- [ ] `index.css` is fully deleted by end of Phase A

---

## 9. Notes

- **Tailwind v4 requires Node.js 20+** — verify environment.
- **No `tailwind.config.js`** — all config lives in CSS via `@theme`.
- **No PostCSS** — the `@tailwindcss/vite` plugin handles everything.
- **shadcn copies code** — components live in `src/components/ui/` and are fully editable.
- **D3 visualizations** (WorldMap, SocialGraph, charts) stay imperative SVG. Only their React wrapper JSX gets converted to Tailwind.
- The `@theme` directive makes tokens available as both CSS custom properties (`var(--color-*)`) and Tailwind utility classes (`bg-bg-primary`, `text-text-accent`).
