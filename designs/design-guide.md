# FROST TSS Wallet — Design Guide

Reference design system for the FROST threshold signature scheme demo application. All values map to Tailwind CSS utility classes. The production frontend uses Tailwind v4 with the Geist font family.

---

## Color Palette

### Backgrounds

| Token              | Hex       | Tailwind Class        | Usage                          |
| ------------------ | --------- | --------------------- | ------------------------------ |
| `base`             | `#0a0a10` | `bg-[#0a0a10]`        | Page background                |
| `surface`          | `#111118` | `bg-surface`          | Card / panel default           |
| `surface-raised`   | `#16161f` | `bg-surface-raised`   | Elevated cards, table rows     |
| `surface-overlay`  | `#1c1c27` | `bg-surface-overlay`  | Hover states, badge background |

### Borders

| Token               | Hex       | Tailwind Class              | Usage                    |
| -------------------- | --------- | --------------------------- | ------------------------ |
| `border`             | `#2a2a3c` | `border-surface-border`     | Primary borders          |
| `border-subtle`      | `#222233` | `border-surface-border-subtle` | Dividers, inner borders |

### Text

| Token       | Hex       | Tailwind Class       | Usage                          |
| ----------- | --------- | -------------------- | ------------------------------ |
| `primary`   | `#f0f0f5` | `text-text-primary`  | Headings, key values           |
| `secondary` | `#9b9bb0` | `text-text-secondary`| Body text, descriptions        |
| `tertiary`  | `#6b6b80` | `text-text-tertiary` | Labels, captions               |
| `muted`     | `#4a4a5e` | `text-text-muted`    | Disabled, placeholder          |

### Brand / Accent

| Token        | Hex       | Usage                              |
| ------------ | --------- | ---------------------------------- |
| `frost-400`  | `#5888ff` | Links, selected indicators         |
| `frost-500`  | `#3366ff` | Button hover backgrounds           |
| `frost-600`  | `#1a44f5` | Primary button background          |
| `frost-700`  | `#1333e1` | Primary button hover               |

### Status Colors

| Status        | Color Hex  | Background (10% opacity)   | Badge Text Class     |
| ------------- | ---------- | -------------------------- | -------------------- |
| Waiting       | `#6b7280`  | `bg-surface-overlay`       | `text-text-muted`    |
| In Progress   | `#3b82f6`  | `bg-accent-blue/10`        | `text-accent-blue`   |
| Complete      | `#22c55e`  | `bg-accent-green/10`       | `text-accent-green`  |
| Error / Failed| `#ef4444`  | `bg-accent-red/10`         | `text-accent-red`    |

### Node Identity Colors

| Node   | Background             | Text Color        |
| ------ | ---------------------- | ----------------- |
| Node A | `bg-frost-600/15`      | `text-frost-400`  |
| Node B | `bg-purple-600/15`     | `text-purple-400` |

---

## Typography

### Font Families

| Role      | Family                           | Tailwind Class | Usage                              |
| --------- | -------------------------------- | -------------- | ---------------------------------- |
| Sans      | Inter / Geist Sans               | `font-sans`    | All UI text                        |
| Monospace | Geist Mono                       | `font-mono`    | Addresses, keys, hashes, amounts   |

The production app uses `Geist` (sans) and `Geist Mono` from `next/font/google`. The mockup uses Inter as a CDN fallback; the final build should use Geist Sans throughout.

### Type Scale

| Element          | Size Class  | Weight Class   | Effective Size |
| ---------------- | ----------- | -------------- | -------------- |
| Page title       | `text-xl`   | `font-semibold`| 20px / 600     |
| Section heading  | `text-base` | `font-semibold`| 16px / 600     |
| Card heading     | `text-sm`   | `font-semibold`| 14px / 600     |
| Body text        | `text-sm`   | `font-normal`  | 14px / 400     |
| Label            | `text-xs`   | `font-medium`  | 12px / 500     |
| Caption / badge  | `text-[11px]`| `font-medium` | 11px / 500     |
| Micro text       | `text-[10px]`| `font-medium` | 10px / 500     |

### Uppercase Labels

Table headers and section subheadings use uppercase tracking:

```
text-xs font-medium uppercase tracking-wider text-text-muted
```

---

## Spacing Scale

The design follows Tailwind's default 4px base spacing. Key recurring values:

| Context                  | Value   | Tailwind  |
| ------------------------ | ------- | --------- |
| Page horizontal padding  | 24px    | `px-6`    |
| Page vertical padding    | 32px    | `py-8`    |
| Card internal padding    | 24px    | `p-6`     |
| Compact card padding     | 16px    | `p-4`     |
| Section gap              | 24px    | `space-y-6` or `gap-6` |
| Inner element gap        | 12px    | `gap-3`   |
| Small gap                | 8px     | `gap-2`   |
| Tight gap                | 6px     | `gap-1.5` |

### Max Width

