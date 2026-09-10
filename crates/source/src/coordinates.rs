//! Explicit UTF-16 coordinates at host and source-map boundaries.

use serde::{Deserialize, Serialize};

use crate::{BytePos, Position, SourceFile};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Utf16Offset(pub u32);

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CoordinateError {
    #[error("invalid UTF-16 offset {0:?}")]
    Offset(Utf16Offset),
    #[error("invalid UTF-16 position {0:?}")]
    Position(Position),
    #[error("invalid UTF-8 byte offset {0:?}")]
    Byte(BytePos),
}

/// Bidirectional scalar-boundary index. Surrogate interiors have no entry.
/// Built once per source so mapping many spans never rescans the source prefix.
#[derive(Clone, Debug)]
pub struct CoordinateIndex {
    boundaries: Vec<(u32, u32)>,
    lines: Vec<u32>,
}

impl CoordinateIndex {
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

    /// # Errors
    /// Rejects out-of-range coordinates and surrogate interiors.
    pub fn byte(&self, offset: Utf16Offset) -> Result<BytePos, CoordinateError> {
        self.boundaries
            .binary_search_by_key(&offset.0, |&(_, utf16)| utf16)
            .map(|index| BytePos(self.boundaries[index].0))
            .map_err(|_| CoordinateError::Offset(offset))
    }

    /// # Errors
    /// Rejects out-of-range coordinates and UTF-8 interiors.
    pub fn utf16(&self, byte: BytePos) -> Result<Utf16Offset, CoordinateError> {
        self.boundaries
            .binary_search_by_key(&byte.0, |&(byte, _)| byte)
            .map(|index| Utf16Offset(self.boundaries[index].1))
            .map_err(|_| CoordinateError::Byte(byte))
    }

    /// # Errors
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
        let source = SourceFile::new(FileId(1), "x", "a😀\r\n日本語\n").unwrap();
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
}
