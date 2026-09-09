use super::*;

#[test]
fn maps_byte_offsets_to_utf16_positions() {
    let source = SourceFile::new(FileId(1), "Button.tsx", "a😀\nβ");

    assert_eq!(
        source.position(BytePos(0)).unwrap(),
        Position {
            line: 0,
            utf16_column: 0
        }
    );
    assert_eq!(
        source.position(BytePos(5)).unwrap(),
        Position {
            line: 0,
            utf16_column: 3
        }
    );
    assert_eq!(
        source.position(BytePos(6)).unwrap(),
        Position {
            line: 1,
            utf16_column: 0
        }
    );
    assert_eq!(source.utf16_offset(BytePos(6)).unwrap(), 4);
}

#[test]
fn normalizes_module_paths_across_platforms() {
    assert_eq!(
        normalized_module_path(r".\src\components\Menu.tsx"),
        "src/components/Menu.tsx"
    );
    assert_eq!(
        normalized_module_path("./src/components/Menu.tsx"),
        "src/components/Menu.tsx"
    );
}
