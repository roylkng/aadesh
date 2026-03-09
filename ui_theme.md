
# Adesh UI Theme Specification
## Theme: Signal District
## Version: 1.0
## Status: Primary visual direction
## Intended audience: frontend engineers, design-system engineers, animation engineers, product engineers

Cross-spec linkage:
- Adaptive behavior layering and persona policy boundaries are defined in `adaptive_interface.md`.
- Control-plane API and event semantics remain defined by `control_plane_api_spec.md` and `websocket_events_contract.md`.
- This document defines visual language and token semantics only; it does not authorize behavior changes.

Implementation scope gate:
- v0 requires tokenized theme adoption for the localhost Root Owner shell.
- Theme switching and persona-driven runtime adaptation must remain declarative and reversible.

---

# 1. Theme Intent

Signal District is the primary visual and interaction language for Aadesh Agent OS.

This theme should make the product feel like a live cybernetic control environment where intelligent work moves through a visible execution network. The interface must communicate:

- delegated intelligence
- live orchestration
- inspectable execution
- human override and control
- system memory and traces
- calm depth instead of noisy futurism

The intended mood is:

- urban neon at night
- dusty violet infrastructure
- cyan machine signals
- orange human intervention
- restrained cybernetic atmosphere
- high signal density with calm presentation

This is not a generic AI SaaS theme.
This is not a gaming HUD.
This is not a flashy cyberpunk concept art UI.

It must feel like an operational environment for an Agent OS.

---

# 2. Core Design Principles

## 2.1 Focus before spectacle
All futuristic styling must remain subordinate to usability.
Readability, hierarchy, and inspectability come first.

## 2.2 Signal is sparse
Neon and glow effects must be used as state indicators, not decoration.
Most of the interface should remain dark, matte, and quiet.

## 2.3 The graph is a first-class interaction surface
Execution graphs are not decorative.
They are a core product surface and must support inspection, replay, traversal, zoom, pan, and contextual drill-down.

## 2.4 Human and machine must be visually distinguishable
The user should be able to tell:
- what the system is doing
- what an agent is doing
- where a tool was invoked
- where human approval is required

## 2.5 The product should feel alive but controlled
Motion must show flow, propagation, and activity.
Nothing should pulse or animate just to look futuristic.

## 2.6 Depth comes from layering, not noise
Use panel elevation, subtle gradients, edge highlights, and structured spacing.
Do not rely on heavy glassmorphism, excessive blur, or saturated backgrounds.

---

# 3. Visual Identity Summary

## 3.1 Theme keywords
- signal propagation
- cybernetic calm
- urban neon
- orchestration
- execution topology
- nocturnal infrastructure
- trace visibility
- intelligent control

## 3.2 Mental model
The UI should feel like:
- a city of machine signals
- a mission control room at night
- a graph of intelligence in motion
- a system where actions leave visible traces

## 3.3 What to avoid
Avoid the following:
- bright magenta-heavy cyberpunk
- rainbow gradients
- glossy fintech style cards
- generic AI dashboard styling
- excessive glassmorphism
- over-animated backgrounds
- unreadable low-contrast text
- decorative sci-fi fonts

---

# 4. Color System

## 4.1 Color philosophy
The palette is based on three layers:
- environment
- machine signal
- human intervention

Map them as follows:
- environment: dusty violet, deep purple graphite, muted charcoal
- machine activity: cyan, teal, electric blue
- human intervention: orange, amber
- reasoning and cognitive state: indigo, violet
- failure: restrained crimson
- success: green-cyan, not bright pure green

The overall palette should feel cinematic and urban, not synthetic.

---

## 4.2 Core tokens

### Background tokens
```css
--bg-app: #17131f;
--bg-canvas: #1b1624;
--bg-elevated: #20192c;
--bg-deep: #120f18;
```

### Surface tokens

```css
--surface-1: #241d31;
--surface-2: #2c233b;
--surface-3: #332847;
--surface-4: #3a2e52;
```

### Border tokens

```css
--border-subtle: rgba(205, 196, 255, 0.08);
--border-default: rgba(205, 196, 255, 0.14);
--border-strong: rgba(205, 196, 255, 0.22);
--border-active: rgba(25, 227, 255, 0.42);
```

### Machine signal tokens

