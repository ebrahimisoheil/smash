# 23 — Architecture Diagrams

> Source: SMASH_V2.md §24

**These diagrams are part of the implementation contract. Update them when a service boundary or data responsibility changes.**

## 24.1 System context: Sources to governed Memory to agents

```mermaid
flowchart LR
    subgraph Sources[Source systems]
        Files[Local files and uploads]
        SaaS[Notion / Jira / CRM / Drive]
        Media[Calls / images / video]
        ExtMCP[External MCP resources]
    end

    subgraph Smash[SMASH V2]
        Library[Source Library]
        Pipeline[Ingestion and extraction]
        Review[Proposal and Review]
        Memory[Governed Memory]
        Maps[Area Maps and Cross-Map]
        Rules[Rules and Harness]
        Retrieval[Light and Aggressive Retrieval]
    end

    subgraph Agents[Agent surfaces]
        Chat[ChatGPT / Claude]
        Code[Codex / Cursor]
        Internal[Internal agents]
        UI[Next.js application]
    end

    Files --> Library
    SaaS --> Library
    Media --> Library
    ExtMCP --> Library
    Library --> Pipeline
    Pipeline --> Review
    Review --> Memory
    Maps --> Review
    Memory --> Retrieval
    Maps --> Retrieval
    Rules --> Retrieval
    Retrieval --> Chat
    Retrieval --> Code
    Retrieval --> Internal
    Memory --> UI
    Rules -. mechanically gate .-> Agents
```

## 24.2 Community Edition Docker Compose architecture

```mermaid
flowchart TB
    Browser[Browser]
    Agent[Local agent host]

    subgraph Compose[Docker Compose deployment]
        Web[Next.js web]
        API[Axum API - Rust]
        MCP[MCP stdio / local HTTP adapter]
        Worker[Background worker]
        PG[(PostgreSQL canonical store)]
        MinIO[(MinIO object storage)]
        Lance[(LanceDB retrieval index)]
        Init[Migration and initialization job]
    end

    Browser --> Web
    Web --> API
    Agent --> MCP
    MCP --> API
    API --> PG
    API --> MinIO
    API --> Lance
    API -->|enqueue Operation| PG
    Worker -->|claim jobs| PG
    Worker --> MinIO
    Worker --> Lance
    Worker --> PG
    Init --> PG
    Init --> MinIO
```

## 24.3 Managed-service evolution after Community Edition

```mermaid
flowchart TB
    Users[Users and agent clients]
    Edge[CDN / reverse proxy / rate limits]
    Identity[OIDC / SSO / organization identity]
    API[Replicated Axum services]
    Web[Replicated Next.js services]
    Queue[Durable managed queue]
    Workers[Autoscaled processing workers]
    PG[(Managed PostgreSQL)]
    Objects[(Managed S3-compatible storage)]
    Vectors[(Distributed LanceDB or validated vector tier)]
    Audit[Audit / SIEM / observability]

    Users --> Edge
    Edge --> Web
    Edge --> API
    Identity --> Edge
    API --> PG
    API --> Objects
    API --> Vectors
    API --> Queue
    Queue --> Workers
    Workers --> PG
    Workers --> Objects
    Workers --> Vectors
    API --> Audit
    Workers --> Audit
    Identity --> PG
```

## 24.4 Canonical data and projection relationship

```mermaid
flowchart LR
    SourceBytes[Original Source bytes]
    MinIO[(MinIO)]
    Canonical[Sources / chunks / entities / Memory / Rules / Events]
    PG[(PostgreSQL)]
    Projection[Retrieval projection and vectors]
    Lance[(LanceDB)]
    Export[Portable export / Markdown]

    SourceBytes --> MinIO
    MinIO -->|object key and hash| Canonical
    Canonical --> PG
    PG -->|projection jobs| Projection
    Projection --> Lance
    PG --> Export
    MinIO --> Export
    Lance -. rebuildable from canonical records .-> PG
```

## 24.5 Source ingestion pipeline