The main content area uses `max-w-7xl` (1280px) with `mx-auto` centering.

---

## Component Patterns

### Cards

Base card:
```html
<div class="rounded-xl border border-surface-border bg-surface-raised p-6">
  ...
</div>
```

- Corner radius: `rounded-xl` (12px)
- Border: 1px `border-surface-border`
- Background: `bg-surface-raised`
- Padding: `p-6` for standard, `p-4` for compact

Active / highlighted card (e.g., in-progress round):
```html
<div class="rounded-lg border border-accent-blue/30 bg-accent-blue/5 p-4 glow-blue">
  ...
</div>
```

### Buttons

**Primary:**
```html
<button class="rounded-lg bg-frost-600 px-4 py-2 text-sm font-medium text-white
  transition-colors hover:bg-frost-700
  focus:outline-none focus:ring-2 focus:ring-frost-500 focus:ring-offset-2 focus:ring-offset-surface">
  Action
</button>
```

**Secondary (outline):**
```html
<button class="rounded-full border border-surface-border px-3 py-1 text-xs font-medium text-text-secondary
  transition-colors hover:border-frost-600/50 hover:text-frost-400">
  Select
</button>
```

**Disabled:**
```html
<button disabled class="rounded-lg border border-surface-border bg-surface px-4 py-2.5
  text-sm font-medium text-text-muted cursor-not-allowed">
  Aggregate & Broadcast
</button>
```

### Compact Execute Button (inline in round rows):
```html
<button class="rounded-md bg-frost-600 px-2.5 py-1 text-[10px] font-medium text-white hover:bg-frost-700">
  Execute
</button>
```

### Status Badges

General pattern:
```html
<span class="inline-flex items-center rounded-full bg-{status-bg} px-2 py-0.5 text-[11px] font-medium text-{status-color}">
  Status Label
</span>
```

| Status      | Classes                                                                   |
| ----------- | ------------------------------------------------------------------------- |
| Waiting     | `bg-surface-overlay text-text-muted`                                      |
| In Progress | `bg-accent-blue/10 text-accent-blue badge-pulse`                          |
| Complete    | `bg-accent-green/10 text-accent-green`                                    |
| Error       | `bg-accent-red/10 text-accent-red`                                        |

The `badge-pulse` class applies a subtle 2s opacity animation to in-progress elements:
```css
@keyframes badge-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}
.badge-pulse { animation: badge-pulse 2s ease-in-out infinite; }
```

### Monospace Address Display

Truncated address with copy button:
```html
<div class="flex items-center gap-2">
  <code class="font-mono text-sm text-text-primary">GkXn8bE7...w7bQ2</code>
  <button class="copy-btn rounded-md p-1 text-text-muted transition-colors
    hover:bg-surface-overlay hover:text-text-secondary" title="Copy address">
    <!-- clipboard SVG icon -->
  </button>
</div>
```

Truncation convention: first 8 characters + `...` + last 4 characters.

### Full Key / Address Display (with border):
```html
<div class="flex items-center justify-between rounded-lg border border-surface-border bg-surface px-4 py-3">
  <code class="font-mono text-sm text-text-primary">GkXn8bE7Y4pLqR9mN2jKv5sW6tHd3cFxU1aZ0yBw7bQ2</code>
  <button class="copy-btn ml-3 rounded-md p-1.5 text-text-tertiary ..."><!-- icon --></button>
</div>
```

### Form Inputs

```html
<input type="text" placeholder="Enter value..."
  class="w-full rounded-lg border border-surface-border bg-surface px-3 py-2.5
  font-mono text-sm text-text-primary placeholder-text-muted outline-none
  transition-colors focus:border-frost-600/50 focus:ring-1 focus:ring-frost-600/30">
```

- Background: `bg-surface`
- Border: `border-surface-border`, changes to `border-frost-600/50` on focus
- Focus ring: `focus:ring-1 focus:ring-frost-600/30`
- Monospace font for address inputs; sans font for labels

### Tables

Use CSS Grid for table layout (not `<table>`) for better styling control:

```html
<!-- Header -->
<div class="grid grid-cols-[60px_1fr_140px_140px] border-b border-surface-border px-6 py-3">
  <span class="text-xs font-medium uppercase tracking-wider text-text-muted">Column</span>
  ...
</div>
<!-- Row -->
<div class="grid grid-cols-[60px_1fr_140px_140px] items-center border-b border-surface-border-subtle px-6 py-4
  transition-colors hover:bg-surface-overlay/30">
  ...
</div>
```

Selected row:
```html
<div class="row-selected ...">  <!-- adds blue left border + subtle blue bg -->
```

```css
.row-selected {
  background-color: rgba(59, 130, 246, 0.08);
  border-left: 3px solid #3b82f6;
}
```

### Progress Bar (Segmented)