```css
--signal-cyan-1: #19e3ff;
--signal-cyan-2: #0cd0ff;
--signal-cyan-3: #6cecff;
--signal-teal-1: #18d7c8;
```

### Cognitive / reasoning tokens

```css
--reason-violet-1: #9b6cff;
--reason-violet-2: #7c4dff;
--reason-indigo-1: #5b6dff;
```

### Human action tokens

```css
--human-orange-1: #ff7a3c;
--human-orange-2: #ff8c42;
--human-amber-1: #ffc168;
```

### Semantic state tokens

```css
--success: #2ed6a1;
--warning: #ffb454;
--danger: #ff5e73;
--info: #56b8ff;
```

### Text tokens

```css
--text-primary: #f3efff;
--text-secondary: #c8c2dc;
--text-muted: #938ca9;
--text-disabled: #676177;
--text-on-accent: #0f1117;
```

### Glow tokens

```css
--glow-cyan-soft: 0 0 0 1px rgba(25,227,255,0.20), 0 0 18px rgba(25,227,255,0.14);
--glow-cyan-medium: 0 0 0 1px rgba(25,227,255,0.32), 0 0 26px rgba(25,227,255,0.22);
--glow-violet-soft: 0 0 0 1px rgba(155,108,255,0.22), 0 0 18px rgba(155,108,255,0.12);
--glow-orange-soft: 0 0 0 1px rgba(255,122,60,0.24), 0 0 18px rgba(255,122,60,0.16);
--glow-danger-soft: 0 0 0 1px rgba(255,94,115,0.24), 0 0 18px rgba(255,94,115,0.16);
```

---

## 4.3 Color usage ratios

Use color with discipline.

Approximate screen distribution:

* 70% dark environment and surface colors
* 18% text and neutral UI chrome
* 7% machine signal cyan
* 3% violet reasoning accents
* 2% warm human intervention accents

Do not invert this ratio.
If accents dominate the screen, the theme fails.

---

## 4.4 Gradients

Gradients must be subtle and rare.

Allowed:

* faint surface gradient from deeper violet to slightly lighter purple-gray
* signal lines that transition cyan to teal
* reasoning ring gradients from indigo to violet

Avoid:

* large rainbow washes
* high saturation background gradients
* gradient text for content-heavy areas

Recommended example:

```css
background: linear-gradient(180deg, #2c233b 0%, #241d31 100%);
```

---

# 5. Typography

## 5.1 Typeface strategy

The palette is expressive, so typography must remain calm and highly legible.

Recommended stack:

* Inter
* Satoshi
* Manrope

Recommended mono stack:

* JetBrains Mono
* IBM Plex Mono

Preferred implementation:

* primary UI font: Inter
* mono font: JetBrains Mono

---

## 5.2 Type scale

```css
--font-display-xl: 32px;
--font-display-lg: 28px;
--font-h1: 24px;
--font-h2: 20px;
--font-h3: 16px;
--font-body: 14px;
--font-body-sm: 13px;
--font-caption: 12px;
--font-micro: 11px;
```

---

## 5.3 Font weights

```css
--weight-regular: 400;
--weight-medium: 500;
--weight-semibold: 600;
--weight-bold: 700;
```

Use:

* page titles: 600
* section labels: 500
* metadata and helper labels: 500 or 400
* body text: 400
* counts and state labels: 600

---

## 5.4 Typography rules

* Never use futuristic decorative fonts for body content.
* Avoid extremely thin weights.
* Use tighter tracking only for labels or small uppercase metadata.
* Use mono only for logs, identifiers, metrics, and run steps.

---

## 5.5 Label style

Uppercase micro-labels may be used for:

* state groups
* trace sections
* metadata headings
* graph legends

Example:

* EXECUTION STATE
* RECENT RUNS
* ACTIVE TOOLS

Use:

* 11px to 12px
* medium weight
* letter spacing 0.08em to 0.12em
* muted text color

---

# 6. Spacing and Radius System

## 6.1 Base spacing scale

```css
--space-2: 2px;
--space-4: 4px;
--space-6: 6px;
--space-8: 8px;
--space-10: 10px;
--space-12: 12px;
--space-16: 16px;
--space-20: 20px;
--space-24: 24px;
--space-32: 32px;
--space-40: 40px;
```

