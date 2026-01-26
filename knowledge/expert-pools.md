# Expert Pool System

When running alignment dialogues, select domain-specific experts based on relevance to the topic.

## Expert Selection Algorithm

1. **Identify domains** relevant to the topic
2. **Select experts** by relevance tier:
   - **Core** (4): Highest relevance (0.75-0.95)
   - **Adjacent** (5): Medium relevance (0.50-0.70)
   - **Wildcard** (3): Low relevance but bring fresh perspectives (0.25-0.45)
3. **Assign pastry names** for identification (Muffin, Cupcake, Scone, Eclair, Donut, Brioche, Croissant, Macaron, Cannoli, Strudel, Beignet, Churro)

## Domain Expert Pools

### Infrastructure / DevOps
| Expert | Domain | Relevance |
|--------|--------|-----------|
| Platform Architect | Infra | 0.95 |
| SRE Lead | Infra | 0.90 |
| Database Architect | Infra | 0.85 |
| Security Engineer | Infra | 0.80 |
| Network Engineer | Infra | 0.70 |
| Cost Analyst | Finance | 0.55 |
| Compliance Officer | Legal | 0.45 |
| UX Researcher | Product | 0.35 |

### Product / Feature
| Expert | Domain | Relevance |
|--------|--------|-----------|
| Product Manager | Product | 0.95 |
| UX Designer | Product | 0.90 |
| Frontend Architect | Eng | 0.85 |
| Customer Advocate | Product | 0.80 |
| Data Analyst | Analytics | 0.70 |
| Backend Engineer | Eng | 0.65 |
| QA Lead | Eng | 0.55 |
| Marketing Strategist | Business | 0.35 |

### ML / AI
| Expert | Domain | Relevance |
|--------|--------|-----------|
| ML Architect | AI | 0.95 |
| Data Scientist | AI | 0.90 |
| MLOps Engineer | AI | 0.85 |
| AI Ethics Researcher | AI | 0.80 |
| Feature Engineer | AI | 0.70 |
| Platform Engineer | Infra | 0.60 |
| Privacy Counsel | Legal | 0.50 |
| Cognitive Scientist | Research | 0.35 |

### Governance / Policy
| Expert | Domain | Relevance |
|--------|--------|-----------|
| Governance Specialist | Gov | 0.95 |
| Legal Counsel | Legal | 0.90 |
| Ethics Board Member | Gov | 0.85 |
| Compliance Officer | Legal | 0.80 |
| Risk Analyst | Finance | 0.70 |
| Community Manager | Community | 0.60 |
| Economist | Economics | 0.50 |
| Anthropologist | Research | 0.35 |

### API / Integration
| Expert | Domain | Relevance |
|--------|--------|-----------|
| API Architect | Eng | 0.95 |
| Developer Advocate | Community | 0.90 |
| Integration Engineer | Eng | 0.85 |
| Security Architect | Security | 0.80 |
| Documentation Lead | Community | 0.70 |
| SDK Developer | Eng | 0.65 |
| Support Engineer | Community | 0.55 |
| Partner Manager | Business | 0.40 |

### General (default)
| Expert | Domain | Relevance |
|--------|--------|-----------|
| Systems Architect | Eng | 0.95 |
| Technical Lead | Eng | 0.90 |
| Product Manager | Product | 0.85 |
| Senior Engineer | Eng | 0.80 |
| QA Engineer | Eng | 0.70 |
| DevOps Engineer | Infra | 0.65 |
| Tech Writer | Community | 0.55 |
| Generalist | General | 0.40 |

## Expert Prompt Enhancement

Each expert receives their domain context in the prompt:

```
You are {expert_name} 🧁, a {domain_role} with expertise in {domain}.
Relevance to this topic: {relevance_score}

Bring your unique domain perspective while respecting that others see parts of the elephant you cannot.
```

## Panel Composition

For N=12 experts (typical for complex RFCs):
- 4 Core experts (highest domain relevance)
- 5 Adjacent experts (related domains)
- 3 Wildcard experts (distant domains for fresh thinking)

The Wildcards are crucial - they prevent groupthink and surface unexpected perspectives.

## Sampling Without Replacement

Each expert is used once per dialogue. If running multiple panels or rounds needing fresh experts, draw from the remaining pool.

---

*"The blind men who've never touched an elephant before often find the parts the experts overlook."*
