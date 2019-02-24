# unsegen

`unsegen` is a library facilitating the creation of text user interface (TUI) applications akin to ncurses.
Currently, `unsegen` only provides a Rust interface.

## Overview

The library consists of four modules:

* base: Basic terminal rendering including `Terminal` setup, "slicing" using `Windows`, and formatted writing to `Windows` using `Cursors`.
* widget: `Widget` abstraction and some basic `Widget`s useful for creating basic building blocks of text user interfaces.
* input: Raw terminal input events, common abstractions for application (component) `Behavior` and means to easily distribute events.
* container: Higher level window manager like functionality using `Container`s as the combination of widget and input concepts.

The following libraries are built on top of unsegen and provide higher level functionality:

* unsegen-jsonviewer provides an interactive widget that can be used to display json values.
* unsegen-pager provides a memory or file backed line buffer viewer with syntax highlighting and line decorations.
* unsegen-signal uses unsegen's input module to raise signals on the usual key combinations (e.g., SIGINT on CTRL-C).
* unsegen-terminal provides a pseudoterminal that can be easily integrated into applications using unsegen.

## Getting Started

`unsegen` is [available on crates.io](https://crates.io/crates/unsegen). You can install it by adding this line to your `Cargo.toml`:

```toml
unsegen = "0.1"
```

## Examples

There are examples at the top of each modules' documentation which should be sufficient to get you going.

For a fully fledged application using `unsegen`, you can have a look at [ugdb](https://github.com/ftilde/ugdb), which was developed alongside `unsegen` and the primary motivation for it.

## Screenshots

Here is a screenshot of [ugdb](https://github.com/ftilde/ugdb), which is implemented on top of `unsegen`.

((TODO))

## Some notes on implementation

For simplicity, layouting is done in every draw call.
This, in conjunction with recursive calls to calculate space demand of widgets, leads to not-so-great asymptotic runtime.
However, I found this not to be a problem in practice so far.
If this is problematic for, please file an issue.
There are workarounds (caching the `draw`-result of widgets) for which convenient wrappers can be implemented in the library, but have not so far.

## Licensing

The majority of `unsegen` is released under the MIT license. This applies to all files that do not explicitly state to be licensed differently.
