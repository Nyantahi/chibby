# Chibby Design System

A simple, opinionated design guide for building consistent UI across Chibby.
Follow this guide when creating new components or modifying existing ones.

---

## 1. Color Palette

Chibby uses a **dark-first** palette with teal as the primary accent and amber for warmth.
All colors are defined as CSS custom properties in `:root`.

### Dark Theme (default)

| Token                    | Hex         | Usage                              |
| ------------------------ | ----------- | ---------------------------------- |
| `--color-bg`             | `#0d1117`   | Page background                    |
| `--color-surface`        | `#141a22`   | Cards, sidebar, panels             |
| `--color-surface-hover`  | `#1b2330`   | Hover state on surfaces            |
| `--color-surface-active` | `#222d3b`   | Active/pressed state on surfaces   |
| `--color-border`         | `#273040`   | Default borders                    |
| `--color-border-light`   | `#344052`   | Subtle dividers                    |
| `--color-text`           | `#e6edf3`   | Primary text                       |
| `--color-text-muted`     | `#8b949e`   | Secondary/helper text              |
| `--color-text-dim`       | `#545d68`   | Disabled or placeholder text       |

### Accent Colors

| Token                     | Hex                        | Usage                            |
| ------------------------- | -------------------------- | -------------------------------- |
| `--color-primary`         | `#2dd4a8`                  | Primary actions, links, active   |
| `--color-primary-hover`   | `#22b892`                  | Hover on primary elements        |
| `--color-primary-bg`      | `rgba(45, 212, 168, 0.10)` | Tinted backgrounds for active    |
| `--color-accent`          | `#f0a850`                  | Secondary accent (badges, icons) |
| `--color-accent-hover`    | `#d99540`                  | Hover on accent elements         |
| `--color-accent-bg`       | `rgba(240, 168, 80, 0.10)` | Tinted backgrounds for accent    |

### Semantic Colors

| Token                  | Hex                        | Usage                 |
| ---------------------- | -------------------------- | --------------------- |
| `--color-success`      | `#2dd4a8`                  | Passed, deployed, ok  |
| `--color-success-bg`   | `rgba(45, 212, 168, 0.12)` | Success tint          |
| `--color-failed`       | `#f47067`                  | Failed, error         |
| `--color-failed-bg`    | `rgba(244, 112, 103, 0.12)`| Error tint            |
| `--color-running`      | `#f0a850`                  | In-progress, building |
| `--color-running-bg`   | `rgba(240, 168, 80, 0.12)` | Running tint          |
| `--color-pending`      | `#545d68`                  | Queued, waiting       |
| `--color-pending-bg`   | `rgba(84, 93, 104, 0.12)`  | Pending tint          |
| `--color-skipped`      | `#545d68`                  | Skipped steps         |
| `--color-cancelled`    | `#545d68`                  | Cancelled runs        |

> **Rule:** Never hard-code hex values in components. Always use the `var(--token)` form.

---

## 2. Typography

| Token              | Size         | px   | Use for                            |
| ------------------ | ------------ | ---- | ---------------------------------- |
| `--font-size-2xs`  | `0.75rem`    | 12   | Fine print, timestamps             |
| `--font-size-xs`   | `0.8125rem`  | 13   | Badges, labels, metadata           |
| `--font-size-sm`   | `0.875rem`   | 14   | Nav items, table cells, captions   |
| `--font-size-base` | `1rem`       | 16   | Body text (default)                |
| `--font-size-md`   | `1.125rem`   | 18   | Subheadings (h5, h6)              |
| `--font-size-lg`   | `1.375rem`   | 22   | Section titles (h3, h4)           |
| `--font-size-xl`   | `1.75rem`    | 28   | Page titles (h2)                  |
| `--font-size-2xl`  | `2.125rem`   | 34   | Hero / top-level heading (h1)     |

**Font stacks:**

- **Sans:** `-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif`
- **Mono:** `'SF Mono', 'Fira Code', 'Roboto Mono', monospace`

**Rules:**

- Body text: `font-weight: 400`
- Labels and nav: `font-weight: 500`
- Headings: `font-weight: 600` or `700`
- Never go above `700`. Never use italic for UI text.
- Line-height: `1.5` for body, `1.3` for headings.

---

## 3. Spacing

Use the spacing scale for all margins, paddings, and gaps.

| Token         | Value      |
| ------------- | ---------- |
| `--space-xs`  | `0.25rem`  |
| `--space-sm`  | `0.5rem`   |
| `--space-md`  | `0.75rem`  |
| `--space-lg`  | `1rem`     |
| `--space-xl`  | `1.5rem`   |
| `--space-2xl` | `2rem`     |

**Rules:**