```mermaid
flowchart LR
    Add[Upload / connector / MCP resource]
    Verify[Verify identity, checksum, type and permissions]
    Store[Store immutable Source version]
    Extract[Extract text, OCR, transcript or structure]
    Chunk[Create stable chunks and evidence coordinates]
    Index[Index lexical and vector projections]
    Propose[Propose entities, relations, Memory and Map changes]
    Review[Human or policy review]
    Active[Active governed Memory]
    Quarantine[Quarantine or actionable failure]

    Add --> Verify
    Verify --> Store
    Verify -->|unsafe| Quarantine
    Store --> Extract
    Extract -->|failure| Quarantine
    Extract --> Chunk
    Chunk --> Index
    Chunk --> Propose
    Propose --> Review
    Review -->|accept| Active
    Review -->|reject or defer| Propose
```

## 24.6 Memory proposal and upsert decision graph

```mermaid
flowchart TD
    Candidate[Candidate claim with reason and evidence]
    Auth{Authorized?}
    Rule{Rule decision}
    Idem{Idempotent replay?}
    Exact{Exact duplicate?}
    Semantic{Semantic duplicate?}
    Conflict{Contradiction?}
    Proposal[Create or update Proposal]
    Merge[Merge evidence into reviewed logical Memory]
    Resolve[Conflict Review and supersession decision]
    Active[Create active Memory version]
    Event[Append Event atomically]
    Block[Block and record reason]

    Candidate --> Auth
    Auth -->|no| Block
    Auth -->|yes| Rule
    Rule -->|block| Block
    Rule -->|allow / warn / approval| Idem
    Idem -->|yes| Event
    Idem -->|no| Exact
    Exact -->|yes| Merge
    Exact -->|no| Semantic
    Semantic -->|possible| Proposal
    Semantic -->|no| Conflict
    Conflict -->|yes| Resolve
    Conflict -->|no| Proposal
    Proposal -->|approved| Active
    Resolve -->|approved replacement| Active
    Merge --> Event
    Active --> Event
    Block --> Event
```

## 24.7 Light and Aggressive retrieval router

```mermaid
flowchart TD
    Request[Query + actor + agent + task + active Area + budget]
    Policy[Resolve permissions and Rules]
    Router{Retrieval router}

    subgraph Light[Light search]
        L1[PostgreSQL lexical candidates]
        L2[LanceDB vector candidates with prefilters]
        L3[Merge and deterministic ranking]
        L4[Compact Memory packet]
    end

    subgraph Aggressive[Aggressive search]
        A1[Decompose question]
        A2[Cross-Map and bounded graph expansion]
        A3[Memory plus Source-chunk retrieval]
        A4[Contradiction and temporal checks]
        A5[Rerank / synthesize with citations]
        A6[Answer packet, trace and optional Proposals]
    end

    Request --> Policy --> Router
    Router -->|default| L1
    Router -->|default| L2
    L1 --> L3
    L2 --> L3
    L3 --> L4
    Router -->|explicit, low confidence, cross-Area or high impact| A1
    A1 --> A2 --> A3 --> A4 --> A5 --> A6
    L4 -->|insufficient or contradictory| A1
```

## 24.8 Area Maps and Cross-Map architecture

```mermaid
flowchart LR
    subgraph Sales[Sales Area Map v3]
        Account[Account]
        Opportunity[Opportunity]
        Champion[Champion]
        Account -->|has| Opportunity
        Champion -->|supports| Opportunity
    end

    subgraph Marketing[Marketing Area Map v2]
        Audience[Audience]
        Campaign[Campaign]
        Customer[Customer]
        Campaign -->|targets| Audience
    end

    subgraph Product[Product Area Map v4]
        User[User]
        Problem[Problem]
        Signal[Customer Signal]
        User -->|experiences| Problem
        Signal -->|supports| Problem
    end

    CrossMap[Versioned Cross-Map registry]
    Account -->|equivalent_to, approved| CrossMap
    Customer -->|equivalent_to, approved| CrossMap
    Audience -->|related_to, approved| CrossMap
    User -->|narrower_than, approved| CrossMap
    Signal -->|derived_from, approved| CrossMap
```

## 24.9 Rule harness around agent actions

```mermaid
sequenceDiagram
    participant Agent
    participant Harness as SMASH Rule Harness
    participant Memory as Memory and Source services
    participant Tool as External MCP tool
    participant Events as Activity log

    Agent->>Harness: Request retrieval or controlled action
    Harness->>Memory: Resolve actor, Area, data sensitivity and applicable Rules
    Memory-->>Harness: Authorized context and Rule versions
    alt blocked
        Harness->>Events: Record block and rationale
        Harness-->>Agent: Blocked with safe next action
    else approval required
        Harness->>Events: Record pending approval
        Harness-->>Agent: Ask user for approval
    else allowed
        Harness->>Tool: Execute constrained action
        Tool-->>Harness: Result
        Harness->>Events: Record action, Rule and result classification
        Harness-->>Agent: Sanitized result
    end
```