Preferred primary rhythm:

* internal control spacing: 8px, 12px
* card padding: 16px, 20px
* section spacing: 24px, 32px

---

## 6.2 Border radius

Rounded, but not soft consumer app rounded.

```css
--radius-xs: 6px;
--radius-sm: 8px;
--radius-md: 12px;
--radius-lg: 16px;
--radius-xl: 20px;
--radius-pill: 999px;
```

Usage:

* buttons: 10px to 12px
* cards: 14px to 16px
* graph nodes: 12px to 16px
* pills and status chips: pill radius

Avoid:

* completely square
* bubble-round consumer aesthetics

---

# 7. Elevation and Surface Model

## 7.1 Surface philosophy

Most surfaces should feel matte, layered, and low-gloss.

Use elevation through:

* subtle shadow
* subtle inner highlight
* border opacity change
* local glow for active state

Do not use heavy drop shadows everywhere.

---

## 7.2 Surface recipes

### Base panel

```css
background: linear-gradient(180deg, #2c233b 0%, #241d31 100%);
border: 1px solid rgba(205, 196, 255, 0.10);
box-shadow: inset 0 1px 0 rgba(255,255,255,0.03);
```

### Elevated panel

```css
background: linear-gradient(180deg, #332847 0%, #2c233b 100%);
border: 1px solid rgba(205, 196, 255, 0.14);
box-shadow: 0 8px 30px rgba(0,0,0,0.28), inset 0 1px 0 rgba(255,255,255,0.04);
```

### Active panel

```css
border: 1px solid rgba(25,227,255,0.28);
box-shadow: 0 0 0 1px rgba(25,227,255,0.14), 0 0 24px rgba(25,227,255,0.10);
```

---

# 8. Layout Model

## 8.1 Primary shell layout

The product shell should support three persistent zones:

### Left rail

Purpose:

* primary navigation
* context switching
* quick system state
* account / workspace switch

Width:

* collapsed: 72px
* expanded: 240px to 280px

Items:

* Home
* Missions
* Runs
* Agents
* Memory
* Tools
* Logs
* Settings

---

### Center workspace

Purpose:

* main working surface
* graph, mission board, run explorer, memory view

This zone gets priority and should hold:

* page title
* mode controls
* graph canvas or object view
* active mission content

---

### Right context panel

Purpose:

* node inspection
* trace details
* tool output
* artifact preview
* agent state
* approval actions

Width:

* 320px to 420px

Should be collapsible.

---

## 8.2 Top bar

The top bar should be calm and compact.
It should contain:

* workspace title or current object path
* global search
* quick command trigger
* notification / system status
* user profile

Search field should feel more like a command line than a generic search box.

---

# 9. Core Interaction Surfaces

## 9.1 Missions Overview

This is not a dashboard in the traditional sense.
It should show active intelligent work in motion.

Must include:

* active missions
* current state
* agent participation
* graph preview or trace sparkline
* intervention required items
* recent artifacts
* live system signal

---

## 9.2 Run Explorer

This is a primary surface.
It should visualize:

* run topology
* step graph
* traversal order
* node states
* tool calls
* outputs
* failures
* human checkpoints

This surface must allow:

* zoom
* pan
* node click
* trace replay
* filter by agent / tool / state
* timeline scrub

---

## 9.3 Agent Profile

Should feel like inspecting an operative entity.
Must include:

* role / mandate
* capabilities
* tool permissions
* recent runs
* performance
* error modes
* reliability trend
* associated artifacts

---

## 9.4 Tool Registry

Should feel like a capability network, not a settings page.
Must include:

* tool cards or capability list
* supported input types
* permissions and boundaries
* recent usage
* compatibility with agents
* failure rates and latency where applicable

---

## 9.5 Memory Explorer

Should feel like a living archive of work.
Must include:

* memory objects
* linked runs
* associated artifacts
* retrieval traces
* timestamps
* semantic search
* relationships between memory and mission objects

---

# 10. Graph System Specification

## 10.1 Role of graph

The graph is the heart of Signal District.
It is the main product differentiator and must be treated as core UX, not an embellishment.

The graph represents:

* delegation paths
* execution order
* tool invocation
* information movement
* result formation
* decision checkpoints

---

## 10.2 Node types

