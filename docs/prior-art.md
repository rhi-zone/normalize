# Prior Art & Research References

Related work that influenced moss's design or represents the competitive landscape.

## Program Synthesis

### DreamCoder
- **Paper**: [DreamCoder: Bootstrapping Inductive Program Synthesis with Wake-Sleep Library Learning](https://arxiv.org/abs/2006.08381)
- **Relevance**: Moss aims to be "DreamCoder for LLMs" - using LLMs as the synthesis engine rather than enumeration, but with similar goals of discovering reusable abstractions
- **Key ideas**:
  - Compression-based abstraction discovery
  - MDL (Minimum Description Length) scoring for abstractions
  - Library learning: extract common patterns into reusable primitives
- **Moss approach**: Instead of enumerating programs, we use LLMs with structural context. The abstraction discovery could still apply to learned preferences/patterns.

### Other Synthesis Systems

**Enumerative / Search-based:**
- **Escher/Myth**: Enumerative synthesis with examples
- **SyPet/InSynth**: Component-based synthesis (combining library functions)
- **FlashFill/PROSE**: Programming by Example
- **Sketch/Rosette**: Hole-filling in user templates

**Type-directed:**
- **Synquid**: Refinement type-guided synthesis with liquid types
- **λ² (Lambda Squared)**: Bidirectional type+example guided search
- **Idris**: Dependently typed language with proof search / auto tactics
- **Agda**: Dependently typed proof assistant, Agsy auto-search

**Logic/Relational:**
- **miniKanren**: Relational programming, run programs "backwards"
- **Prolog**: Logic programming, unification-based search

**SMT-based:**
- **Z3**: SMT solver used by many synthesis tools
- **Rosette**: Solver-aided programming (uses Z3)

See `docs/synthesis-generators.md` for how these map to moss generator plugins.

## Coding Agents

### SWE-agent
- **Repo**: https://github.com/swe-agent/swe-agent
- **What it is**: Princeton's autonomous agent for software engineering tasks (GitHub issues → PRs)
- **Why it matters**: Direct competitor/prior art for `moss run`
- **What to learn**:
  - Their agent-computer interface (ACI) design
  - How they handle long-horizon tasks
  - Their benchmark (SWE-bench) performance

### Aider
- **Repo**: https://github.com/paul-gauthier/aider
- **What it is**: AI pair programming in terminal
- **Why it matters**: Popular CLI coding assistant
- **What to learn**:
  - Their edit format (search/replace blocks)
  - How they handle multi-file edits
  - Git integration patterns

### OpenHands (formerly OpenDevin)
- **Repo**: https://github.com/All-Hands-AI/OpenHands
- **What it is**: Platform for AI software developers
- **Why it matters**: Open source agent framework
- **What to learn**: Their sandbox/runtime approach

### GUIRepair
- **Paper**: https://sites.google.com/view/guirepair
- **What it is**: GUI-based program repair
- **Relevance**: Alternative interaction modality (visual vs text)

## Questions to Answer

For each competitor:
1. Do they do something moss doesn't?
2. Have they discovered patterns we should adopt?
3. What's their weakness that moss addresses?
4. Are they solving the same problem differently, or a different problem?

The key question: **Is moss's structural-awareness approach actually better, or just different?**

We should periodically benchmark against SWE-bench and compare approaches.