```html
<div class="flex gap-1.5">
  <div class="h-2 flex-1 rounded-full bg-accent-green"></div>      <!-- complete -->
  <div class="h-2 flex-1 rounded-full bg-accent-blue badge-pulse"></div>  <!-- in progress -->
  <div class="h-2 flex-1 rounded-full bg-surface-overlay"></div>    <!-- waiting -->
</div>
```

Each segment = one step. Total segments = total steps (e.g., 6 for DKG with 2 nodes x 3 rounds).

### Status Timeline (Horizontal Stepper)

Used in signing request detail for lifecycle visualization:

```html
<div class="flex items-center gap-0">
  <!-- Complete step -->
  <div class="flex flex-col items-center">
    <div class="flex h-7 w-7 items-center justify-center rounded-full bg-accent-green/15">
      <!-- checkmark SVG -->
    </div>
    <span class="mt-1.5 text-[10px] text-accent-green">Step Name</span>
  </div>
  <!-- Connector line (complete) -->
  <div class="mx-1 h-0.5 flex-1 bg-accent-green"></div>
  <!-- Active step -->
  <div class="flex flex-col items-center">
    <div class="flex h-7 w-7 items-center justify-center rounded-full bg-accent-blue/20 badge-pulse">
      <div class="h-2.5 w-2.5 rounded-full bg-accent-blue"></div>
    </div>
    <span class="mt-1.5 text-[10px] font-medium text-accent-blue">Step Name</span>
  </div>
  <!-- Connector line (incomplete) -->
  <div class="mx-1 h-0.5 flex-1 bg-surface-border"></div>
  <!-- Waiting step -->
  <div class="flex flex-col items-center">
    <div class="flex h-7 w-7 items-center justify-center rounded-full bg-surface-overlay">
      <div class="h-2 w-2 rounded-full bg-text-muted"></div>
    </div>
    <span class="mt-1.5 text-[10px] text-text-muted">Step Name</span>
  </div>
</div>
```

Connector colors: `bg-accent-green` between complete steps, `bg-surface-border` before incomplete steps. Use gradient `from-accent-green to-accent-blue` on the connector between the last complete and current active step.

### Node Identity Badges

Small icon badge used in panel headers:
```html
<!-- Node A -->
<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-frost-600/15 text-frost-400">
  <span class="text-sm font-semibold">A</span>
</div>
<!-- Node B -->
<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-purple-600/15 text-purple-400">
  <span class="text-sm font-semibold">B</span>
</div>
```

Compact version for signing panels: `h-7 w-7 rounded-md text-xs`.

### Glow Effects

Subtle box-shadow glow for active/success states:
```css
.glow-blue {
  box-shadow: 0 0 20px rgba(59, 130, 246, 0.15), 0 0 4px rgba(59, 130, 246, 0.1);
}
.glow-green {
  box-shadow: 0 0 20px rgba(34, 197, 94, 0.15), 0 0 4px rgba(34, 197, 94, 0.1);
}
```

Use sparingly: only on the currently actionable round and the DKG-complete master key card.

---

## Layout Patterns

### Page Structure

```
Header (sticky, h-14, backdrop-blur)
  Logo + Title (left)
  Tab Navigation (center-right)
  Network Indicator (right)

Main Content (max-w-7xl, px-6, py-8)
  Page heading + description + action button
  Content cards / panels

Footer (h-10, border-t, muted text)
```

### Tab Navigation

Active tab: `border-b-2 border-frost-500 text-text-primary`
Inactive tab: `border-b-2 border-transparent text-text-tertiary hover:text-text-secondary`

Tabs are full-height buttons inside the header bar, so the bottom border aligns with the header border.

### Two-Column Node Layout (DKG, Signing Detail)

```html
<div class="grid grid-cols-2 gap-6">
  <!-- Node A panel -->
  <!-- Node B panel -->
</div>
```

### Split View (Signing Tab)

```html
<div class="grid grid-cols-[340px_1fr] gap-6">
  <!-- Left: fixed-width request list -->
  <!-- Right: flexible detail panel -->
</div>
```

The left list has a fixed 340px width; the detail panel fills remaining space.

---

## Interaction Notes

These are design intentions for the implementation team.

1. **Tab switching** should be instant (no page reload). Use client-side state or URL hash.
2. **Copy buttons** should show a brief "Copied!" tooltip or change the icon to a checkmark for 1.5s.
3. **Execute Round buttons** should show a loading spinner inside the button while the API call runs, then update the round status.
4. **Status badges** with the `badge-pulse` animation should only pulse while the step is actively in progress. Remove the animation class when complete.
5. **Request list** items should be clickable to load their detail in the right panel.
6. **Aggregate & Broadcast** button should be disabled (as shown) until all signing rounds for both nodes are complete.
7. **Explorer link** opens in a new tab. URL pattern: `https://explorer.solana.com/tx/{signature}?cluster=devnet`.
8. **Selected sender** persists across the Wallets and Signing tabs.
9. **Error state** for a round should show a red border, red status badge, and optionally an error message below the round row.
