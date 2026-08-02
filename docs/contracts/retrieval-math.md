# V2 retrieval mathematics

This is the mathematical reference for [the V2 retrieval algorithm](retrieval-algorithm.md).
The equations define comparable signals; benchmark reports decide which
parameters and stages are enabled in production.

## 1. Vector normalization and similarity

For an embedding vector (x \in \mathbb{R}^{d}), normalize with a small guard:

```math
\hat{x} = \frac{x}{\max(\lVert x \rVert_2, \varepsilon)},
\qquad
\lVert x \rVert_2 = \sqrt{\sum_{i=1}^{d} x_i^2},
\qquad \varepsilon = 10^{-9}.
```

For unit vectors, cosine similarity is a dot product:

```math
\operatorname{cos}(q,x) = \hat{q} \cdot \hat{x}
  = \sum_{i=1}^{d} \hat{q}_i\hat{x}_i.
```

If a backend returns squared Euclidean distance, use the exact unit-vector
identity:

```math
\lVert \hat{q} - \hat{x} \rVert_2^2
  = 2 - 2\operatorname{cos}(q,x),
\qquad
\operatorname{cos}(q,x) = 1 - \frac{1}{2}d_2^2.
```

If a backend returns cosine distance (d_{cos} = 1 - \operatorname{cos}), use
`similarity = 1 - d_cos`. The adapter must declare which distance convention it
returns; silently treating squared L2 as cosine distance changes rankings.

## 2. BM25 lexical score

For document (D), query terms (t \in Q), term frequency (f(t,D)),
document length (|D|), average length (operatorname{avgdl}), collection
size (N), and document frequency (n(t)):

```math
\operatorname{BM25}(D,Q) =
\sum_{t \in Q} \operatorname{IDF}(t)
\frac{f(t,D)(k_1+1)}{f(t,D)+k_1\left(1-b+b\frac{|D|}{\operatorname{avgdl}}\right)},
```

```math
\operatorname{IDF}(t) =
\ln\left(\frac{N-n(t)+0.5}{n(t)+0.5}+1\right).
```

The starting parameters are (k_1=1.2) and (b=0.75), inherited from the
legacy Tantivy profile. They remain benchmark-tunable. BM25 score is only
comparable within the same query/corpus; do not blend raw BM25 with raw cosine.

## 3. Reciprocal Rank Fusion

For rankings (R_1,\ldots,R_m), where (r_j(x)) is the 1-based rank of item
(x) when present:

```math
\operatorname{RRF}(x) =
\sum_{j:x\in R_j}\frac{1}{k+r_j(x)}.
```

Higher is better. The V2 baseline is (k=60). Duplicate IDs within one
ranking count once. RRF is the default fusion candidate because it does not
assume BM25 and dense scores share a scale.

When a downstream component requires a lower-is-better retrieval cost, use:

```math
d_{rrf}(x) =
\begin{cases}
1/\operatorname{RRF}(x), & \operatorname{RRF}(x)>0,\\
\infty, & \text{otherwise.}
\end{cases}
```

## 4. Weighted score fusion

For a channel score (s), min-max normalization over that channel is:

```math
\operatorname{norm}(s_i) =
\begin{cases}
\dfrac{s_i-s_{min}}{s_{max}-s_{min}}, & s_{max}>s_{min},\\
1, & s_{max}=s_{min}.
\end{cases}
```

Let (v(x)) be normalized dense similarity and (ell(x)) normalized lexical
score. The evaluated weighted profile is:

```math
S_\alpha(x)=\alpha v(x)+(1-\alpha)\ell(x),
\qquad 0\le\alpha\le1.
```

Absent-channel contributions are zero. The legacy starting profile is
(alpha=0.9), but V2 uses it only when the benchmark selects it over RRF.

## 5. Relevance floor

For lexical scores (b_i), define (b_{top}=\max_i b_i). A per-query floor
keeps:

```math
b_i \ge \lambda b_{top},
\qquad \lambda=0.35 \text{ as the starting candidate}.
```

Apply this before fusion and only to lexical hits. A fused floor is not
well-defined for RRF because RRF intentionally compresses rank differences.

## 6. Final rank modifiers

Use a lower-is-better retrieval cost (d(x)): vector distance, (d_{rrf}), or
(1-S_\alpha) after the chosen fusion. For record age (a) in days:

```math
R(a)=\exp\left(-\ln(2)\frac{a}{H}\right),
\qquad H=30\text{ days as the starting half-life}.
```

With confidence (c\in[0,1]) and small weight (w_c=0.1):

```math
\operatorname{rank\_cost}(x)=
\frac{d(x)}{R(a_x)\left(1+w_c c_x\right)}.
```

Lower is better. Missing or invalid timestamps use a conservative fallback;
the exact fallback is a configuration decision. Ineligible records are
removed before this formula, so recency or confidence can never override
authorization or lifecycle state.

## 7. Cross-encoder ordering

For query (q), candidate text (D_i), and cross-encoder parameters
(\theta):

```math
s_i=f_\theta([\operatorname{CLS}]\Vert q\Vert[\operatorname{SEP}]\Vert D_i\Vert[\operatorname{SEP}]).
```

Order by descending (s_i), with typed ID as a deterministic tie-breaker. The
baseline scores only the top (N=30) fused candidates. Softmax is not applied
for ranking because any monotonic transform preserves order.

## 8. Token-budget packing

Let (C_i) be the token cost of candidate (i), measured with the configured
tokenizer, and (B) the caller's token budget. The baseline is greedy:

```text
used = 0
for candidate in ranked_order:
    if used + tokens(candidate) > B:
        stop
    emit candidate
    used += tokens(candidate)
```

This preserves ranking priority. The packet reports `tokens_used`, estimated
evidence coverage, and any omitted follow-up evidence.

## 9. Retrieval metrics

With retrieved top-(k) set (T_k(q)) and non-empty gold set (G(q)):

```math
\operatorname{Recall@}k(q)=
\begin{cases}1,&T_k(q)\cap G(q)\ne\emptyset,\\0,&\text{otherwise.}\end{cases}
```

For binary relevance (rel_i):

```math
\operatorname{DCG@}k=\sum_{i=0}^{k-1}\frac{rel_i}{\log_2(i+2)},
\qquad
\operatorname{nDCG@}k=\frac{\operatorname{DCG@}k}{\operatorname{IDCG@}k}.
```

If the first relevant item is at 1-based rank (r^*):

```math
\operatorname{MRR}(q)=\begin{cases}1/r^*,&r^*\text{ exists},\\0,&\text{otherwise.}\end{cases}
```

For session-based datasets, deduplicate chunk hits to session IDs before
computing these metrics. QA accuracy is a separate, judge-dependent metric and
must not be conflated with retrieval recall.

## 10. Benchmark gate

Phase E must report at least Recall@5/10, MRR, nDCG@10, latency percentiles,
packet tokens, unauthorized-result rate, wrong-Area rate, and degraded-mode
behavior. A tuning change is accepted only with the full configuration,
dataset split, sample size, model/provider versions, and comparison against
the exact-search baseline.

The legacy project measured a 99% Recall@5 result on a limited LongMemEval-S
sample under a specific embedding/fusion/reranker profile. V2 may use that
profile as a reproduction target, but it is not a V2 result until rerun under
V2 authorization, lifecycle, identity, and fixture rules.