- Use `gap` for flex/grid layouts, not margin hacks.
- Sidebar padding: `--space-lg`
- Card internal padding: `--space-lg`
- Page content padding: `--space-xl` vertical, `--space-2xl` horizontal
- Minimum touch target: `32px` height

---

## 4. Border Radius

| Token          | Value  | Use for                    |
| -------------- | ------ | -------------------------- |
| `--radius-sm`  | `4px`  | Badges, small chips        |
| `--radius-md`  | `6px`  | Buttons, inputs, cards     |
| `--radius-lg`  | `8px`  | Modals, dialogs, dropdowns |

**Rule:** Don't use `border-radius: 50%` except for avatars or status dots.

---

## 5. Components

### Buttons

```text
Primary:    bg: --color-primary       text: #0d1117    radius: --radius-md
Hover:      bg: --color-primary-hover
Secondary:  bg: transparent           text: --color-text-muted  border: --color-border
Hover:      bg: --color-surface-hover text: --color-text
Danger:     bg: transparent           text: --color-failed      border: --color-failed
Hover:      bg: --color-failed-bg
```

- Height: `32px` (default), `28px` (small), `36px` (large)
- Padding: `0 --space-lg`
- Font: `--font-size-sm`, weight `500`
- Always `cursor: pointer`. Disabled: `opacity: 0.5`, `pointer-events: none`

### Cards

```text
bg:     --color-surface
border: 1px solid --color-border
radius: --radius-md
pad:    --space-lg
```

- Hover (if clickable): `border-color: --color-border-light`
- No box-shadow. Keep it flat.

### Inputs

```text
bg:          --color-bg
border:      1px solid --color-border
radius:      --radius-md
pad:         --space-sm --space-md
text:        --color-text
placeholder: --color-text-dim
```

- Focus: `border-color: --color-primary`, `outline: none`, add `box-shadow: 0 0 0 2px var(--color-primary-bg)`
- Height: `32px`

### Status Badges

```text
.status-success  { color: --color-success;  bg: --color-success-bg; }
.status-failed   { color: --color-failed;   bg: --color-failed-bg;  }
.status-running  { color: --color-running;  bg: --color-running-bg; }
.status-pending  { color: --color-pending;  bg: --color-pending-bg; }
```

- Font: `--font-size-xs`, weight `600`, uppercase
- Padding: `2px 8px`, radius: `--radius-sm`

### Sidebar Navigation

- Active link: `bg: --color-primary-bg`, `color: --color-primary`
- Hover: `bg: --color-surface-hover`, `color: --color-text`
- Default: `color: --color-text-muted`
- Icon size: `16px`, gap: `--space-sm`

---

## 6. Iconography

- **Library:** [Lucide React](https://lucide.dev/)
- **Default size:** `16px` for inline/nav, `20px` for page headers, `24px` for empty states
- **Color:** Inherit from parent text color
- **Rule:** Don't mix icon libraries. Stick to Lucide.

---

## 7. Motion

- **Duration:** `150ms` for micro-interactions (hover, focus), `250ms` for panels/modals
- **Easing:** `ease` for most, `ease-out` for entrances, `ease-in` for exits
- **Rule:** No animation on first paint. No bouncing. Keep it subtle.

---

## 8. Layout Patterns

### App Shell

```text
+--sidebar(220px)--+-------main-content-------+
|  logo            |                           |
|  nav links       |   .page (max 960px)       |
|                  |                           |
|  settings        |                           |
+------------------+---------------------------+
```

### Page with Side Panel

```text
.page-with-sidebar {
  display: flex;
  gap: --space-xl;
  max-width: 1200px;
}
```

---

## 9. Do / Don't

| Do                                           | Don't                                      |
| -------------------------------------------- | ------------------------------------------ |
| Use CSS variables for all colors              | Hard-code hex values in components         |
| Use the spacing scale                         | Use arbitrary pixel values                 |
| Keep surfaces flat (no shadows)               | Add box-shadow to cards                    |
| Use `--color-text-muted` for secondary info   | Use opacity to dim text                    |
| One primary action per view                   | Multiple teal buttons competing            |
| Use amber accent for non-critical highlights  | Use red/failed color for non-error states  |
| Test contrast on dark background              | Assume colors are readable                 |

---

## 10. Applying This Guide

When the design system is applied, update the `:root` block in `frontend/styles/index.css` with the tokens from sections 1-4. All existing components that reference `var(--color-*)` tokens will pick up the new palette automatically.

**Checklist for new components:**

1. Uses only design tokens (no raw hex/rgb)
2. Follows spacing scale
3. Uses correct font size token for context
4. Has hover/focus/disabled states
5. Works at sidebar width (220px) if applicable
