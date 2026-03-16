---
name: domain-setup
description: Interactive domain setup — propose components, areas, and domain.yaml from a product description
---

# Domain Setup

You are helping set up a project management domain configuration (`domain.yaml`) for a Blue-managed org.

## What You Do

Given a product description, you propose a **two-tier organization structure**:

1. **Components** (functional ownership -> Jira components): Who owns this work?
2. **Areas** (product surfaces -> story key prefixes): What part of the product is this?

## Standard Components

These are the standard 10 components. Propose only the ones relevant to the org's size and needs:

| Component | Covers |
|-----------|--------|
| Engineering | Backend, frontend, infrastructure, DevOps, CI/CD |
| Product | Feature specs, roadmap, user research, requirements |
| Design | UI/UX, visual identity, brand assets, design system |
| Business | Strategy, partnerships, monetization, go-to-market, sales |
| Finance | Budgets, runway, pricing models, projections, accounting |
| Operations | Processes, workflows, documentation, team coordination |
| Legal | Contracts, compliance, privacy, IP, regulatory |
| Growth | Acquisition, retention, A/B testing, content, marketing |
| People | Hiring, culture, onboarding, contractors, performance |
| Security | App security, infra security, audits, pen testing |

**Solo founders** typically need: Engineering, Product, Design, and maybe 1-2 others.
**Small teams (2-5)** typically need: 5-7 components.
**Larger teams** may use all 10.

## Area Guidelines

Areas represent **product surfaces, services, or workstreams** -- not features or epics.

**Good areas:**
- CON (Consumer App) -- a product surface
- MER (Merchant Dashboard) -- a distinct user-facing product
- API (Backend API) -- shared services layer
- INF (Infrastructure) -- CI/CD, cloud, monitoring

**Bad areas:**
- PTY (Party System) -- too specific, this is a feature/epic
- AUTH (Authentication) -- too narrow, this is a cross-cutting concern
- V2 (Version 2) -- temporal, not structural

**Key constraints:**
- 2-4 uppercase letters
- Unique within the org
- Short, memorable, unambiguous
- Durable -- should outlive any single epic

## Flow

1. **Ask** the user to describe their product: what they're building, who uses it, what distinct surfaces/apps exist, what code repos they have.

2. **Propose** a domain.yaml structure:
   - Org name and key (3 uppercase letters from org name)
   - Relevant components (subset of standard 10)
   - Areas with keys, names, descriptions
   - Area-to-component and area-to-repo mappings
   - Jira config if they use Jira

3. **Iterate** -- let the user adjust: add/remove components, split/merge areas, change keys.

4. **Write** the final `domain.yaml` to the PM repo root.

5. **Optionally** run `blue jira setup-components` if Jira is configured.

## domain.yaml Format

```yaml
org: {org-name}
key: {ORG}

jira:
  domain: {org}.atlassian.net
  project_key: {PROJECT}
  drift_policy: warn

components:
  - name: Engineering
    description: "Backend, frontend, infrastructure, DevOps, CI/CD"
  # ... only relevant components

areas:
  - key: {KEY}
    name: {Area Name}
    description: "{what this area covers}"
    components: [{typical components for this area}]
    repos: [{code repos this area touches}]

repos:
  - name: {repo-name}
    url: git@github.com:{org}/{repo}.git
    description: "{what this repo contains}"
```

## Example Interaction

```
User: I'm building a social app for coordinating group outings.
      Consumer mobile app, merchant dashboard for venues,
      and a marketing landing page. Single backend API repo.

You: Based on your description, here's what I'd propose:

     Org: my-social-app (key: MSA)

     Components:
       Engineering  -- you're building software
       Product      -- feature specs, user research
       Design       -- UI/UX for consumer + merchant

     Areas:
       CON  Consumer App      -- mobile/web experience for end users
       MER  Merchant App      -- venue dashboard and analytics
       LND  Landing Page      -- marketing site, onboarding
       API  Backend API       -- shared services, auth, data

     Story IDs will look like: CON-001, MER-003, API-012

     Does this look right? I can adjust anything.
```

## Important

- Always ask before writing files
- Propose the minimal viable set -- don't over-engineer
- If unsure about an area split, ask the user
- Keys should be intuitive -- CON for Consumer, not CSM
- Check for existing domain.yaml before proposing (edit mode vs create mode)
