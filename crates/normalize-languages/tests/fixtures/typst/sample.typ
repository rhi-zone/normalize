#import "template.typ": report, section, styled
#import "@preview/tablex:0.0.6": tablex, cellx

#set document(
  title: "Project Report",
  author: "Alice Smith",
)

#set text(font: "New Computer Modern", size: 11pt)
#set page(margin: (top: 2cm, bottom: 2cm, left: 2.5cm, right: 2.5cm))
#set heading(numbering: "1.1")

#show heading: it => {
  set text(weight: "bold")
  it
}

#let project_name = "Normalize"
#let version = "0.1.0"

#let format_version(major, minor, patch) = {
  [#major.#minor.#patch]
}

#let summary_table(rows) = {
  tablex(
    columns: (auto, 1fr, auto),
    [*Name*], [*Description*], [*Status*],
    ..rows
  )
}

= Introduction

This document describes the #project_name project (version #format_version(0, 1, 0)).

== Motivation

The tool addresses several common problems:

- Code analysis across many languages
- Consistent output formatting
- Integration with existing toolchains

== Scope

#section[
  This report covers the architecture, design decisions, and implementation
  details of version #version.
]

= Architecture

The system consists of several components:

#summary_table((
  [Core], [Symbol extraction and indexing], [Stable],
  [CLI], [Command-line interface], [Active],
  [Rules], [Static analysis rules], [Active],
))

= Conclusion

The #project_name tool provides a unified interface for code analysis.
