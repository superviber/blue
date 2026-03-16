# PM Domain Setup Guide

This guide walks through setting up a project management domain with Blue's two-tier organization model (RFC 0070).

## Quick Start

```bash
# In your PM repo (or any code repo)
# Use the Claude Code skill:
/domain-setup
```

The skill will ask about your product and propose a structure.

## Concepts

### Components (Functional Ownership)

Components answer: **"Who owns this work?"**

They map 1:1 to Jira components, giving you:
- Default assignees per component
- Component-level burndown charts
- Admin-controlled categorization

Blue provides 10 standard components. Use the subset that fits your org.

### Areas (Product Surfaces)

Areas answer: **"What part of the product is this?"**

They provide story key prefixes (`CON-001`, `MER-003`) and persist across epics. An area like "Consumer App" spans dozens of epics over the product's lifetime.

Good areas are **durable product surfaces**, not features:
- Consumer App (CON), Merchant Dashboard (MER), Backend API (API)
- Not: Party System (PTY), Auth Flow (AUTH) -- these are epics/features

### How They Compose

```
Epic: TMS-03 "Party System v2"
  CON-015  [Engineering]              "Party creation API v2"
  CON-016  [Engineering]              "Party invite deeplinks"
  CON-017  [Design]                   "Party creation flow redesign"
  CON-018  [Product]                  "Party v2 user stories"
  CON-019  [Engineering, Security]    "Party auth token hardening"
  API-008  [Engineering]              "Rate limiting for party endpoints"
```

## domain.yaml Reference

```yaml
org: the-move-social
key: TMS

jira:
  domain: themovesocial.atlassian.net
  project_key: SCRUM
  drift_policy: warn

components:
  - name: Engineering
    description: "Backend, frontend, infrastructure, DevOps, CI/CD"
  - name: Product
    description: "Feature specs, roadmap, user research, requirements"
  - name: Design
    description: "UI/UX, visual identity, brand assets, design system"

areas:
  - key: CON
    name: Consumer App
    description: "Consumer-facing mobile and web experience"
    components: [Engineering, Product, Design]
    repos: [themove-backend, themove-frontend]
  - key: MER
    name: Merchant App
    description: "Merchant dashboard, analytics, and self-serve tools"
    components: [Engineering, Product, Design, Finance]
    repos: [themove-backend, merchant-frontend]
  - key: API
    name: Backend API
    description: "Core shared services -- auth, notifications, data pipeline"
    components: [Engineering, Security]
    repos: [themove-backend]

repos:
  - name: themove-backend
    url: git@github.com:the-move-social/themove-backend.git
    description: "Backend API services"
  - name: themove-frontend
    url: git@github.com:the-move-social/themove-frontend.git
    description: "React web application"
```

### Field Reference

| Field | Required | Description |
|-------|----------|-------------|
| `org` | Yes | Org name (kebab-case) |
| `key` | Yes | 2-4 uppercase letters, used for epic IDs (TMS-01) |
| `jira.domain` | No | Jira Cloud domain |
| `jira.project_key` | No | Jira project key |
| `jira.drift_policy` | No | `warn`, `block`, or `overwrite` (default: `warn`) |
| `components[].name` | Yes | Component name (matches Jira component) |
| `components[].description` | No | What this component covers |
| `components[].lead` | No | Default assignee for this component |
| `areas[].key` | Yes | 2-4 uppercase letters, story ID prefix |
| `areas[].name` | Yes | Human-readable area name |
| `areas[].description` | No | What this area covers |
| `areas[].components` | No | Typical components (for auto-categorization) |
| `areas[].repos` | No | Code repos this area touches |
| `repos[].name` | Yes | Repo name |
| `repos[].url` | No | Git clone URL |
| `repos[].description` | No | What this repo contains |

## Story Front Matter

```yaml
---
type: story
id: CON-015
title: "Party creation API v2"
area: CON
components: [Engineering]
epic: TMS-03
repo: themove-backend
status: backlog
points: 3
sprint: s01
labels: [api, party]
---
```

## Jira Integration

Components from domain.yaml are synced as Jira components:

```bash
blue jira setup-components    # Create Jira components from domain.yaml
blue sync                     # Sync stories with component + area:KEY label
```

### Filtering in Jira

| View | JQL |
|------|-----|
| All engineering work | `component = Engineering` |
| All consumer app work | `labels = "area:CON"` |
| Engineering on consumer app | `component = Engineering AND labels = "area:CON"` |