At minimum support these node types:

* Goal
* Planner
* Agent
* Tool
* Memory Recall
* Verification
* Approval Checkpoint
* Artifact
* Failure / Blocked State

Each node type should have:

* unique icon
* label
* state indicator
* optional metadata
* click / inspect behavior

---

## 10.3 Node states

Every node must visually support:

### Idle

* muted surface
* no glow
* subtle border

### Running

* cyan edge glow
* active pulse
* light animation at node perimeter

### Reasoning

* violet glow
* softer than running
* optional ring or aura effect

### Waiting for human

* orange border
* stable amber/orange light
* no pulse spam

### Completed

* settled cyan or teal outline
* minimal residual glow

### Failed

* crimson border
* subdued red glow
* clear iconography

### Disabled / skipped

* low-opacity surface
* muted label
* no activity

---

## 10.4 Edge styling

Edges should be readable and quiet when inactive.

Inactive edge:

```css
stroke: rgba(180, 172, 210, 0.16);
stroke-width: 1.5;
```

Active edge:

```css
stroke: rgba(25,227,255,0.65);
stroke-width: 2;
filter: drop-shadow(0 0 6px rgba(25,227,255,0.30));
```

Reasoning edge:

```css
stroke: rgba(155,108,255,0.55);
```

Human checkpoint edge:

```css
stroke: rgba(255,122,60,0.52);
```

Avoid thick neon tubes.
Keep edges elegant.

---

## 10.5 Graph layout behavior

Preferred layout should support:

* directed top-down execution
* branching flows
* optional left-to-right trace mode
* grouped subgraphs
* collapsible clusters

Good candidates:

* ELK layout for structured DAGs
* React Flow custom layout for interactive UX

---

## 10.6 Traversal animation

Traversal animation is required.

### Purpose

To show how a mission or run moved through the system.

### Visual behavior

* a signal packet travels along an edge
* the traversed portion of the edge lights up
* the destination node activates on arrival
* child edges may branch after activation
* traversal settles after completion

### Speed

Recommended default:

* 500ms to 900ms per edge segment
* staggered by dependency and graph depth

### Style

Preferred:

* glowing dot or short elongated pulse
* tail fade behind the dot
* slight bloom on arrival

Avoid:

* cartoonish rocket motion
* bouncing travel
* overly large moving particles

---

## 10.7 Replay mode

Replay mode is strongly recommended.

Behavior:

* user selects a completed run
* graph replays execution order
* timeline scrub lets user move through states
* logs and node metadata update with scrubber position

This feature will materially differentiate the product.

---

# 11. Motion System

## 11.1 Motion principles

Motion must:

* reveal state
* indicate causality
* support orientation
* reinforce signal propagation

Motion must not:

* distract
* pulse constantly
* animate large backgrounds unnecessarily

---

## 11.2 Motion timings

Suggested durations:

* hover transitions: 120ms to 180ms
* press transitions: 80ms to 120ms
* panel expand / collapse: 180ms to 240ms
* modal / side panel slide: 220ms to 280ms
* traversal edge animation: 500ms to 900ms
* graph node activation: 180ms to 240ms

Easing:

```css
cubic-bezier(0.22, 1, 0.36, 1)
```

---

## 11.3 Motion patterns

### Hover

* border brightens slightly
* text clarity improves
* optional local shadow increase

### Press

* slight darkening or scale 0.98
* glow compresses briefly

### Active state

* narrow edge glow or underline
* not full-card pulsation

### Panel entry

* low-distance upward fade
* or lateral slide from side context rail

### Node activation

* quick bloom
* edge pulse arrives
* steady state remains

---

# 12. Components

## 12.1 Buttons

### Button sizes

* small: 30px to 32px height
* medium: 36px to 40px height
* large: 44px to 48px height

### Primary button

Use for:

* Run Mission
* Execute
* Create Mission
* Start Replay

Style:

* dark filled cyan-tinted background
* subtle cyan glow on hover
* high contrast label

Example:

```css
background: linear-gradient(180deg, rgba(25,227,255,0.22), rgba(25,227,255,0.12));
border: 1px solid rgba(25,227,255,0.32);
color: var(--text-primary);
box-shadow: var(--glow-cyan-soft);
```

### Secondary button

Use for standard non-destructive actions.

