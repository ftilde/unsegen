unsegen
=======

unsegen is a library facilitating the creation of text user interface (TUI) applications akin to ncurses and the like.
Currently, unsegen only provides a Rust interface.

The following libraries are built on top of unsegen:

* unsegen-terminal provides a pseudoterminal that can be easily integrated into applications using unsegen.
* unsegen-jsonviewer provides an interactive widget that can be used to display json values.
* unsegen-signal uses unsegen's input module to raise signals on the usual key combinations (e.g., SIGINT on CTRL-C).

**State**

At its current state unsegen is more or less usable, but the documentation is still quite lacking.
I currently plan on improve the documentation and fix inconveniences in the API along the way.
Once that is done I will release version 0.1.0.

**Licensing**

The majority of unsegen is released under the MIT license. This applies to all files that do not explicitly state to be licensed differently.
