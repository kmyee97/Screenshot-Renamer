use screenshot_renamer_lib::ocr_text_cleaner::clean_ocr_text;

#[test]
fn removes_noise_and_normalizes_whitespace_conservatively() {
    assert_eq!(clean_ocr_text(" File \nEdit\nView\n|||\n TypeError:\tmain.rs\u{a0}v1.2.3\nTypeError: main.rs v1.2.3\nOrder #AB-123   12.99 "), "TypeError: main.rs v1.2.3\nOrder #AB-123 12.99");
    assert_eq!(
        clean_ocr_text("\u{200b}Hello\u{feff} world\r\n https://example.com/a?b=2\n日本語 résumé"),
        "Hello world\nhttps://example.com/a?b=2\n日本語 résumé"
    );
    assert_eq!(clean_ocr_text("\n---\n...\n_|_\n"), "");
    assert_eq!(
        clean_ocr_text("Project File Manager"),
        "Project File Manager"
    );
}

#[test]
fn clean_input_is_stable_and_output_is_bounded_at_character_boundaries() {
    let input = "error E0432: unresolved import\nmain.rs:12\nOrder #123";
    assert_eq!(clean_ocr_text(input), input);
    let long = clean_ocr_text(&"日".repeat(20000));
    assert!(long.chars().count() <= 4096);
    assert_eq!(clean_ocr_text(&long), long);
}