Style:

* surface background
* subtle border
* no large glow

### Ghost button

Use inside dense panels or toolbars.
Style:

* transparent background
* only hover surface tint

### Human action button

Use for:

* Approve
* Escalate
* Manual Override

Style:

* orange tint
* warm glow on hover

### Danger button

Use for:

* Stop Run
* Revoke
* Delete

Style:

* muted red tone
* restrained danger glow

---

## 12.2 Inputs

### Input philosophy

Inputs should feel like command surfaces, not retail form fields.

### Input style

```css
background: rgba(18,15,24,0.88);
border: 1px solid rgba(205,196,255,0.12);
color: var(--text-primary);
border-radius: 12px;
```

Focus state:

```css
border-color: rgba(25,227,255,0.38);
box-shadow: 0 0 0 3px rgba(25,227,255,0.10);
```

Use cases:

* global search
* command entry
* filter panel
* mission creation

Avoid:

* bright white input fields
* thick heavy outlines

---

## 12.3 Chips and tags

Use for:

* agent type
* run state
* tool category
* execution status
* memory source

Style:

* small rounded pill
* muted base
* accent color only if semantically meaningful

Examples:

* Running: cyan tint
* Reasoning: violet tint
* Approval Needed: orange tint
* Failed: red tint

---

## 12.4 Cards

Cards are operational modules, not marketing tiles.

Card anatomy:

* title
* status
* summary
* optional chart / sparkline / graph preview
* footer actions or metadata

Cards should allow:

* hover inspectability
* context menu
* state-colored accent if active

---

## 12.5 Tables / lists

Even if graph is primary, structured lists are still important.

Style:

* dark surfaces
* subtle row separators
* hover tint with low opacity
* no harsh zebra striping

Use tables for:

* logs
* artifacts
* recent runs
* memory entries
* tool registry

---

## 12.6 Tabs

Use understated tabs.

Preferred styles:

* underline accent
* soft active border
* compact pill only in specific contexts

Avoid:

* oversized segmented controls everywhere

---

## 12.7 Side panels

The right panel is critical.

Style:

* elevated dark surface
* subtle border-left highlight
* content grouped into stacked sections
* should feel like an inspection console

---

# 13. Icons and Symbol Language

## 13.1 Icon style

Use:

* thin to medium stroke
* geometric consistency
* modern technical aesthetic

Avoid:

* playful rounded icons
* cartoon icons
* over-detailed skeuomorphic icons

---

## 13.2 Semantic icon mapping

Use distinct icons for:

* planner
* worker agent
* memory
* tool
* artifact
* verification
* approval
* failure
* trace log

These should remain visually coherent across graph nodes, cards, and menus.

---

# 14. Data Visualization

## 14.1 Chart philosophy

Charts should be compact and quiet.
Use them as support for operational understanding, not decorative analytics.

Good uses:

* activity sparkline
* latency trend
* success rate
* agent run count
* memory load
* tool usage

---

## 14.2 Chart styling

* line color: cyan or violet depending on semantic meaning
* grid lines: extremely subtle
* axis labels: muted
* no bright chart fills unless needed
* avoid heavy area chart opacity

---

## 14.3 Trace mini-previews

Small graph or line previews in cards are encouraged.
This reinforces the theme identity.

Use on:

* mission cards
* recent runs
* agent activity cards

---

# 15. Background and Atmospheric Texture

## 15.1 Background rule

Background should not be flat black.
Use a dusty purple-black environment with extremely subtle depth.

Recommended methods:

* radial dark overlay
* low-opacity noise texture
* faint grid lines
* occasional ambient bloom near graph zones

Opacity should remain low.

---

## 15.2 Allowed atmospheric effects

Allowed:

* very faint grid
* slight vignette
* localized cyan bloom around active graph regions
* subtle grain texture

Avoid:

* moving particles
* rain effects
* large animated fog layers
* excessive scanlines

---

# 16. Accessibility and Readability

## 16.1 Contrast

All body text must meet readable contrast against surfaces.
Do not sacrifice readability for aesthetic darkness.

## 16.2 Color as secondary cue

Never rely on color alone for state.
Combine with:

* icon
* label
* pulse style
* border shape or state marker

## 16.3 Motion reduction

Support reduced motion mode.
When enabled:

