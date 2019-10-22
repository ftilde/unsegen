# Changelog

All breaking changes are marked with [BC] and potentially require API consumer changes after updating to the respective version.
## UNRELEASED
### Added
- Add `LineEdit::cursor_pos` to retrieve (byte) cursor position.
- Add `LineEdit::set_cursor_pos` to set (byte) cursor position.
- Implement `Deref<Target=LineEdit>` for PromptLine.
- Add `Behavior` implementation for slices of `ToEvent`s.
### Fixed
- Fix erasing characters in `LineEdit`.

## [0.2.1] - 2019-07-21
### Fixed
- Fix wrapping cursor outside of visible window.

## [0.2.0] - 2019-07-20
### Added
- Add Default variant to base::Color enum. [BC]
### Changed
- Change Default::default of base::Style to return default foreground and background Colors.
- All methods of base::{Style,Text}FormatModifier take self by value. [BC]
- All methods of base::Terminal propagate IO errors to the caller instead of panicking on failure. [BC]
- The output sink type of a base::Terminal is now required to be a std::unix::io::AsRawFd. [BC]

## [0.1.2] - 2019-04-04
### Added
- Add add_{vertical/horizontal} methods to Demand2D.
### Changed
- Allow construction of Terminals from arbitrary `io::Write`s.

## [0.1.1] - 2019-03-23
### Fixed
- Correctly specified MIT license.

## [0.1.0] - 2019-03-23
### Added
- Initial release.
