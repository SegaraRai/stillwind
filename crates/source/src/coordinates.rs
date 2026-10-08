//! Explicit UTF-16 coordinates at host and source-map boundaries.
//!
//! Editors, language servers and source maps count UTF-16 code units, while every Rust
//! pass in this workspace addresses text in UTF-8 bytes. [`CoordinateIndex`] snapshots
//! the scalar boundaries of one [`SourceFile`] in both systems and converts in either
//! direction. A coordinate that falls inside a surrogate pair names no character, so it
//! is rejected instead of rounded to the nearest boundary; rounding would silently
//! attach an edit to the wrong text. Build one index per source and reuse it:
//! construction walks the whole text, while each query is a binary search.

use serde::{Deserialize, Serialize};

use crate::{BytePos, Position, SourceFile};

/// Failure to translate between a host coordinate and a source byte offset.
///
/// The variant names the rejected input: a UTF-16 offset, a line/column [`Position`],
/// or a [`BytePos`] that does not start a character.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CoordinateError {
    /// The offset is out of range or inside a surrogate pair.
    #[error("invalid UTF-16 offset {0:?}")]
    Offset(
        /// Rejected document-wide UTF-16 offset.
        Utf16Offset,
    ),
    /// The line does not exist, or the column crosses the end of the line.
    #[error("invalid UTF-16 position {0:?}")]
    Position(
        /// Rejected zero-based line and UTF-16 column.
        Position,
    ),
    /// The byte offset is out of range or splits a multi-byte character.
    #[error("invalid UTF-8 byte offset {0:?}")]
    Byte(
        /// Rejected UTF-8 byte offset.
        BytePos,
    ),
}

/// A UTF-16 code-unit offset from the start of a source.
///
/// Use [`CoordinateIndex`] to validate and translate this host coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Utf16Offset(
    /// Zero-based UTF-16 code-unit count; scalar-boundary validity requires a source index.
    pub u32,
);

/// Bidirectional scalar-boundary index between UTF-16 code units and UTF-8 bytes.
///
/// Every character boundary of the source appears exactly once, in both coordinate
/// systems. Surrogate interiors, multi-byte interiors and out-of-range values have no
/// entry and therefore fail to convert instead of being rounded. The index borrows
/// nothing, so it may outlive the temporary borrows used to build it.
///
/// Built once per source so mapping many spans never rescans the source prefix.
#[derive(Clone, Debug)]
pub struct CoordinateIndex {
    /// Pairs of UTF-8 byte and UTF-16 code-unit offsets, strictly increasing in both
    /// coordinates; includes zero and EOF.
    boundaries: Vec<(u32, u32)>,
    /// UTF-16 offsets of line starts in ascending order; begins at zero and includes
    /// the position after each LF.
    lines: Vec<u32>,
}

impl CoordinateIndex {
    /// Builds the index in one pass over the source.
    ///
    /// The cost is proportional to the character count, which is why callers should
    /// build it once and reuse it for many span mappings.
    #[allow(clippy::cast_possible_truncation)] // A scalar occupies at most four bytes/two units.
    pub fn new(source: &SourceFile) -> Self {
        let mut boundaries = Vec::new();
        let mut lines = vec![0];
        let mut byte = 0;
        let mut utf16 = 0;
        for character in source.text().chars() {
            boundaries.push((byte, utf16));
            // SourceFile limits the byte length to u32; UTF-16 cannot exceed it.
            byte += character.len_utf8() as u32;
            utf16 += character.len_utf16() as u32;
            if character == '\n' {
                lines.push(utf16);
            }
        }
        boundaries.push((byte, utf16));
        Self { boundaries, lines }
    }

    /// Returns the byte offset of the scalar that starts at `offset`.
    ///
    /// # Errors
    ///
    /// Rejects out-of-range coordinates and surrogate interiors.
    pub fn byte(&self, offset: Utf16Offset) -> Result<BytePos, CoordinateError> {
        self.boundaries
            .binary_search_by_key(&offset.0, |&(_, utf16)| utf16)
            .map(|index| BytePos(self.boundaries[index].0))
            .map_err(|_| CoordinateError::Offset(offset))
    }

    /// Returns the UTF-16 offset of the scalar that starts at `byte`.
    ///
    /// # Errors
    ///
    /// Rejects out-of-range coordinates and UTF-8 interiors.
    pub fn utf16(&self, byte: BytePos) -> Result<Utf16Offset, CoordinateError> {
        self.boundaries
            .binary_search_by_key(&byte.0, |&(byte, _)| byte)
            .map(|index| Utf16Offset(self.boundaries[index].1))
            .map_err(|_| CoordinateError::Byte(byte))
    }

    /// Resolves a zero-based line and UTF-16 column to the byte offset of the scalar at
    /// that position.
    ///
    /// The terminating newline itself is addressable; a column reaching the next
    /// line's start is rejected. EOF is also addressable, including column zero on
    /// the empty line after a trailing newline.
    ///
    /// # Errors
    ///
    /// Rejects nonexistent lines, columns crossing a newline, and surrogate interiors.
    pub fn at_position(&self, position: Position) -> Result<BytePos, CoordinateError> {
        let invalid = || CoordinateError::Position(position);
        let start = *self.lines.get(position.line as usize).ok_or_else(invalid)?;
        let absolute = start
            .checked_add(position.utf16_column)
            .ok_or_else(invalid)?;
        if self
            .lines
            .get(position.line as usize + 1)
            .is_some_and(|&next| absolute >= next)
        {
            return Err(invalid());
        }
        self.byte(Utf16Offset(absolute)).map_err(|_| invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileId;

    #[test]
    fn roundtrips_unicode_crlf_and_eof_without_accepting_scalar_interiors() {
        let source = SourceFile::new(FileId(1), None, None, "a😀\r\n日本語\n").unwrap();
        let index = CoordinateIndex::new(&source);
        for byte in (0..=source.text().len()).map(|byte| BytePos(u32::try_from(byte).unwrap())) {
            if source.text().is_char_boundary(byte.0 as usize) {
                let utf16 = index.utf16(byte).unwrap();
                assert_eq!(index.byte(utf16).unwrap(), byte);
                assert_eq!(
                    index.at_position(source.position(byte).unwrap()).unwrap(),
                    byte
                );
            } else {
                assert!(index.utf16(byte).is_err());
            }
        }
        assert!(index.byte(Utf16Offset(2)).is_err());
        assert!(index.byte(Utf16Offset(u32::MAX)).is_err());
        assert!(
            index
                .at_position(Position {
                    line: 0,
                    utf16_column: 5
                })
                .is_err()
        );
        assert!(
            index
                .at_position(Position {
                    line: 3,
                    utf16_column: 0
                })
                .is_err()
        );
    }

    /// Every scalar boundary is addressable, including the ones around CRLF and the
    /// end of the buffer, while interior offsets and out-of-range lines are not.
    #[test]
    fn line_terminators_and_eof_are_addressable_scalar_boundaries() {
        let source = SourceFile::new(FileId(1), None, None, "😀\r\n").unwrap();
        let index = CoordinateIndex::new(&source);
        for (line, utf16_column, expected) in [(0, 2, 4), (0, 3, 5), (1, 0, 6)] {
            assert_eq!(
                index.at_position(Position { line, utf16_column }).unwrap(),
                BytePos(expected)
            );
        }
        for (line, utf16_column) in [(0, 4), (0, 1), (1, 1)] {
            assert!(
                index.at_position(Position { line, utf16_column }).is_err(),
                "{line}:{utf16_column}"
            );
        }
    }
}
