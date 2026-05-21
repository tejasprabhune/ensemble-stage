# Web layer

This directory contains all frontend assets for the Stage web application.

## Structure

```
web/
  static/
    fonts/
      Inter-Variable.woff2   self-hosted Inter variable font (Latin subset)
    styles/
      tokens.css             CSS custom properties: colors, spacing, typography
      base.css               global element styles and utility classes
    style.css                component-level styles (topbar, pages, viewer)
    app.js                   keyboard navigation and shortcuts overlay
    viewer.js                trace viewer (timeline and chat views)
  templates/
    base.html                public shell (light theme, 60px topbar)
    base-app.html            authenticated shell (dark theme, 40px topbar, left rail)
    landing.html             homepage
    project.html             run list for a project
    run_detail.html          individual run with trace viewer and metadata sidebar
    account.html             API key management
    sweep.html               sweep detail (stub)
    training_run.html        training run detail (stub)
```

## Design system

### Theme

Two themes are defined: `light` for signed-out and marketing surfaces, `dark` for the authenticated app shell. The theme is set server-side via the `data-theme` attribute on `<html>`. There is no client-side toggle.

Templates that extend `base.html` use the light theme. Templates that extend `base-app.html` use the dark theme. Both base templates link the same three CSS files, so all tokens and base styles are available regardless of theme.

### Color tokens

Defined in `styles/tokens.css`. Dark theme values are the `:root` defaults; light theme overrides them via `[data-theme="light"]`.

| Token | Purpose |
|---|---|
| `--color-bg` | Page background |
| `--color-bg-elevated` | Cards, inputs, elevated surfaces |
| `--color-fg` | Primary text |
| `--color-fg-muted` | Secondary text, labels, metadata |
| `--color-fg-subtle` | Tertiary text, separators, decorative |
| `--color-border` | Default 1px borders |
| `--color-border-strong` | Focused or emphasized borders |
| `--color-accent` | Primary interactive color (buttons, active states, links) |
| `--color-accent-fg` | Text on `--color-accent` backgrounds |
| `--color-link` | Link color (same as accent in both themes) |
| `--color-link-hover` | Link hover color |
| `--color-success` | Completed status |
| `--color-warning` | Running/in-progress status |
| `--color-error` | Failed status |
| `--color-code-bg` | Background for `<code>` and `<pre>` elements |

### Spacing

An 8px grid. The half-step (`--space-1: 4px`) is for tight internal padding.

| Token | Value |
|---|---|
| `--space-1` | 4px |
| `--space-2` | 8px |
| `--space-3` | 16px |
| `--space-4` | 24px |
| `--space-5` | 32px |
| `--space-6` | 40px |
| `--space-7` | 48px |
| `--space-8` | 64px |

### Typography

Body type is Inter (self-hosted variable font, Latin subset). Calling Code is kept for monospace use cases via Typekit. The variable font covers weights 100–900.

| Token | Value | Use |
|---|---|---|
| `--font-sans` | Inter, system-ui | All body and heading text |
| `--font-mono` | calling-code | Code, run IDs, numeric columns, tabular data |
| `--text-xs` | 12px | Labels, uppercase section headers |
| `--text-sm` | 13px | Table content, metadata, code |
| `--text-base` | 14px | Body text, form controls |
| `--text-md` | 16px | Slightly larger body, subheadings |
| `--text-lg` | 18px | (reserved) |
| `--text-xl` | 22px | Section headings (h2) |
| `--text-2xl` | 28px | (reserved) |
| `--text-3xl` | 32px | Page title (h1) |

Headings use `--font-sans` at regular or medium weight. Not bold by default. The hierarchy steps down modestly from h1 (32px) to body (14px).

Use `--font-mono` only where monospace genuinely helps: run IDs, timestamps in tables, cost figures, code blocks, and the diff table. Do not use it for general body text or button labels.

### Layout constants

| Token | Value | Use |
|---|---|---|
| `--topbar-h-public` | 60px | Public topbar height |
| `--topbar-h-app` | 40px | App shell topbar height |
| `--rail-w` | 200px | Left navigation rail width |
| `--sidebar-w` | 320px | Run detail right sidebar width |

### Visual rules

**No rounded corners.** Do not set `border-radius` on any element. The only exception is `.avatar` (user avatar circles), where `border-radius: 50%` is functional.

**No shadows.** Do not use `box-shadow` anywhere. Elevation is expressed through borders and background color differences, not blur or drop shadows.

**Flat surfaces.** Boxes share borders or have tight gaps. The grid is the primary visual element.

**Buttons.** All buttons are rectangles with a 1px border. Primary buttons fill with `--color-accent` and use `--color-accent-fg` text. Secondary buttons are transparent with `--color-border`. Use the `.btn` class for the base style and `.btn-primary` for the filled variant. `.btn-sm` reduces padding for topbar contexts.

**Tables.** HTML tables with 1px borders (`border-collapse: collapse`). No zebra striping. Use `--text-sm` for content and `--font-mono` for numeric or ID columns.

**Status indicators.** Use `.status-dot` + `.status-dot-{running,completed,failed,cancelled,queued}` for status squares in tables. These are 8px filled squares (not circles). The running state pulses.

**Focus rings.** 2px solid `--color-accent`, offset 2px. Applied globally via `:focus-visible` in `base.css`.

### App shell template blocks

`base-app.html` exposes the following blocks for authenticated pages to override:

| Block | Purpose |
|---|---|
| `title` | `<title>` content |
| `topnav` | Breadcrumb in the topbar left area |
| `topbar_center` | Center area of the topbar (filter bar on project pages) |
| `rail_project` | Current project name in the left rail |
| `rail_nav` | Navigation links list in the left rail |
| `content` | Main page content |

Pages without project context (such as `account.html`) leave `rail_project` and `rail_nav` empty.

### Adding new pages

Authenticated pages should extend `base-app.html` and override the blocks above. Public or marketing pages should extend `base.html`.

Reference `--color-*` tokens for all colors. Reference `--space-*` for padding and margin. Use `--font-mono` only for code, IDs, and numeric data.
