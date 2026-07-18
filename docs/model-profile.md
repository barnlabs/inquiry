# Inquiry Planner Profile

The planner profile is a specification, not model weights and not a claim of custom training.

Current decision as of 2026-07-16:

- v0.1 ships no transformer runtime or weights;
- deterministic routing is the supported default;
- future experiments may evaluate openly licensed local models in roughly the 2B–4B class for mobile and desktop, but no family, size, or quantization is recommended until measured on representative devices.

The model may propose only a typed plan with facets, entities, geography, dates, and connector names. It may not invent citations, facts, formulas, units, source quality, or confidence. The engine validates policy and schema, executes connectors, and calculates values independently.

Before enabling a runtime, evaluate public and synthetic prompts across routing accuracy, schema validity, entity ambiguity, over-collection, prompt injection, unsafe person targeting, medical overreach, source selection, latency, peak memory, and energy use. Publish the prompt set, device/runtime/version, quantization, results, and known failures. Do not call the profile “custom trained” unless a separately governed training project actually exists and publishes a model card.

## Inkling review

Thinking Machines Lab's July 2026 [Inkling announcement](https://thinkingmachines.ai/news/introducing-inkling/) and [model card](https://thinkingmachines.ai/model-card/inkling/) describe an Apache-2.0 open-weights multimodal Mixture-of-Experts transformer with 975B total parameters and 41B active parameters. The announcement also describes an Inkling-Small preview with 12B active parameters. Those are interesting future agent/provider profiles, but neither matches Inquiry's stated 2B–4B light-local target for ordinary student laptops or mobile devices.

Inquiry therefore does not bundle, download, or recommend Inkling as its local planner. Any later evaluation must record the exact weights or hosted provider, total storage, quantization, peak unified/GPU memory, time to first token, sustained latency, energy use, license and acceptable-use terms, and the same safety/relevance benchmark used for smaller candidates. A remote profile must also disclose query transmission and cost before use.