* traversal should become simpler or fade-based
* pulsing should be minimized
* graph replay can use instant state jumps or subtle linear highlights

---

# 17. Implementation Guidance

## 17.1 Engineering recommendation

Build the theme as design tokens plus semantic component recipes.

Recommended architecture:

* CSS custom properties or Tailwind theme extension
* semantic tokens mapped to components
* motion tokens separate from component tokens
* graph node and edge token definitions isolated in graph module

---

## 17.2 Suggested token groups

Organize tokens by:

* color
* text
* surface
* spacing
* radius
* shadow
* glow
* motion
* graph-node
* graph-edge
* semantic-state

---

## 17.3 State mapping model

Support a normalized UI state model across components:

* idle
* hover
* active
* selected
* running
* reasoning
* waiting_human
* success
* warning
* failed
* disabled

This ensures consistent styling across:

* cards
* nodes
* chips
* buttons
* list items

---

# 18. Page Templates

## 18.1 Mission Control page

Must include:

* active missions
* graph or graph preview
* intervention queue
* recent artifacts
* agent status overview

Graph should be above the fold.

---

## 18.2 Run Explorer page

Must include:

* large central graph
* execution timeline or replay scrub
* right-side node inspection
* bottom or side trace log
* state filters

This is the flagship page.

---

## 18.3 Agent Profile page

Must include:

* agent role card
* recent runs
* tool capability section
* behavioral metrics
* current state
* related memory and artifacts

---

## 18.4 Memory Explorer page

Must include:

* search-first header
* memory entities
* graph or relational listing
* source linkage
* trace to originating runs

---

## 18.5 Tool Registry page

Must include:

* grouped capability cards
* permissions
* health / usage status
* supported agents
* recent invocation metrics

---

# 19. Example Semantic Mapping

## 19.1 Color semantics

* cyan = execution, live machine activity
* violet = reasoning, planning, model cognition
* orange = human intervention, review, approval
* teal-green = success and healthy live status
* red = failure, interruption, blocked state

## 19.2 Layout semantics

* center = execution truth
* left = navigation and system mode
* right = object inspection and intervention

## 19.3 Motion semantics

* edge traversal = work progressing
* node bloom = state transition
* panel slide = context shift
* sparkline or graph update = live telemetry

---

# 20. Non-Negotiable Rules

1. The graph must be implemented as a primary interaction surface, not as a decorative widget.
2. Neon colors must be used as signal accents only.
3. Text legibility must remain high across all primary surfaces.
4. Background must use dusty violet-black tones, not plain black.
5. Human-required steps must be clearly distinguishable from machine execution.
6. Traversal animation must communicate causality, not just motion.
7. Cards and panels must feel operational and serious, not consumer-app friendly.
8. Typography must remain calm and modern, never gimmicky.
9. Glow effects must remain localized and restrained.
10. The interface must feel alive but never visually noisy.

---

# 21. Optional Future Enhancements

These are not required for first implementation but are aligned with the theme.

## 21.1 Signal Replay mode

Replay a run with animated graph traversal and synchronized logs.

## 21.2 Activity heat layer

Heat map showing most active nodes or tools over time.

## 21.3 District Map mode

Alternative graph mode where capability clusters appear like city districts.

## 21.4 Temporal trace lens

Slide through time and see graph state at each run step.

## 21.5 Memory pulse overlay

Show memory retrieval paths as distinct violet signal flows.

---

# 22. Implementation Priorities

## Phase 1

* core shell layout
* color token system
* typography
* cards, buttons, inputs
* mission overview page
* right inspection panel

## Phase 2

* graph node library
* graph edge styling
* traversal animation
* run explorer page
* state chips and semantic badges

## Phase 3

* replay mode
* memory explorer
* richer telemetry visualizations
* animated tool invocation traces
* reduced motion support polishing

---

# 23. Final Outcome Standard

A successful implementation should make users feel:

* this system has depth
* I can see what the agents are doing
* this is not just chat
* I can inspect and control execution
* the product feels futuristic without being exhausting
* the graph is the operating surface of the system

If the result looks like a generic dark dashboard with cyan buttons, the theme has not been implemented correctly.

Signal District succeeds only if the interface visibly expresses:

* topology
* execution
* traces
* agency
* control
* memory