## 24.10 Agent session and memory loop

```mermaid
stateDiagram-v2
    [*] --> Status
    Status --> Brief: session starts
    Brief --> Work
    Work --> LightRecall: durable context needed
    LightRecall --> Work: compact packet
    Work --> AggressiveSearch: verify, compare, investigate
    AggressiveSearch --> Work: cited packet and trace
    Work --> RuleCheck: controlled action requested
    RuleCheck --> Work: allow, warn or approval
    RuleCheck --> Blocked: block
    Work --> Capture: session ends
    Capture --> Proposal: memory-worthy observation
    Capture --> [*]: nothing durable
    Proposal --> Review
    Review --> ActiveMemory: accepted
    Review --> [*]: rejected or deferred
    ActiveMemory --> [*]
    Blocked --> Work
```

## 24.11 MCP server, consumer, skills, prompts, and registry

```mermaid
flowchart TB
    subgraph Hosts[Agent hosts]
        Codex[Codex]
        Claude[Claude]
        ChatGPT[ChatGPT]
        Other[Other MCP clients]
    end

    Skills[SMASH skills and prompts]
    LocalMCP[Community MCP server: stdio]
    RemoteMCP[Managed MCP server: Streamable HTTP + OAuth]
    Core[Rust application core crate]
    Gateway[External MCP gateway and Rule checks]
    Catalog[Trusted installed-server catalog]
    Registry[Official MCP Registry metadata]
    External[Approved external MCP servers]

    Skills --> Hosts
    Codex --> LocalMCP
    Claude --> LocalMCP
    ChatGPT --> RemoteMCP
    Other --> RemoteMCP
    LocalMCP --> Core
    RemoteMCP --> Core
    Core --> Gateway
    Catalog --> Gateway
    Gateway --> External
    LocalMCP -. package metadata .-> Registry
    RemoteMCP -. remote metadata .-> Registry
```

## 24.12 High-level relational model

```mermaid
erDiagram
    TENANT ||--o{ MEMBERSHIP : authorizes
    TENANT ||--o{ AREA : contains
    TENANT ||--o{ EVENT : records
    TENANT ||--o{ AI_SESSION : owns
    MEMBERSHIP }o--|| ENTERPRISE_ROLE : grants
    AREA ||--o{ MAP_VERSION : defines
    AREA ||--o{ SOURCE : owns
    AREA ||--o{ ENTITY : owns
    AREA ||--o{ MEMORY : scopes
    AREA ||--o{ RULE : governs
    MAP_VERSION ||--o{ MAP_KIND : contains
    MAP_VERSION ||--o{ MAP_RELATION : contains
    MAP_VERSION ||--o{ CROSS_MAP_MAPPING : maps
    SOURCE ||--o{ SOURCE_VERSION : versions
    SOURCE_VERSION ||--o{ SOURCE_ARTIFACT : derives
    SOURCE_VERSION ||--o{ CHUNK : addresses
    ENTITY ||--o{ RELATIONSHIP : source
    ENTITY ||--o{ RELATIONSHIP : target
    MEMORY ||--o{ MEMORY_VERSION : versions
    MEMORY_VERSION ||--o{ EVIDENCE_LINK : supported_by
    CHUNK ||--o{ EVIDENCE_LINK : cites
    MEMORY ||--o{ MEMORY : supersedes
    PROPOSAL }o--|| AREA : targets
    PROPOSAL }o--o{ EVIDENCE_LINK : proposes_from
    RULE ||--o{ RULE_VERSION : versions
    OPERATION ||--o{ EVENT : emits
    AI_SESSION ||--o{ AI_RUN : contains
    AI_RUN ||--o{ RETRIEVAL_EVENT : retrieves
    RETRIEVAL_EVENT ||--o{ RETRIEVAL_ITEM : selects
    MEMORY_VERSION ||--o{ RETRIEVAL_ITEM : influences
    AI_RUN ||--o{ RULE_EVALUATION : evaluates
    AI_RUN ||--o{ TOOL_CALL : executes
    AI_RUN ||--o{ DECISION_ENVELOPE : produces
    DECISION_ENVELOPE ||--o{ OUTCOME_LINK : affects
```

