import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

export default withMermaid(
  defineConfig({
  vite: {
    optimizeDeps: {
      include: ['mermaid'],
    },
  },

  markdown: {
    languageAlias: {
      scm: 'scheme',
    },
  },

  title: 'Normalize',
  description: 'Code intelligence CLI with structural awareness',

  base: '/normalize/',

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/normalize/logo.svg' }],
  ],

  themeConfig: {
    logo: '/logo.svg',

    nav: [
      { text: 'Guide', link: '/introduction' },
      { text: 'CLI Reference', link: '/cli/commands' },
      { text: 'Design', link: '/philosophy' },
      { text: 'rhi', link: 'https://docs.rhi.zone/' },
    ],

    sidebar: {
      '/': [
        {
          text: 'Guide',
          items: [
            { text: 'Introduction', link: '/introduction' },
            { text: 'Rules', link: '/rules' },
            { text: 'Syntax Rules', link: '/syntax-rules' },
            { text: 'Fact Rules', link: '/fact-rules' },
            { text: 'Primitives Spec', link: '/primitives-spec' },
            { text: 'Language Support', link: '/language-support' },
          ]
        },
        {
          text: 'CLI Reference',
          items: [
            { text: 'Commands', link: '/cli/commands' },
            { text: 'view', link: '/cli/view' },
            { text: 'edit', link: '/cli/edit' },
            { text: 'history', link: '/cli/history' },
            { text: 'analyze', link: '/cli/analyze' },
            { text: 'text-search', link: '/cli/text-search' },
            { text: 'facts', link: '/cli/facts' },
            { text: 'rules', link: '/cli/rules' },
            { text: 'sessions', link: '/cli/sessions' },
            { text: 'package', link: '/cli/package' },
            { text: 'tools', link: '/cli/tools' },
            { text: 'serve', link: '/cli/serve' },
            { text: 'generate', link: '/cli/generate' },
            { text: 'translate', link: '/cli/translate' },
            { text: 'aliases', link: '/cli/aliases' },
            { text: 'context', link: '/cli/context' },
            { text: 'daemon', link: '/cli/daemon' },
            { text: 'grammars', link: '/cli/grammars' },
            { text: 'init', link: '/cli/init' },
            { text: 'update', link: '/cli/update' },
            { text: 'Tools (MCP)', link: '/tools' },
          ]
        },
        {
          text: 'Design',
          items: [
            { text: 'Philosophy', link: '/philosophy' },
            { text: 'Architecture Decisions', link: '/architecture-decisions' },
            { text: 'Lint Architecture', link: '/lint-architecture' },
            { text: 'CLI Design', link: '/cli-design' },
            { text: 'Unification', link: '/unification' },
            { text: 'View Filtering', link: '/view-filtering' },
            { text: 'Shadow Git', link: '/design/shadow-git' },
            { text: 'Builtin Rules', link: '/design/builtin-rules' },
            { text: 'Rule Sharing', link: '/design/rule-sharing' },
            { text: 'Sessions Refactor', link: '/design/sessions-refactor' },
            { text: 'Syntax Linting', link: '/design/syntax-linting' },
            { text: 'Test Gaps', link: '/design/test-gaps' },
          ]
        },
        {
          text: 'Development',
          collapsed: true,
          items: [
            { text: 'Dogfooding', link: '/dogfooding' },
            { text: 'Session Modes', link: '/session-modes' },
            { text: 'Documentation Strategy', link: '/documentation' },
            { text: 'Repository Coverage', link: '/repository-coverage' },
            { text: 'AST-grep Retro', link: '/retro-ast-grep-implementation' },
          ]
        },
        {
          text: 'Research',
          collapsed: true,
          items: [
            { text: 'Spec', link: '/spec' },
            { text: 'LLM Evaluation', link: '/llm-evaluation' },
            { text: 'LLM Comparison', link: '/llm-comparison' },
            { text: 'LangGraph Evaluation', link: '/langgraph-evaluation' },
            { text: 'LLM Code Consistency', link: '/llm-code-consistency' },
            { text: 'Edit Paradigm Comparison', link: '/edit-paradigm-comparison' },
            { text: 'Prior Art', link: '/prior-art' },
            { text: 'Ampcode', link: '/research/ampcode' },
            { text: 'Agent Adaptation', link: '/research/agent-adaptation' },
            { text: 'Recursive Language Models', link: '/research/recursive-language-models' },
            { text: 'Log Analysis', link: '/log-analysis' },
            { text: 'Low-Priority Research', link: '/research-low-priority' },
          ]
        },
        {
          text: 'Workflows',
          collapsed: true,
          items: [
            { text: 'Overview', link: '/workflows/' },
            { text: 'API Review', link: '/workflows/api-review' },
            { text: 'Binding Generation', link: '/workflows/binding-generation' },
            { text: 'Breaking API Changes', link: '/workflows/breaking-api-changes' },
            { text: 'Bug Fix', link: '/workflows/bug-fix' },
            { text: 'Bug Investigation', link: '/workflows/bug-investigation' },
            { text: 'Codebase Onboarding', link: '/workflows/codebase-onboarding' },
            { text: 'Codebase Orientation', link: '/workflows/codebase-orientation' },
            { text: 'Code Review', link: '/workflows/code-review' },
            { text: 'Code Synthesis', link: '/workflows/code-synthesis' },
            { text: 'Cross-Language Migration', link: '/workflows/cross-language-migration' },
            { text: 'Cross-Workflow Analysis', link: '/workflows/cross-workflow-analysis' },
            { text: 'Cryptanalysis', link: '/workflows/cryptanalysis' },
            { text: 'Dead Code Elimination', link: '/workflows/dead-code-elimination' },
            { text: 'Debugging Practices', link: '/workflows/debugging-practices' },
            { text: 'Debugging Production Issues', link: '/workflows/debugging-production-issues' },
            { text: 'Dependency Tracing', link: '/workflows/dependency-tracing' },
            { text: 'Dependency Updates', link: '/workflows/dependency-updates' },
            { text: 'Documentation Sync', link: '/workflows/documentation-sync' },
            { text: 'Documentation Synthesis', link: '/workflows/documentation-synthesis' },
            { text: 'Feature Implementation', link: '/workflows/feature-implementation' },
            { text: 'Flaky Test Debugging', link: '/workflows/flaky-test-debugging' },
            { text: 'Grammar/Parser Generation', link: '/workflows/grammar-parser-generation' },
            { text: 'Malware Analysis', link: '/workflows/malware-analysis' },
            { text: 'Merge Conflict Resolution', link: '/workflows/merge-conflict-resolution' },
            { text: 'Migration', link: '/workflows/migration' },
            { text: 'Performance Regression Hunting', link: '/workflows/performance-regression-hunting' },
            { text: 'Quality Audit', link: '/workflows/quality-audit' },
            { text: 'Question Answering', link: '/workflows/question-answering' },
            { text: 'Refactoring', link: '/workflows/refactoring' },
            { text: 'Reverse Engineering (Binary)', link: '/workflows/reverse-engineering-binary' },
            { text: 'Reverse Engineering (Code)', link: '/workflows/reverse-engineering-code' },
            { text: 'Security Audit', link: '/workflows/security-audit' },
            { text: 'Steganography Detection', link: '/workflows/steganography-detection' },
            { text: 'Tech Debt', link: '/workflows/tech-debt' },
            { text: 'Test Coverage', link: '/workflows/test-coverage' },
          ]
        },
        {
          text: 'Archive',
          collapsed: true,
          items: [
            { text: 'Agent', link: '/archive/agent' },
            { text: 'Agent Architecture', link: '/archive/agent-architecture' },
            { text: 'Agent Prompts', link: '/archive/agent-prompts' },
            { text: 'Agent Commands', link: '/archive/agent-commands' },
            { text: 'Agent Dogfooding', link: '/archive/agent-dogfooding' },
            { text: 'Agent State Machine', link: '/archive/agent-state-machine' },
            { text: 'Agent v2', link: '/archive/agent-v2' },
            { text: 'Agentic Loop', link: '/archive/agentic-loop' },
            { text: 'Batch Edit', link: '/archive/batch-edit' },
            { text: 'Architecture Review (Dec 22)', link: '/archive/architecture-review-dec22' },
            { text: 'Comprehensive Overview', link: '/archive/comprehensive-overview' },
            { text: 'Driver Architecture', link: '/archive/driver-architecture' },
            { text: 'Execution Primitives', link: '/archive/execution-primitives' },
            { text: 'Generators', link: '/archive/generators' },
            { text: 'Hybrid Loops', link: '/archive/hybrid-loops' },
            { text: 'Lua API', link: '/archive/lua-api' },
            { text: 'Lua CLI', link: '/archive/lua-cli' },
            { text: 'Lua Test', link: '/archive/lua-test' },
            { text: 'Lua Type', link: '/archive/lua-type' },
            { text: 'Memory', link: '/archive/memory' },
            { text: 'Rust-Python Boundary', link: '/archive/rust-python-boundary' },
            { text: 'Script', link: '/archive/script' },
            { text: 'Synthesis Roadmap', link: '/archive/synthesis-roadmap' },
            { text: 'Telemetry', link: '/archive/telemetry' },
            { text: 'Workflow Format', link: '/archive/workflow-format' },
          ]
        },
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/rhi-zone/normalize' }
    ],

    search: {
      provider: 'local'
    },

    editLink: {
      pattern: 'https://github.com/rhi-zone/normalize/edit/master/docs/:path',
      text: 'Edit this page on GitHub'
    },
  },

}),
)