## 24.13 Managed tenant topology

```mermaid
flowchart TB
    subgraph Platform[Shared SMASH platform]
        API[Axum services]
        Worker[Workers]
        PG[(Shared PostgreSQL schema)]
        MinIO[(Shared MinIO service)]
        Catalog[LanceDB catalog]
        Placement[Tenant placement registry]
    end

    subgraph Acme[Tenant: Acme]
        AcmeRows[Rows with tenant_id = Acme]
        AcmeObjects[tenants/Acme object prefixes]
        AcmeVectors[tenant_Acme namespace]
    end

    subgraph Globex[Tenant: Globex]
        GlobexRows[Rows with tenant_id = Globex]
        GlobexObjects[tenants/Globex object prefixes]
        GlobexVectors[tenant_Globex namespace]
    end

    API --> Placement
    Worker --> Placement
    Placement --> PG
    Placement --> MinIO
    Placement --> Catalog
    PG --> AcmeRows
    PG --> GlobexRows
    MinIO --> AcmeObjects
    MinIO --> GlobexObjects
    Catalog --> AcmeVectors
    Catalog --> GlobexVectors
```

## 24.14 Tenant provisioning state machine

```mermaid
stateDiagram-v2
    [*] --> Provisioning
    Provisioning --> TenantRecord
    TenantRecord --> FirstEnterpriseAdmin
    FirstEnterpriseAdmin --> DefaultMemoryMapRules
    DefaultMemoryMapRules --> ObjectPrefixes
    ObjectPrefixes --> LanceNamespace
    LanceNamespace --> PlacementRecorded
    PlacementRecorded --> Verification
    Verification --> Active: all checks pass
    Verification --> Failed: actionable failure
    Failed --> Provisioning: idempotent retry
    Active --> Suspended: enterprise or policy action
    Suspended --> Deleting: confirmed retention-aware deletion
    Deleting --> Deleted
```

## 24.15 Enterprise role and access model

```mermaid
flowchart TD
    Tenant[Enterprise tenant]
    EnterpriseAdmin[Enterprise Admin]
    Governance[AI Governance Admin]
    AreaAdmin[Area Admin]
    User[Normal User]
    Agent[Agent or service identity]
    PlatformOperator[SMASH platform operator]

    AllContent[All tenant content allowed by enterprise policy]
    AllTraces[All tenant AI traces and analytics]
    AreaContent[Assigned Area content and traces]
    UserContent[Permitted and owned content]
    ScopedTools[Explicit machine scopes]
    Infrastructure[Infrastructure metadata]

    Tenant --> EnterpriseAdmin
    Tenant --> Governance
    Tenant --> AreaAdmin
    Tenant --> User
    Tenant --> Agent
    EnterpriseAdmin --> AllContent
    EnterpriseAdmin --> AllTraces
    Governance --> AllTraces
    AreaAdmin --> AreaContent
    User --> UserContent
    Agent --> ScopedTools
    PlatformOperator --> Infrastructure
    PlatformOperator -. break-glass grant only .-> AllContent
```

## 24.16 AI decision trace, replay, and outcome graph

```mermaid
flowchart TD
    Session[Agent session]
    Run[AI run or task]
    Retrieval[Retrieval event]
    Context[Immutable decision envelope]
    Model[Model invocation]
    Rules[Rule evaluations]
    Approval[Human approval]
    Tool[Tool call]
    Decision[Recommendation or action]
    Outcome[Application or business outcome]
    Feedback[Human correction or acceptance]
    Forensic[Forensic replay]
    Reproduction[Execution reproduction]
    Counterfactual[Counterfactual replay]
    Analytics[Enterprise decision analytics]

    Session --> Run
    Run --> Retrieval --> Context
    Context --> Model
    Context --> Rules
    Rules --> Approval
    Rules --> Tool
    Approval --> Tool
    Model --> Decision
    Tool --> Decision
    Decision --> Outcome --> Feedback
    Context --> Forensic
    Context --> Reproduction
    Context --> Counterfactual
    Retrieval --> Analytics
    Decision --> Analytics
    Outcome --> Analytics
    Feedback --> Analytics
```
